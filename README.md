
### Introduction

mqtt-bench is a tool for benchmarking your MQTT server  

### Build

mqtt-bench is written in RUST, you can build target with

```bash
cargo build
```
or 
```bash
cargo build --release
```

### Usage

You can start a benchmark via running the target with a config file:

```
mqtt-bench -f config.yaml
```

### Configuration

The example config file 

```yaml
group: github.com/zhao-kun/mqtt-bench # Fix value for the future extension
version: v1.0.1 # Fix value, current is v1.0.1
kind: publish # Fix value, current only support publish, for future we can support subscribe, etc....
metaData:
  name: task-demo # benchmarking task name
spec:
  brokerAddr: ["127.0.0.1:1883"] # brokers' address of the the MQTT server
  clientId: client_id # client_id a prefix of the client id, each connection will append a random string to it
  connection: 100 # concurrent things, each connection represented a things
  userName: admin # credentials for MQTT server
  password: bbbb # credentials for MQTT server
  payload: "hello world" # the payload will be published to mqtt server
  thinkTime: 5000 # the duration between two action (sent packet to mqtt server) of a single things
  duration: 60 # The duration of the benchmarking
  topicTemplate: "/${tenantName}/${infoModelId}/${thirdThingsId}/raw" # topic template, evaluated with the `data` section
  data: # attending to evaluating the topicTemplate
    tenantName: "google"
    infoModelId: "demo_v1"
    thirdThingsId: thirdThingsID
```
