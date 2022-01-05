use actix_web::{web, App, HttpServer, Responder};
use clap::Arg;
use std::sync::Arc;

mod config;
mod stressing;

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
        handles.push(tokio::spawn(stressing::run(client, cfg)))
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
