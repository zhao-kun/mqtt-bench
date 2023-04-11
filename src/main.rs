use config::{Config, GroupVersionKind};
use std::time::Duration;
use std::{sync::Arc, thread::sleep};
use stressing_registry::MetricRegistry;

use tokio::{
    select,
    task::JoinHandle,
    time::Instant,
    time::{self},
};

use metrics_util::MetricKindMask;

use metrics_exporter_prometheus::PrometheusBuilder;
use sys_info;

mod config;
mod stressing;
mod stressing_registry;
mod util;

#[tokio::main]
async fn main() {
    let matches = clap::Command::new("MQTT stress test program")
        .version("0.2.0")
        .author("Kun Zhao")
        .about("MQTT stress test program")
        .arg(
            clap::arg!(--"file" <PATH>)
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .short(Some('f'))
                .help("Config file for stress test"),
        )
        .arg(
            clap::arg!(--"max-connections" <NUM>)
                .value_parser(clap::value_parser!(usize))
                .short(Some('c'))
                .help("Max connections for the test"),
        )
        .get_matches();

    // Start prometheus exporter
    let builder = PrometheusBuilder::new();
    builder
        .idle_timeout(
            MetricKindMask::COUNTER | MetricKindMask::GAUGE,
            Some(Duration::from_secs(10)),
        )
        .install()
        .expect("failed to install Prometheus recorder");

    let path = matches
        .get_one::<std::path::PathBuf>("file")
        .expect("config file path must be specified");

    let max_connnection = matches
        .get_one::<usize>("max-connections")
        .unwrap_or(&usize::MAX);

    let file_path = path.as_os_str().to_str().expect("extract file path");
    let spec =
        config::Stressing::from_file(file_path).expect("config file should be a valid yaml file");

    let task_name = spec.meta().name;
    let original = stressing_registry::MetricRegistry::new(task_name);
    original.start_task();
    let reg = Arc::new(original);
    let handles = match spec.spec {
        config::Spec::Test(_) => panic!("unsupported spec"),
        config::Spec::Publish(config) => start_publish_tasks(reg.clone(), config, max_connnection),
    };

    futures::future::join_all(handles).await;
    reg.task_stopped();

    println!("Sleep 30 seconds before exiting...");
    sleep(Duration::from_secs(30));
    println!("All tasks run finished");
}

fn start_publish_tasks(
    reg: Arc<MetricRegistry>,
    config: Config,
    max_connection: &usize,
) -> Vec<JoinHandle<()>> {
    let len = if config.things_info.len() < *max_connection {
        config.things_info.len()
    } else {
        *max_connection as usize
    };

    let mut handles = vec![];
    let arc_cfg = Arc::new(config);

    let hostname = sys_info::hostname().unwrap();

    // Run tasks for the stressing test
    for i in 0..len {
        let cfg = arc_cfg.clone();

        handles.push(tokio::spawn(stressing::run(reg.clone(), cfg, i)))
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
    return handles;
}
