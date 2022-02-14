use mqtt::{packet::*, Encodable};
use rand::{self, Rng};
use std::{
    io::{Error, ErrorKind, Result},
    ops::Add,
    panic,
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpStream, select, sync::broadcast, time, time::Instant};

use crate::config;
use crate::stressing_registry;

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

    // Main loop
    loop {
        if current > loops {
            println!("loop ended, task finished");
            return;
        }
        select! {
            _ = heartbeat.tick() => {
                if let Ok(packet) = new_publish_packet(&state, &cfg,  payload.clone()){
                    tx_ch.send(packet).unwrap();
                }else {
                    registry.timeout_pubacks_inc();
                    println!("heartbeat arrive but puback not received");
                }
                current = current + 1;
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
                            println!("connection was established")
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
    cfg: &config::Config,
    payload: Vec<u8>,
) -> Result<PublishPacket> {
    if state != &StressState::Published {
        println!("Do nothing as connection not build");
        return Err(Error::new(ErrorKind::Other, "not ready"));
    }
    let prefix = String::from("/d2s/") + &cfg.tenant_name + "/" + &cfg.info_model_id;
    let topic = prefix + "/" + &cfg.third_things_id + &cfg.topic_suffix;

    let packet = PublishPacket::new(
        mqtt::TopicName::new(topic).unwrap(),
        QoSWithPacketIdentifier::Level1(1),
        payload,
    );
    return Ok(packet);
}

async fn connect_broker(client: &str, cfg: &config::Config) -> Result<TcpStream> {
    let mut stream = match TcpStream::connect(&cfg.broker_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("connect {} error: {}", cfg.broker_addr, e);
            return Err(e);
        }
    };
    println!(
        "broker {} was connected send connect packet",
        cfg.broker_addr
    );

    let client_id = if cfg.same_client_id {
        &cfg.client_id
    } else {
        client
    };

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
