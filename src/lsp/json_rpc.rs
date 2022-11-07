use std::error::Error;

use lsp_types::{notification::Notification, request::Request};
use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt};

pub const JSON_RPC_VERSION: &str = "2.0";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct _Response<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response<T> {
    pub id: Option<usize>,
    pub response: Result<T, JsonRpcError>,
}

impl<T> TryFrom<_Response<T>> for Response<T> {
    type Error = Box<dyn Error>;

    fn try_from(response: _Response<T>) -> Result<Self, Self::Error> {
        if !(response.result.is_some() ^ response.error.is_some()) {
            return Err("got response without result and error".into());
        }

        if let Some(result) = response.result {
            Ok(Self {
                id: response.id,
                response: Ok(result),
            })
        } else {
            Ok(Self {
                id: response.id,
                response: Err(response.error.unwrap()),
            })
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: isize,
    pub message: String,
}

pub fn build_request<R: Request>(id: usize, params: &R::Params) -> Vec<u8> {
    let j = json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": R::METHOD,
            "params": serde_json::to_value(params).expect("failed to serialize params"),
            "id": id,
    });

    let json_str = j.to_string();

    format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str)
        .as_bytes()
        .into()
}

pub fn build_notification<N: Notification>(params: &N::Params) -> Vec<u8> {
    let j = json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": N::METHOD,
            "params": serde_json::to_value(params).expect("failed to serialize params"),
    });

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
    let re = Regex::new(r"Content-Length: (\d+)(?:\r\nContent-Type: [^\r]*)?\r\n\r\n").unwrap();
    let mut buf = vec![];

    // get content-length
    let content_length = loop {
        let byte = reader.read_u8().await?;
        buf.push(byte);
        let text = std::str::from_utf8(&buf)?;

        if let Some(matches) = re.captures(text) {
            let first_match = matches.get(1).ok_or("failed to extract content-length")?;
            break first_match.as_str().parse::<usize>()?;
        }
    };

    // read the rest of the message
    let mut msg_buf = Vec::with_capacity(content_length);

    if let Err(e) = reader.read_buf(&mut msg_buf).await {
        return Err(Box::new(e));
    }

    while msg_buf.len() < content_length {
        let mut next_buf = Vec::with_capacity(content_length - msg_buf.len());
        if let Err(e) = reader.read_buf(&mut next_buf).await {
            return Err(Box::new(e));
        }

        msg_buf.append(&mut next_buf);
    }

    Ok(msg_buf)
}

pub fn get_response_result<T: DeserializeOwned>(buf: &[u8]) -> Result<Response<T>, Box<dyn Error>> {
    match serde_json::from_slice::<_Response<T>>(buf) {
        Ok(it) => it,
        Err(err) => {
            eprintln!("got error on buf size: {}", buf.len());
            eprintln!(
                "buf {}",
                std::str::from_utf8(buf).unwrap().chars().nth(100).unwrap()
            );
            return Err(Box::new(err));
        }
    }
    .try_into()
}
