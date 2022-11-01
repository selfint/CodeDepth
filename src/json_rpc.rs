use std::error::Error;

use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt};

pub const JSON_RPC_VERSION: f32 = 2.0;

#[derive(Debug, Clone, Default, Deserialize)]
struct _Response<T> {
    pub id: usize,
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response<T> {
    pub id: usize,
    pub response: Result<T, JsonRpcError>,
}

impl<T> From<_Response<T>> for Response<T> {
    fn from(response: _Response<T>) -> Self {
        assert!(response.result.is_some() ^ response.error.is_some());

        if let Some(result) = response.result {
            Self {
                id: response.id,
                response: Ok(result),
            }
        } else {
            Self {
                id: response.id,
                response: Err(response.error.unwrap()),
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: isize,
    pub message: String,
}

pub fn build_request<T: Serialize>(id: usize, method: &str, params: &Option<T>) -> Vec<u8> {
    let mut j = json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "method": method,
    });

    if let Some(params) = params {
        j["params"] = serde_json::to_value(params).expect("failed to serialize params");
    }

    let json_str = j.to_string();

    format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str)
        .as_bytes()
        .into()
}

pub fn build_notification<T: Serialize>(method: &str, params: &Option<T>) -> Vec<u8> {
    let mut j = json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": method,
    });

    if let Some(params) = params {
        j["params"] = serde_json::to_value(params).expect("failed to serialize params");
    }

    let json_str = j.to_string();

    format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str)
        .as_bytes()
        .into()
}

pub async fn get_next_response<R>(reader: &mut R) -> Result<Vec<u8>, Box<dyn Error>>
where
    R: AsyncRead + std::marker::Unpin,
{
    // match content-length: \d+
    // and also match the separating \r\n\r\n to the actual content
    // so that when we read the message we just read 'content-length' bytes
    let re = Regex::new(r"Content-Length: (\d+)\r\n\r\n").unwrap();
    let mut buf = vec![];

    // get content-length
    let content_length = loop {
        let byte = match reader.read_u8().await {
            Ok(byte) => byte,
            Err(_) => continue,
        };

        buf.push(byte);
        let text = std::str::from_utf8(&buf)?;

        if let Some(matches) = re.captures(&text) {
            let first_match = matches.get(1).ok_or("failed to extract content-length")?;
            break first_match.as_str().parse::<usize>()?;
        }
    };

    // read the rest of the message
    let mut msg_buf = Vec::with_capacity(content_length);
    if let Err(e) = reader.read_buf(&mut msg_buf).await {
        return Err(Box::new(e));
    }

    Ok(msg_buf)
}

pub fn get_response_result<T: DeserializeOwned>(buf: &[u8]) -> Result<Response<T>, Box<dyn Error>> {
    Ok(match serde_json::from_slice::<_Response<T>>(&buf) {
        Ok(it) => it,
        Err(err) => {
            return Err(format!("got err: {} on buf: {:?}", err, std::str::from_utf8(&buf)).into())
        }
    }
    .into())
}
