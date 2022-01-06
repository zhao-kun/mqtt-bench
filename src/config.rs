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
    pub payload: String,
    #[serde(default = "default_think_time")]
    pub think_time: i32,
    pub info_model_id: String,
    pub tenant_name: String,
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
