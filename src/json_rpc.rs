use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

pub const JSON_RPC_VERSION: f32 = 2.0;

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

#[derive(Debug, PartialEq, Clone, Default, Deserialize)]
struct Response<T> {
    pub id: usize,
    pub jsonrpc: String,
    pub result: T,
}

pub fn get_response_result<T: for<'de> Deserialize<'de>>(buf: &[u8]) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_slice::<Response<T>>(&buf)?.result)
}
