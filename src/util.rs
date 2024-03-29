use std::collections::HashMap;
use std::sync::Arc;
use text_template::*;
#[derive(Clone)]
pub struct MyClient {
    pub client: reqwest::Client,
}

impl MyClient {
    pub fn new() -> Self {
        let client = reqwest::Client::new();
        MyClient { client }
    }
}
pub fn render_template(template: &str, context: &HashMap<&str, &str>) -> String {
    let template = Template::from(template);
    let text = template.fill_in(context);
    return text.to_string();
}

pub async fn http_rpc_call(
    http_client: &Arc<MyClient>,
    http_url: &str,
    request: &str,
    extractor: &str,
) -> String {
    let url = reqwest::Url::parse(http_url).unwrap();

    let res = http_client
        .client
        .post(url)
        .body(request.to_owned().to_string())
        .header("Content-type", "application/json")
        .send()
        .await;
    match res {
        Ok(response) => {
            if response.status() != reqwest::StatusCode::OK {
                println!(
                    "request url: {:?} with body: {:?}, error: {:?}, message: {:?}",
                    http_url,
                    request,
                    response.status(),
                    response.text().await.unwrap()
                );
                return "".to_string();
            }
            let result: String = response.text().await.unwrap();
            extract_token(&result, extractor)
        }
        Err(err) => {
            println!(
                "request url: {:?} with body: {:?}, error: {:?}",
                http_url, request, err
            );
            return "".to_string();
        }
    }
}

fn extract_token(content: &str, token_extractor: &str) -> String {
    jsonpath_rust::JsonPathFinder::from_str(content, token_extractor)
        .unwrap()
        .find()
        .as_array()
        .unwrap()
        .get(0)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[cfg(test)]
mod util_tests {

    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::util::extract_token;
    use crate::util::MyClient;

    use super::http_rpc_call;

    const RESPONSE: &str = r#"{
  "data": {
    "clientip": "string",
    "mqtt": {
      "host": "string",
      "port": 0,
      "sslport": 0
    },
    "owner": "string",
    "token": "this is a real token",
    "uuid": "string"
  },
  "message": "string",
  "result": true
}"#;
    const REQUEST: &str = r#"{
  "devices": [
    {
      "devid": "string",
      "devtype": "string"
    }
  ],
  "password": "string",
  "username": "string"
}"#;

    const TOKEN_EXTRACTOR: &str = ".data.token";

    #[test]
    fn test_extractor() {
        let token = extract_token(RESPONSE, TOKEN_EXTRACTOR);
        println!("token is {:?}", token);
        assert!(token == "this is a real token")
    }

    const PATH: &str = "/v2/things/mqtt/tokens";
    #[tokio::test]
    async fn test_http_rpc() {
        // Start a lightweight mock server.
        let server = MockServer::start().await;
        let http_client = Arc::new(MyClient::new());

        Mock::given(method("POST"))
            .and(path(PATH))
            .respond_with(ResponseTemplate::new(200).set_body_raw(RESPONSE, "application/json"))
            .mount(&server)
            .await;

        // Create a mock on the server.
        let url = format!("{}{}", server.uri(), PATH);

        println!("make a http request to mockserver {}", url);

        let result = http_rpc_call(&http_client, &url, REQUEST, TOKEN_EXTRACTOR).await;

        assert_eq!(result, "this is a real token");
    }
}
