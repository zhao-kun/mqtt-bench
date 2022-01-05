use clap::{App, Arg};
use config::Config;
use mqtt::{packet::*, Encodable};
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
        handles.push(tokio::spawn(stressing(client, cfg)))
    }
    futures::future::join_all(handles).await;
    println!("All tasks run finished");
}

async fn stressing(client: String, cfg: Arc<Config>) {
    let mut state = StressState::Connecting;
    let mut stream;
    if let Ok(str) = connect_broker(&client, &cfg).await {
        stream = str;
    } else {
        return;
    }
    let (mut rx, mut tx) = stream.split();
    let mut heartbeat =
        time::interval_at(Instant::now(), Duration::from_millis(cfg.think_time as u64));
    let (tx_ch, _rx) = broadcast::channel(10);
    let mut rx_ch = tx_ch.subscribe();
    loop {
        select! {
            _ = heartbeat.tick() => {
                if let Ok(packet) = new_publish_packet(&state, &cfg, &client){
                    tx_ch.send(packet).unwrap();
                }else {
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
                        return ;
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
            },
        }
    }
}

fn new_publish_packet(state: &StressState, cfg: &Config, client: &str) -> Result<PublishPacket> {
    if state != &StressState::Published {
        println!("Do nothing as connection not build");
        return Err(Error::new(ErrorKind::Other, "not ready"));
    }
    let topic = String::from("/d2s/")
        + &cfg.tenant_name
        + "/"
        + &cfg.info_model_id
        + "/"
        + client
        + "/event/eventName";
    let packet = PublishPacket::new(
        mqtt::TopicName::new(topic).unwrap(),
        QoSWithPacketIdentifier::Level1(1),
        cfg.payload.clone(),
    );
    return Ok(packet);
}

async fn connect_broker(client: &str, cfg: &Config) -> Result<TcpStream> {
    let mut stream = match TcpStream::connect(&cfg.broker_addr).await {
        Ok(stream) => stream,
        Err(e) => {
            println!("connect {} error: {}", cfg.broker_addr, e);
            return Err(e);
        }
    };
    println!(
        "broker {} was connected send connpkt packet",
        cfg.broker_addr
    );
    let mut conn = ConnectPacket::new(client);
    conn.set_clean_session(true);
    conn.set_user_name(Option::Some(cfg.user_name.clone()));
    conn.set_password(Option::Some(cfg.password.clone()));
    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    stream.write_all(&buf[..]).await?;

    Ok(stream)
}
