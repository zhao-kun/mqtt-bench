use base64::{engine::general_purpose, Engine as _};
use mqtt::{packet::*, Encodable};
use rand::{self, Rng};
use std::{
    io::{Error, ErrorKind, Result},
    panic,
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpStream, select, sync::broadcast, time, time::Instant};

use crate::config::{self, get_things_password};
use crate::stressing_registry;
use crate::util::{render_template, MyClient};

#[derive(PartialEq, Debug)]
enum StressState {
    Connecting,
    Published,
    Publishing,
}

pub async fn run(
    http_client: Arc<MyClient>,
    registry: Arc<stressing_registry::MetricRegistry>,
    cfg: Arc<config::Config>,
    things_idx: usize,
) {
    // Send ConnectPacket to the broker
    let mut state = StressState::Connecting;
    let mut stream;
    let client_id = cfg.get_client_id(things_idx);
    shuffle_sleep(120000).await;
    if let Ok(str) = connect_broker(&cfg, things_idx, &client_id, http_client).await {
        stream = str;
    } else {
        registry.exited_tasks_inc();
        return;
    }

    let (mut rx, mut tx) = stream.split();

    // Increases running task counter
    registry.running_tasks_inc();
    registry.ongoing_connection_inc();

    // Generating random delays for hashing publish packet action
    let mut heartbeat =
        time::interval_at(Instant::now(), Duration::from_millis(cfg.think_time as u64));

    // Using a dedicated channel for sending publish packet
    let (tx_ch, _rx) = broadcast::channel(10);
    let mut rx_ch = tx_ch.subscribe();

    // Calculating the payload
    let payload = get_payload(&cfg, things_idx);

    let loops = cfg.duration * 1000 / cfg.think_time;
    let mut sent = 0;
    let mut sending = 0;
    let mut sendack = 0;
    let topic = get_topic(&cfg, things_idx, &client_id);

    // Main loop
    loop {
        if sendack >= loops {
            println!("client_id {} normaly finished,", client_id);
            break;
        }
        select! {
            _ = heartbeat.tick() => {
                if let Ok(packet) = new_publish_packet(&client_id, &state, &topic,  &payload){
                    tx_ch.send(packet).unwrap();
                    sending += 1;
                }else {
                    registry.timeout_pubacks_inc();
                }
            },
            result = rx_ch.recv() => {
                let packet = result.unwrap();
                let mut buf = Vec::new();
                packet.encode(&mut buf).unwrap();
                tx.write_all(&buf[..]).await.unwrap();
                state = StressState::Publishing;
                sent = sent + 1;
            },
            result = VariablePacket::parse(&mut rx) => {
                let packet = match result {
                    Ok(packet) => packet,
                    Err(e) => {
                        println!("parse packet error:{}", e);
                        break;
                    }
                };

                match packet {
                    VariablePacket::PingrespPacket(..) => {
                        println!("Receiving PINGRESP from broker ..");
                    }
                    VariablePacket::ConnackPacket(_ack) => {
                        if state == StressState::Connecting && _ack.connect_return_code() == mqtt::control::ConnectReturnCode::ConnectionAccepted{
                            state = StressState::Published;
                            println!("client_id: {} connection was established", client_id);
                            registry.established_connection_inc();
                            registry.ongoing_connection_decr();
                        } else {
                            println!("client_ID: {} failed to authorize, early exited, recv invalid connack {:?} under the state {:?}, task ended!",client_id, _ack, state);
                            stream.shutdown().await.unwrap();
                            registry.ongoing_connection_decr();
                            registry.exited_tasks_inc();
                            return;
                        }
                    }
                    VariablePacket::PubackPacket(_ack) => {
                        if state == StressState::Publishing {
                            state = StressState::Published;
                            sendack +=1;
                            registry.publish_packets_inc();
                        } else {
                            println!("client_id: {} recv invalid Puback, puback should be return when state is publishing", client_id);
                            registry.invalid_pubacks_inc();
                        }
                    }
                    _ => {
                    }
                }
            },
        }
    }

    println!(
        "client_id: {} task finished, total sent {}, sending {}, sendack {}",
        client_id, sent, sending, sendack
    );
    // Updating counter of the exiting tasks
    stream.shutdown().await.unwrap();
    registry.exited_tasks_inc();
    registry.established_connection_decr();
    return;
}

async fn retry<'a, F, T>(
    f: F,
    http_client: &'a Arc<MyClient>,
    cfg: &'a config::Config,
    things_idx: usize,
    total: usize,
) -> String
where
    F: Fn(&'a Arc<MyClient>, &'a config::Config, usize) -> T + 'a,
    T: std::future::Future<Output = String>,
{
    let mut count = 0;
    loop {
        let result = f(http_client, cfg, things_idx).await;
        if result.eq("") {
            count = count + 1;
            if count > total {
                return "".to_string();
            }
            println!(
                "retrying due to error, get things password, count is {}",
                count
            );
            shuffle_sleep(5000).await;
        } else {
            return result;
        }
    }
}

