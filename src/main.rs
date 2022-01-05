use clap::{App, Arg};
use config::Config;
use mqtt::packet::*;
use std::{
    io::{Error, ErrorKind, Result},
    panic,
    sync::Arc,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpStream, select, sync::broadcast, time, time::Instant};

mod config;

#[derive(PartialEq, Debug)]
enum StressState {
    Connecting,
    Published,
    Publishing,
}

#[tokio::main]
async fn main() {
    let matches = App::new("MQTT stress test program")
        .version("0.1.0")
        .author("Kun Zhao")
        .about("MQTT stress test program")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .takes_value(true)
                .help("Config file for stress test"),
        )
        .get_matches();

    let path = matches.value_of("file").unwrap_or("config.yml");

    let config = config::Config::from_file(path).unwrap();
    let connection = config.connection;
    let mut handles = vec![];
    let arc_cfg = Arc::new(config);
    for i in (0..connection).rev() {
        let cfg = arc_cfg.clone();
        let client = cfg.client_id.clone() + &i.to_string();
        let mut state = StressState::Connecting;
        handles.push(tokio::spawn(async move {
            let mut stream = connect_broker(&cfg).await.unwrap();
            let (mut rx, mut tx) = stream.split();
            let mut heartbeat = time::interval_at(Instant::now(), Duration::from_secs(1)); 
            let (tx_ch, _rx) = broadcast::channel(10);
            let mut rx_ch = tx_ch.subscribe();
            loop {
                select! {
                    _ = heartbeat.tick() => {
                        if let Ok(packet) = send_packet(&state, &cfg, &client){
                            tx_ch.send(packet).unwrap();
                        }else {
                            println!("heartbeat arrive but puback not received");
                        }
                    },
                    result = rx_ch.recv() => {
                        let packet = result.unwrap();
                        let mut buf = Vec::new();
                        packet.encode_packet(&mut buf).unwrap();
                        tx.write_all(&buf[..]).await.unwrap();
                        state = StressState::Publishing;
                    },
                    Ok(packet) = VariablePacket::parse(&mut rx) => {
                        match packet {
                            VariablePacket::PingrespPacket(..) => {
                                println!("Receiving PINGRESP from broker ..");
                            }
                            VariablePacket::ConnackPacket(_ack) => {
                                if state == StressState::Connecting && _ack.connect_return_code() == mqtt::control::ConnectReturnCode::Reserved(0) {
                                    state = StressState::Published;
                                } else {
                                    println!("recv invalid connack {:?} under the state {:?}, task ended!", _ack, state);
                                    return ;
                                }
                            }
                            VariablePacket::PubackPacket(_ack) => {
                                if state == StressState::Publishing {
                                    state = StressState::Published;
                                } else {
                                    println!("recv invalid Puback, puback should be return when state is publishing");
                                }
                            }
                            _ => {

                            }
                        }
                    }
                }
            }
        }));
    }
    futures::future::join_all(handles).await;
}

fn send_packet(state: &StressState, cfg: &Config, client: &str) -> Result<PublishPacket> {
    if state != &StressState::Published {
        println!("Do nothing as connection not build");
        return Err(Error::new(ErrorKind::Other, "not ready"));
    }
    let topic =
        String::from("/") + &cfg.tenant_name + "/" + &cfg.info_model_id + "/" + &client + "/status";
    let packet = PublishPacket::new(
        mqtt::TopicName::new(topic).unwrap(),
        QoSWithPacketIdentifier::Level1(1),
        cfg.payload.clone(),
    );
    return Ok(packet);
}

async fn connect_broker(cfg: &Config) -> Result<TcpStream> {
    let mut stream = match TcpStream::connect(&cfg.broker_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("connect {} error: {}", cfg.broker_addr, e);
            return Err(e);
        }
    };
    let mut conn = ConnectPacket::new(cfg.client_id.clone());
    conn.set_user_name(Option::Some(cfg.user_name.clone()));
    conn.set_password(Option::Some(cfg.password.clone()));
    let mut buf = Vec::new();
    conn.encode_packet(&mut buf).unwrap();
    stream.write_all(&buf[..]).await.unwrap();
    Ok(stream)
}
