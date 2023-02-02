use std::collections::HashMap;
use text_template::*;

pub fn render_template(template: &str, context: &HashMap<&str, &str>) -> String {
    let template = Template::from(template);
    let text = template.fill_in(context);
    return text.to_string();
}

pub async fn http_rpc_call(http_url: &str, request: &str, extractor: &str) -> String {
    let url = reqwest::Url::parse(http_url).unwrap();
    let response: reqwest::Response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!(request))
        .send()
        .await
        .unwrap();
    if response.status() != reqwest::StatusCode::OK {
        panic!(
            "request url: {:?} with body: {:?}, error: {:?}",
            http_url,
            request,
            response.status()
        );
    }
    let result: String = response.text().await.unwrap();
    extract_token(&result, extractor)
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
    use crate::util::extract_token;
    use httpmock::prelude::*;

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
    "token": "token2",
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

    const TOKEN_EXTRACTOR: &str = "$.data.token";

    #[test]
    fn test_extractor() {
        let token = extract_token(RESPONSE, TOKEN_EXTRACTOR);
        println!("token is {:?}", token);
        assert!(token == "token2")
    }

    const PATH: &str = "/v2/things/mqtt/tokens";
    #[test]
    fn test_http_rpc() {
        // Start a lightweight mock server.
        let server = MockServer::start();

        // Create a mock on the server.
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path(PATH)
                .json_body(serde_json::json!(REQUEST));

            then.status(200)
                .header("content-type", "applicatoin/json; charset=UTF-8")
                .json_body(serde_json::json!(RESPONSE));
        });

        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async { http_rpc_call(&server.url(PATH), REQUEST, TOKEN_EXTRACTOR).await });

        mock.assert();

        assert_eq!(result, "token2");
    }
}
