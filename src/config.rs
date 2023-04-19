use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::{
    collections::HashMap,
    fs,
    io::{Error, ErrorKind, Result},
};

use crate::util::{http_rpc_call, render_template};

const DEFAULT_AUTHENTICATION_PAYLOAD: &str = r#"
{
    "devices":[
        {
            "devid":"${thirdThingsId}",
            "devtype":"${infoModelName}"
        }
    ],
    "password":"${password}",
    "username":"${tenantName}"
}
"#;

const DEFAULT_TOKEN_EXTRACTOR: &str = ".data.token";

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
pub struct DynamicToken {
    pub url: String,
    pub payload: String,
    pub token_extractor: String,
    #[serde(default = "default_method_value")]
    pub method: String,
    pub servers: Vec<String>,
}

fn default_method_value() -> String {
    "POST".to_string()
}

impl DynamicToken {
    pub fn new() -> DynamicToken {
        DynamicToken {
            url: "".to_string(),
            payload: DEFAULT_AUTHENTICATION_PAYLOAD.to_string(),
            token_extractor: DEFAULT_TOKEN_EXTRACTOR.to_string(),
            method: default_method_value(),
            servers: vec![],
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThingsInfo {
    pub tenant_name: String,

    pub info_model_name: String,

    pub third_things_id: String,

    pub password: String,

    #[serde(default = "default_hashmap")]
    pub context: HashMap<String, String>,
}

impl ThingsInfo {
    pub fn to_map(self: &ThingsInfo) -> HashMap<&str, &str> {
        let mut result: HashMap<&str, &str> = self
            .context
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        insert("tenantName", &self.tenant_name, &mut result);
        insert("infoModelName", &self.info_model_name, &mut result);
        insert("thirdThingsId", &self.third_things_id, &mut result);
        insert("password", &self.password, &mut result);
        result
    }
}
fn insert<'a: 'b, 'b>(k: &'a str, value: &'a str, m: &mut HashMap<&'b str, &'b str>) {
    if value.len() >= 2
        && value.char_indices().nth(0).unwrap().1 == '"'
        && value.char_indices().nth_back(0).unwrap().1 == '"'
    {
        m.insert(k, rem_first_and_last(value));
    } else {
        m.insert(k, value);
    }
}

fn rem_first_and_last(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next();
    chars.next_back();
    chars.as_str()
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_broker_addr")]
    pub broker_addr: Vec<String>,

    #[serde(default = "default_client_id")]
    pub client_id: String,

    #[serde(default = "default_user_name")]
    pub user_name: String,

    #[serde(default = "default_password")]
    pub password: String,

    #[serde(default = "default_think_time")]
    pub think_time: i32,

    #[serde(default = "default_random_client_id")]
    pub random_client_id: bool,

    #[serde(default = "default_topic_suffix")]
    pub topic_suffix: String,

    #[serde(default = "default_is_payload_base64")]
    pub is_payload_base64: bool,

    #[serde(default = "default_hashmap")]
    pub things_payloads: HashMap<String, String>,

    #[serde(default = "default_duration")]
    pub duration: i32,

    #[serde(default = "default_things_info")]
    pub things_info: Vec<ThingsInfo>,

    pub topic_template: String,

    #[serde(default = "default_dynamic_token")]
    pub dynamic_token: DynamicToken,
}

impl Config {
    pub fn to_context<'a>(
        &'a self,
        things_idx: usize,
        client_id: &'a str,
    ) -> HashMap<&str, &'a str> {
        let mut result = self.things_info[things_idx].to_map();
        result.insert("clientId", client_id);
        result
    }
    pub fn get_client_id(&self, things_idx: usize) -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        let client = self.client_id.clone() + &s;

        if self.random_client_id {
            return client;
        };
        let str = self.things_info[things_idx]
            .info_model_name
            .to_owned()
            .clone();
        let third = &self.things_info[things_idx].third_things_id;
        str + ":" + third
    }

    pub async fn get_things_password(&self, things_idx: usize) -> String {
        if self.dynamic_token.url == "" {
            return self.password.clone();
        }

        let context = self.things_info[things_idx].to_map();
        let request = render_template(&self.dynamic_token.payload, &context);

        let idx = rand::thread_rng().gen_range(1..self.dynamic_token.servers.len());
        let url = self.dynamic_token.servers[idx].clone() + self.dynamic_token.url.as_str();
        http_rpc_call(&url, &request, &self.dynamic_token.token_extractor).await
    }
}

pub async fn get_things_password(config: &Config, things_idx: usize) -> String {
    config.get_things_password(things_idx).await
}

fn default_dynamic_token() -> DynamicToken {
    DynamicToken::new()
}

fn default_hashmap() -> HashMap<String, String> {
    HashMap::new()
}