async fn connect_broker<'a>(
    cfg: &'a config::Config,
    things_idx: usize,
    client_id: &'a str,
    http_client: Arc<MyClient>,
) -> std::result::Result<TcpStream, std::io::Error> {
    println!("client id is {}", client_id);
    let password = retry(get_things_password, &http_client, cfg, things_idx, 10).await;
    if password.eq("") {
        return Err(Error::new(
            ErrorKind::Other,
            "server can't handle request, password is empty",
        ));
    }

    let mut broker_addr = cfg.broker_addr[0].clone();
    if cfg.broker_addr.len() > 1 {
        let num = rand::thread_rng().gen_range(0..cfg.broker_addr.len());
        broker_addr = cfg.broker_addr[num].clone()
    }

    shuffle_sleep(30000).await; // avoid the file descriptor was exhausted
    let mut stream = match TcpStream::connect(&broker_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("connect {} error: {}", broker_addr, e);
            return Err(e);
        }
    };
    println!("broker {} was connected send connect packet", broker_addr);

    let mut conn = ConnectPacket::new(client_id);
    conn.set_clean_session(true);
    conn.set_user_name(Option::Some(cfg.user_name.clone()));
    conn.set_password(Option::Some(password));
    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    stream.write_all(&buf[..]).await?;
    Ok(stream)
}
// shuffle_sleep sleep random mills millseconds
async fn shuffle_sleep(max_mills: u64) {
    let mills = rand::thread_rng().gen_range(1..max_mills);
    time::sleep(Duration::from_millis(mills)).await;
}

fn new_publish_packet(
    client_id: &str,
    state: &StressState,
    topic: &String,
    payload: &Vec<u8>,
) -> Result<PublishPacket> {
    if state != &StressState::Published {
        println!(
            "Do nothing for client: {} as connection not build, current state is {:?}",
            client_id, state
        );
        return Err(Error::new(ErrorKind::Other, "not ready"));
    }

    let packet = PublishPacket::new(
        mqtt::TopicName::new(topic).unwrap(),
        QoSWithPacketIdentifier::Level1(1),
        payload.clone(),
    );
    return Ok(packet);
}

fn get_topic(cfg: &config::Config, idx: usize, client_id: &str) -> String {
    let context = cfg.to_context(idx, client_id);
    render_template(&cfg.topic_template, &context)
}

fn get_payload(cfg: &config::Config, idx: usize) -> Vec<u8> {
    let tenant_name = &cfg.things_info[idx].tenant_name;
    let res = cfg.things_payloads.get(tenant_name);
    match res {
        Some(payload) => {
            return if cfg.is_payload_base64 {
                println!("convert payload to base64");
                return match general_purpose::STANDARD.decode(payload) {
                    Ok(result) => {
                        let mut vec = Vec::new();
                        vec.extend_from_slice(&result);
                        println!("payload length is {}", vec.len());
                        vec
                    }
                    Err(e) => {
                        println!("invalid base64 payload {}, error is {}", payload, e);
                        panic!("payload isn't base64");
                    }
                };
            } else {
                Vec::from(payload.as_bytes())
            };
        }
        None => {
            panic!(
                "things_idx{} client_id{} hasn't payload",
                idx, cfg.things_info[idx].third_things_id
            );
        }
    }
}

#[cfg(test)]
mod tests {

    const YAML_STR2: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: publish
metaData:
  name: task-demo
spec:
  brokerAddr: ["192.168.24.245:1883"]
  clientId: prefix
  dynamicToken:
    servers:
    - 192.168.1.1
    - 192.168.1.2
    - 192.168.1.3
    url: /v2/things/mqtt/tokens
    method: POST
    payload: '{"devices":[{"devid":"${thirdThingsId}","devtype":"${infoModelName}"}],"password":"${password}","username":"${tenantName}"}'
    tokenExtractor: "$.data.token"
  topicTemplate: /d2s/${tenantName}/${infoModelName}/${thirdThingsId}/data
  thinkTime: 10000
  duration: 60
  thingsPayloads:
    "pressure3": AHRvdGFsX2VuZXJneQAyMC43MQB0b2RheV9lbmVyZ3kANTAuNzQAdGVtcGVyYXR1cmUAOTguNzIAZ2ZjaQA2OS45NgBidXNfdm9sdAA4MC42MQBwb3dlcgAyMC45MQBxX3Bvd2VyADQ1LjMyAHBmADg3LjQyAHB2MV92b2x0ADIwLjEyAHB2MV9jdXJyADMyLjEAcHYyX3ZvbHQAMjAuNzUAcHYyX2N1cnIANzcuMjUAcHYzX3ZvbHQAODkuNwBwdjNfY3VycgA4Ni45NgBsMV92b2x0ADQxLjUyAGwxX2N1cnIAOTIuMTcAbDFfZnJlcQAzMi4xNQBsMV9kY2kAOTAuMjMAbDFfcG93ZXIAOTMuOABsMV9wZgA4LjgAdGltZQAxNjc1MjQwMjY4MjAxAA==
  thingsInfo:
  - tenantName: "pressure3"
    infoModelName: "invert"
    thirdThingsId: "device_invert_3_172"
    password: "12345678"
"#;

    #[test]
    fn test_get_payload() {
        use crate::config::spec_from_str;
        use crate::stressing::get_payload;

        let stressing = spec_from_str(YAML_STR2).unwrap();
        let config = match stressing.spec {
            crate::config::Spec::Publish(config) => config,
            _ => panic!("invalid config"),
        };
        let payload = get_payload(&config, 0);
        println!("payload length is {}", payload.len());
        assert!(payload.len() != 0);
    }
}
