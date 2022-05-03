use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{
    collections::HashMap,
    fs,
    io::{Error, ErrorKind, Result},
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    pub name: String,
    #[serde(default = "default_meta_label")]
    pub label: HashMap<String, String>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GVK {
    group: String,
    version: String,
    meta_data: MetaData,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stressing {
    #[serde(flatten)]
    gvk: GVK,

    #[serde(flatten)]
    pub spec: Spec,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "spec")]
pub enum Spec {
    Test(Value),
    Publish(Config),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Value {
    value: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_broker_addr")]
    pub broker_addr: Vec<String>,

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

    #[serde(default = "default_same_client_id")]
    pub same_client_id: bool,

    #[serde(default = "default_topic_suffix")]
    pub topic_suffix: String,

    #[serde(default = "default_is_payload_base64")]
    pub is_payload_base64: bool,

    pub payload: String,

    #[serde(default = "default_duration")]
    pub duration: i32,

    #[serde(default = "default_data")]
    pub data: HashMap<String, String>,

    pub topic_template: String,
}

fn default_data() -> HashMap<String, String> {
    HashMap::new()
}

fn default_meta_label() -> HashMap<String, String> {
    HashMap::new()
}

fn default_broker_addr() -> Vec<String> {
    vec!["127.0.0.1:1883".to_string()]
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

fn default_same_client_id() -> bool {
    false
}

fn default_topic_suffix() -> String {
    "/event/eventName".to_string()
}

fn default_is_payload_base64() -> bool {
    true
}

// default duration is one minute.
fn default_duration() -> i32 {
    60
}

pub trait GroupVersionKind {
    fn group(&self) -> String;
    fn version(&self) -> String;
    fn kind(&self) -> String;
    fn meta(&self) -> MetaData;
}

impl GroupVersionKind for Stressing {
    fn group(&self) -> String {
        self.gvk.group.clone()
    }

    fn version(&self) -> String {
        self.gvk.version.clone()
    }

    fn kind(&self) -> String {
        match self.spec {
            Spec::Test(_) => "test".to_string(),
            Spec::Publish(_) => "publish".to_string(),
        }
    }

    fn meta(&self) -> MetaData {
        self.gvk.meta_data.clone()
    }
}

impl Stressing {
    pub fn from_file(f: &str) -> Result<Stressing> {
        match fs::read_to_string(f) {
            Ok(contents) => spec_from_str(&contents),
            Err(e) => {
                println!("parse file {} failed{}", f, e);
                Err(e)
            }
        }
    }
}

fn spec_from_str(contents: &str) -> Result<Stressing> {
    match serde_yaml::from_str(contents) {
        Ok(result) => Ok(result),
        Err(e) => {
            println!("unmarshal contents {} error:{}", contents, e);
            Err(Error::new(ErrorKind::Other, "unmarshal error"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{spec_from_str, GroupVersionKind, Spec};

    static YAML_STR: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: publish
metaData:
  name: task-demo
spec:
  brokerAddr: ["127.0.0.1:1883"]
  clientId: test
  connection: 100
  userName: admin
  password: admin
  payload: "hello world"
  thinkTime: 5000
  duration: 60
  topicTemplate: /prefix/${tenantName}/${infoModelId}/${thirdThingsId}
  data:
    tenantName: "zlg"
    infoModelId: "google"
    thirdThingsId: client1
"#;
    static YAML_STR2: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: test
metaData:
  name: task-demo
spec:
  value: 127.0.0.1:1883
"#;

    #[test]
    fn spec_should_be_unmarshal() {
        let spec = spec_from_str(YAML_STR).unwrap();
        println!(" spec is {:?}", spec);
        assert!(spec.group() == "github.com/zhao-kun/mqtt-bench");
        assert!(spec.version() == "v1.0.1");
        assert!(spec.meta().name == "task-demo");
        assert!(spec.kind() == "publish");
        let config = match spec.spec {
            Spec::Publish(publish) => publish,
            _ => panic!("should be publish spec"),
        };
        assert!(config.data["tenantName"] == "zlg");
        assert!(config.topic_template == "/prefix/${tenantName}/${infoModelId}/${thirdThingsId}");
    }

    #[test]
    fn spec_should_be_unmarshal2() {
        let spec = spec_from_str(YAML_STR2).unwrap();
        println!(" spec is {:?}", spec);
        assert!(spec.group() == "github.com/zhao-kun/mqtt-bench");
        assert!(spec.version() == "v1.0.1");
        assert!(spec.kind() == "test");
    }
}
