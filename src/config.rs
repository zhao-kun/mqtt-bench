use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs;
use std::io::{Error, ErrorKind, Result};

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_broker_addr")]
    pub broker_addr: String,

    #[serde(default = "default_client_id")]
    pub client_id: String,

    #[serde(default = "default_connection")]
    pub connection: i32,

    #[serde(default = "default_user_name")]
    pub user_name: String,

    #[serde(default = "default_password")]
    pub password: String,

    #[serde(default = "default_think_time")]
    pub think_time: i32,

    pub info_model_id: String,

    pub tenant_name: String,

    #[serde(default = "default_same_things_id")]
    pub same_things_id: bool,

    #[serde(default = "default_topic_suffix")]
    pub topic_suffix: String,

    #[serde(default = "default_is_payload_base64")]
    pub is_payload_base64: bool,

    pub payload: String,
}

fn default_broker_addr() -> String {
    "127.0.0.1:1883".to_string()
}

fn default_client_id() -> String {
    "test".to_string()
}

fn default_connection() -> i32 {
    1000
}

fn default_think_time() -> i32 {
    30000
}
fn default_user_name() -> String {
    "admin".to_string()
}

fn default_password() -> String {
    "admin".to_string()
}

fn default_same_things_id() -> bool {
    true
}

fn default_topic_suffix() -> String {
    "/event/eventName".to_string()
}

fn default_is_payload_base64() -> bool {
    true
}

impl Config {
    pub fn from_file(f: &str) -> Result<Config> {
        match fs::read_to_string(f) {
            Ok(contents) => match serde_yaml::from_str(&contents) {
                Ok(result) => Ok(result),
                Err(e) => {
                    println!("unmarshal contents {} error: {}", contents, e);
                    Err(Error::new(ErrorKind::Other, "unmarshal error"))
                }
            },
            Err(e) => {
                println!("parse file  {} failed {}", f, e);
                return Err(e);
            }
        }
    }
}
