mod json_rpc;

use lsp_types::{InitializeResult, Url};
use std::error::Error;
use tokio::io::AsyncWriteExt;

use json_rpc::{build_request, get_next_response, get_response_result};

pub async fn init(
    server: &mut tokio::process::Child,
    root_uri: Url,
) -> Result<InitializeResult, Box<dyn Error>> {
    let stdin = server
        .stdin
        .as_mut()
        .ok_or("failed to acquire stdin of server process")?;

    let stdout = server
        .stdout
        .as_mut()
        .ok_or("failed to acquire stdout of server process")?;

    let init_params = lsp_types::InitializeParams {
        root_uri: Some(root_uri),
        ..Default::default()
    };
    let init_request = build_request(0, "initialize", &Some(init_params));

    stdin.write_all(&init_request).await?;

    let response = get_next_response(stdout).await?;

    let msg = get_response_result(&response)?;

    Ok(msg)
}

#[cfg(test)]
mod tests {}
