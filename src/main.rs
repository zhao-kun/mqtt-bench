use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use clap::Arg;
use config::Config;
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

mod config;

#[derive(PartialEq, Debug)]
enum StressState {
    Connecting,
    Published,
    Publishing,
}
async fn hello_world() -> impl Responder {
    "Hello World!"
}
fn main() {
    let matches = clap::App::new("MQTT stress test program")
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
    actix_web::rt::System::with_tokio_rt(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(8)
            .thread_name("main-tokio")
            .build()
            .unwrap()
    })
    .block_on(async_main(path));
}

async fn async_main(path: &str) {
    let config = config::Config::from_file(path).unwrap();
    let connection = config.connection;
    let mut handles = vec![];
    let arc_cfg = Arc::new(config);
    for i in (0..connection).rev() {
        let cfg = arc_cfg.clone();
        let client = cfg.client_id.clone() + &i.to_string();
        handles.push(tokio::spawn(stressing(client, cfg)))
    }
    tokio::spawn(async {
        futures::future::join_all(handles).await;
        println!("All tasks run finished");
    });

    HttpServer::new(|| {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .route("/", web::get().to(hello_world))
        //.service(index)
    })
    .workers(2)
    .bind("0.0.0.0:8088")
    .expect("Couldn't bind to port 8088")
    .run()
    .await
    .unwrap();
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

    let num = rand::thread_rng().gen_range(1..30000);
    let mut heartbeat = time::interval_at(
        Instant::now().add(Duration::from_millis(num)),
        Duration::from_millis(cfg.think_time as u64),
    );
    let (tx_ch, _rx) = broadcast::channel(10);
    let mut rx_ch = tx_ch.subscribe();
    let payload = get_payload(&cfg.payload);
    loop {
        select! {
            _ = heartbeat.tick() => {
                if let Ok(packet) = new_publish_packet(&state, &cfg, &client, payload.clone()){
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

fn new_publish_packet(
    state: &StressState,
    cfg: &Config,
    client: &str,
    payload: Vec<u8>,
) -> Result<PublishPacket> {
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
        payload,
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

fn get_payload(origin: &str) -> Vec<u8> {
    return match base64::decode(origin) {
        Ok(payload) => payload,
        Err(_) => Vec::from(origin.as_bytes()),
    };
}
