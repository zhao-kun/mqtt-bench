use mqtt::{packet::*, Encodable};
use rand::{self, Rng};
use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Result},
    ops::Add,
    panic,
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpStream, select, sync::broadcast, time, time::Instant};

use crate::config;
use crate::stressing_registry;
use text_template::*;

#[derive(PartialEq, Debug)]
enum StressState {
    Connecting,
    Published,
    Publishing,
}

pub async fn run(
    registry: Arc<stressing_registry::MetricRegistry>,
    client: String,
    cfg: Arc<config::Config>,
) {
    // Send ConnectPacket to the broker
    let mut state = StressState::Connecting;
    let mut stream;
    if let Ok(str) = connect_broker(&client, &cfg).await {
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
    let num = rand::thread_rng().gen_range(1..30000);
    let mut heartbeat = time::interval_at(
        Instant::now().add(Duration::from_millis(num)),
        Duration::from_millis(cfg.think_time as u64),
    );

    // Using a dedicated channel for sending publish packet
    let (tx_ch, _rx) = broadcast::channel(10);
    let mut rx_ch = tx_ch.subscribe();

    // Calculating the payload
    let payload = if cfg.is_payload_base64 {
        get_payload(&cfg.payload)
    } else {
        Vec::from(cfg.payload.as_bytes())
    };

    let loops = cfg.duration * 1000 / cfg.think_time;
    let mut current = 0;
    let topic = get_topic(&client, &cfg);

    // Main loop
    loop {
        if current > loops {
            println!("loop ended, task finished");
            return;
        }
        select! {
            _ = heartbeat.tick() => {
                if let Ok(packet) = new_publish_packet(&state, &topic,  payload.clone()){
                    tx_ch.send(packet).unwrap();
                    current = current + 1;
                }else {
                    registry.timeout_pubacks_inc();
                    println!("heartbeat arrive but puback not received");
                }
            },
            result = rx_ch.recv() => {
                let packet = result.unwrap();
                let mut buf = Vec::new();
                packet.encode(&mut buf).unwrap();
                tx.write_all(&buf[..]).await.unwrap();
                state = StressState::Publishing;
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
                            println!("connection was established");
                            registry.established_connection_inc();
                        } else {
                            println!("recv invalid connack {:?} under the state {:?}, task ended!", _ack, state);
                            break;
                        }
                    }
                    VariablePacket::PubackPacket(_ack) => {
                        if state == StressState::Publishing {
                            state = StressState::Published;
                            registry.publish_packets_inc();
                        } else {
                            println!("recv invalid Puback, puback should be return when state is publishing");
                            registry.invalid_pubacks_inc();
                        }
                    }
                    _ => {
                    }
                }
            },
        }
    }

    // Updating counter of the exiting tasks
    registry.exited_tasks_inc();
    return;
}

fn new_publish_packet(
    state: &StressState,
    topic: &String,
    payload: Vec<u8>,
) -> Result<PublishPacket> {
    if state != &StressState::Published {
        println!(
            "Do nothing as connection not build, current state is {:?}",
            state
        );
        return Err(Error::new(ErrorKind::Other, "not ready"));
    }

    let packet = PublishPacket::new(
        mqtt::TopicName::new(topic).unwrap(),
        QoSWithPacketIdentifier::Level1(1),
        payload,
    );
    return Ok(packet);
}

fn get_topic(client: &String, cfg: &config::Config) -> String {
    //let prefix = String::from("/d2s/") + &cfg.tenant_name + "/" + &cfg.info_model_id;
    //let topic = prefix + "/" + &cfg.third_things_id + &cfg.topic_suffix;
    let mut context: HashMap<&str, &str> = cfg
        .data
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    context.insert("client_id", client.as_str());

    let template = Template::from(cfg.topic_template.as_str());
    let text = template.fill_in(&context);
    return text.to_string();
}

async fn connect_broker(client: &str, cfg: &config::Config) -> Result<TcpStream> {
    let mut broker_addr = cfg.broker_addr[0].clone();
    if cfg.broker_addr.len() > 1 {
        let num = rand::thread_rng().gen_range(0..cfg.broker_addr.len());
        broker_addr = cfg.broker_addr[num].clone()
    }
    let mut stream = match TcpStream::connect(&broker_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("connect {} error: {}", broker_addr, e);
            return Err(e);
        }
    };
    println!("broker {} was connected send connect packet", broker_addr);

    let client_id = if cfg.same_client_id {
        &cfg.client_id
    } else {
        client
    };

    println!("client id is {}", client_id);

    let mut conn = ConnectPacket::new(client_id);
    conn.set_clean_session(true);
    conn.set_user_name(Option::Some(cfg.user_name.clone()));
    conn.set_password(Option::Some(cfg.password.clone()));
    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    stream.write_all(&buf[..]).await?;

    Ok(stream)
}

fn get_payload(origin: &str) -> Vec<u8> {
    return match base64::decode(origin) {
        Ok(payload) => payload,
        Err(_) => Vec::from(origin.as_bytes()),
    };
}
