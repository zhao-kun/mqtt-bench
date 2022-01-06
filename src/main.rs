use clap::Arg;
use std::sync::Arc;
use std::time::Duration;

use tokio::{select, time, time::Instant};

use metrics_util::MetricKindMask;

use metrics_exporter_prometheus::PrometheusBuilder;
use sys_info;

mod config;
mod stressing;
mod stressing_registry;

#[tokio::main]
async fn main() {
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
    let config = config::Config::from_file(path).expect("config file should be a valid yaml file");
    let connection = config.connection;
    let mut handles = vec![];
    let arc_cfg = Arc::new(config);

    let builder = PrometheusBuilder::new();
    builder
        .idle_timeout(
            MetricKindMask::COUNTER | MetricKindMask::GAUGE,
            Some(Duration::from_secs(10)),
        )
        .install()
        .expect("failed to install Prometheus recorder");

    let hostname = sys_info::hostname().unwrap();
    let reg = Arc::new(stressing_registry::MetricRegistry::new());

    // Run tasks for the stressing test
    for i in (0..connection).rev() {
        let cfg = arc_cfg.clone();
        let client = cfg.client_id.clone() + &i.to_string();
        handles.push(tokio::spawn(stressing::run(reg.clone(), client, cfg)))
    }

    let registry = reg.clone();
    let labels = [(String::from("host"), hostname)];
    tokio::spawn(async move {
        let mut heartbeat = time::interval_at(Instant::now(), Duration::from_millis(1000));
        loop {
            select! {
                _ = heartbeat.tick() => {
                    registry.update(&labels);
                },
            }
        }
    });

    futures::future::join_all(handles).await;
    println!("All tasks run finished");
}
