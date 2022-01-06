use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs;
use std::io::{Error, ErrorKind, Result};

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
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