fn default_things_info() -> Vec<ThingsInfo> {
    Vec::new()
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

fn default_think_time() -> i32 {
    30000
}
fn default_user_name() -> String {
    "admin".to_string()
}

fn default_password() -> String {
    "admin".to_string()
}

fn default_random_client_id() -> bool {
    false
}

fn default_topic_suffix() -> String {
    "".to_string()
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

pub fn spec_from_str(contents: &str) -> Result<Stressing> {
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
    use crate::util::render_template;

    static YAML_STR: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: publish
metaData:
  name: task-demo
spec:
  brokerAddr: ["127.0.0.1:1883"]
  clientId: client_id
  dynamicToken: 
    servers:
    - 192.168.1.1
    - 192.168.1.2
    - 192.168.1.3
    url: /v2/things/mqtt/tokens
    payload: '{"username": "${tenantName}", "password": "${password}" }'
    method: POST
    tokenExtractor: "$.data.token"
  userName: admin
  password: bbbb
  topicTemplate: /prefix/${tenantName}/${infoModelName}/${thirdThingsId}
  thinkTime: 5000
  duration: 60
  thingsPayloads:
   "google": "hello world"
  thingsInfo:
  - tenantName: "google"
    infoModelName: "demo_v1"
    thirdThingsId: thirdThingsID
    password: "things_password"
"#;
    static YAML_STR2: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: test
metaData:
  name: task-demo
spec:
  value: 127.0.0.1:1883
"#;

    const YAML_STR3: &str = r#"group: github.com/zhao-kun/mqtt-bench
version: v1.0.1
kind: publish
metaData:
  name: task-demo
spec:
  brokerAddr: ["192.168.24.245:1883"]
  clientId: prefix
  dynamicToken:
    servers:
    - 192.168.1.1
    - 192.168.1.2
    - 192.168.1.3
    url: /v2/things/mqtt/tokens
    method: POST
    payload: '{"devices":[{"devid":"${thirdThingsId}","devtype":"${infoModelName}"}],"password":"${password}","username":"${tenantName}"}'
    tokenExtractor: "$.data.token"
  topicTemplate: /d2s/${tenantName}/${infoModelName}/${thirdThingsId}/data
  thinkTime: 10000
  duration: 60
  thingsPayloads:
    "pressure3": AHRvdGFsX2VuZXJneQAyMC43MQB0b2RheV9lbmVyZ3kANTAuNzQAdGVtcGVyYXR1cmUAOTguNzIAZ2ZjaQA2OS45NgBidXNfdm9sdAA4MC42MQBwb3dlcgAyMC45MQBxX3Bvd2VyADQ1LjMyAHBmADg3LjQyAHB2MV92b2x0ADIwLjEyAHB2MV9jdXJyADMyLjEAcHYyX3ZvbHQAMjAuNzUAcHYyX2N1cnIANzcuMjUAcHYzX3ZvbHQAODkuNwBwdjNfY3VycgA4Ni45NgBsMV92b2x0ADQxLjUyAGwxX2N1cnIAOTIuMTcAbDFfZnJlcQAzMi4xNQBsMV9kY2kAOTAuMjMAbDFfcG93ZXIAOTMuOABsMV9wZgA4LjgAdGltZQAxNjc1MjQwMjY4MjAxAA==
  thingsInfo:
  - tenantName: "pressure3"
    infoModelName: "invert"
    thirdThingsId: "device_invert_3_172"
    password: "12345678"
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
        assert!(config.things_info[0].tenant_name == "google");
        assert!(config.things_info[0].password == "things_password");
        assert!(config.things_info[0].third_things_id == "thirdThingsID");
        assert!(config.things_info[0].info_model_name == "demo_v1");
        assert!(config.topic_template == "/prefix/${tenantName}/${infoModelName}/${thirdThingsId}");
        assert!(config.dynamic_token.url == "/v2/things/mqtt/tokens");
        assert!(
            config.dynamic_token.payload
                == r#"{"username": "${tenantName}", "password": "${password}" }"#
        );
        assert!(config.dynamic_token.method == "POST");
        assert!(config.dynamic_token.token_extractor == "$.data.token");
        assert!(config.dynamic_token.servers.get(0).unwrap() == "192.168.1.1");
        assert!(config.dynamic_token.servers.get(1).unwrap() == "192.168.1.2");
        assert!(config.dynamic_token.servers.get(2).unwrap() == "192.168.1.3");
    }

    #[test]
    fn spec_should_be_unmarshal2() {
        let spec = spec_from_str(YAML_STR2).unwrap();
        println!(" spec is {:?}", spec);
        assert!(spec.group() == "github.com/zhao-kun/mqtt-bench");
        assert!(spec.version() == "v1.0.1");
        assert!(spec.kind() == "test");
    }

    #[test]
    fn spec_shoudl_be_unmarshal3() {
        let spec = spec_from_str(YAML_STR3).unwrap();
        let config = match spec.spec {
            Spec::Publish(publish) => publish,
            _ => panic!("should be publish spec"),
        };
        println!("{}", config.dynamic_token.payload);

        let context = config.things_info[0].to_map();
        let request = render_template(&config.dynamic_token.payload, &context);

        println!("{}", request);

        assert_eq!(
            request,
            r#"{"devices":[{"devid":"device_invert_3_172","devtype":"invert"}],"password":"12345678","username":"pressure3"}"#
        );

        println!("{:?}", config.things_payloads);
        assert!(config.things_payloads.len() > 0);
        assert!(config
            .things_payloads
            .contains_key(&config.things_info[0].tenant_name))
    }
}
