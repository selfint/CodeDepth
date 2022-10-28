use lsp_types::{InitializeResult, Url};
use regex::Regex;
use serde_json::{json, Value};
use std::error::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

fn to_json_rpc(msg: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", msg.as_bytes().len(), msg)
        .as_bytes()
        .into()
}

async fn read_lsp_msg<R, M>(reader: &mut R) -> Result<M, Box<dyn Error>>
where
    R: AsyncRead + std::marker::Unpin,
    for<'de> M: serde::de::Deserialize<'de>,
{
    // read content-length: \d+
    let re = Regex::new(r"Content-Length: (\d+)\r\n\r\n").unwrap();
    let mut buf = vec![];

    loop {
        // get content-length
        if let Ok(byte) = reader.read_u8().await {
            buf.push(byte);
        };

        let text = std::str::from_utf8(&buf).unwrap();

        let content_length = match re.captures(&text) {
            Some(matches) => match matches.get(1) {
                None => return Err("failed to extract content-length".into()),
                Some(content_length) => match content_length.as_str().parse::<usize>() {
                    Ok(content_length) => Some(content_length),
                    Err(e) => return Err(Box::new(e)),
                },
            },
            None => None,
        };

        if content_length.is_none() {
            continue;
        }

        // read the rest of the message
        let expected_msg_left = content_length.unwrap();
        let mut msg_buf = Vec::with_capacity(expected_msg_left);
        if let Err(e) = reader.read_buf(&mut msg_buf).await {
            return Err(Box::new(e));
        }

        let value: Value = serde_json::from_slice(&msg_buf)?;

        let result = value
            .get("result")
            .ok_or("failed to get result from json response")?;

        // TODO: why do we need to clone here?
        let response = serde_json::from_value::<M>(result.clone())?;

        return Ok(response);
    }
}

pub async fn init(
    server: &mut tokio::process::Child,
    root_uri: Url,
) -> Result<InitializeResult, Box<dyn Error>> {
    let init_msg = lsp_types::InitializeParams {
        root_uri: Some(root_uri),
        ..Default::default()
    };
    let init_msg_str = serde_json::to_value(&init_msg).expect("failed to serialize init message");
    let init_msg_json_rpc = json!({
            "jsonrpc": 2.0,
            "id": 0,
            "method": "initialize",
            "params": init_msg_str
    });
    let init_msg_buf = to_json_rpc(&init_msg_json_rpc.to_string());

    let stdin = server
        .stdin
        .as_mut()
        .ok_or("failed to acquire stdin of server process")?;

    let stdout = server
        .stdout
        .as_mut()
        .ok_or("failed to acquire stdout of server process")?;

    let _stderr = server
        .stderr
        .as_mut()
        .ok_or("failed to acquire stderr of server process")?;

    stdin.write_all(&init_msg_buf).await?;

    let msg: InitializeResult = read_lsp_msg(stdout).await?;

    Ok(msg)
}

#[cfg(test)]
mod tests {}
