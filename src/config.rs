use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs;
use std::io::Result;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub broker_addr: String,
    pub client_id: String,
    pub connection: i32,
    pub user_name: String,
    pub password: String,
    pub payload: String,
    pub think_time: i32,
    pub info_model_id: String,
    pub tenant_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            broker_addr: String::from("127.0.0.1:1818"),
            client_id: String::from("test"),
            connection: Default::default(),
            user_name: Default::default(),
            password: Default::default(),
            payload: Default::default(),
            think_time: Default::default(),
            info_model_id: Default::default(),
            tenant_name: Default::default(),
        }
    }
}

impl Config {
    pub fn from_file(f: &str) -> Result<Config> {
        let buf = fs::read_to_string(f);
        match buf {
            Ok(contents) => {
                let deserialized = serde_yaml::from_str(&contents).unwrap();
                return Ok(deserialized);
            }
            Err(e) => {
                println!("parse file  {} failed {}", f, e);
                return Err(e);
            }
        }
    }
}
