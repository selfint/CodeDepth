pub mod json_rpc;

use std::{error::Error, time::Duration};

use lsp_types::{InitializeResult, SymbolInformation, Url, WorkspaceSymbolParams};
use tokio::io::AsyncWriteExt;

use json_rpc::{build_notification, build_request, get_next_response, get_response_result};

pub async fn init<I, O>(
    input: &mut I,
    output: &mut O,
    root_uri: Url,
) -> Result<InitializeResult, Box<dyn Error>>
where
    I: tokio::io::AsyncWrite + std::marker::Unpin,
    O: tokio::io::AsyncRead + std::marker::Unpin,
{
    let initialize_params = lsp_types::InitializeParams {
        root_uri: Some(root_uri),
        ..Default::default()
    };
    let initialize_request = build_request(0, "initialize", &Some(initialize_params));

    input.write_all(&initialize_request).await?;

    let response = get_next_response(output).await?;

    let msg = get_response_result::<InitializeResult>(&response)?
        .response
        .unwrap();

    let initialized_params = lsp_types::InitializedParams {};
    let initialized_request = build_notification("initialized", &Some(initialized_params));
    input.write_all(&initialized_request).await?;

    Ok(msg)
}

pub async fn get_function_definitions<I, O>(
    input: &mut I,
    output: &mut O,
    max_duration: Duration,
) -> Result<Vec<SymbolInformation>, Box<dyn Error>>
where
    I: tokio::io::AsyncWrite + std::marker::Unpin,
    O: tokio::io::AsyncRead + std::marker::Unpin,
{
    let retry_sleep_duration = 100;
    let retry_amount = max_duration.as_millis() / retry_sleep_duration;
    let mut retries_left = retry_amount;

    let params = Some(WorkspaceSymbolParams {
        // for rust-analyzer we need to append '#' to get function definitions
        // this might not be good for all LSP servers
        // TODO: add option to set query string by lsp server, and maybe this is the default?
        query: "#".into(),
        ..Default::default()
    });

    let request = build_request(1, "workspace/symbol", &params);

    input.write_all(&request).await?;
    let response = get_next_response(output).await?;
    let mut result = get_response_result::<Vec<lsp_types::SymbolInformation>>(&response)
        .unwrap()
        .response;

    // wait for server to index project
    // TODO: add 'lsp-server-ready' check instead of this hack
    while let Err(e) = result {
        // make sure the error just means the server is still indexing
        assert_eq!(e.code, -32801, "got unexpected error from lsp server");
        retries_left -= 1;
        if retries_left == 0 {
            return Err(format!("max retries exceeded: {:?}", e).into());
        }

        std::thread::sleep(Duration::from_millis(retry_sleep_duration as u64));

        input.write_all(&request).await?;
        let response = get_next_response(output).await?;
        result = get_response_result::<Vec<lsp_types::SymbolInformation>>(&response)
            .unwrap()
            .response;
    }

    let symbols = result.unwrap();

    let function_definitions = symbols
        .iter()
        .filter(|&s| s.kind == lsp_types::SymbolKind::FUNCTION)
        .map(|s| s.to_owned())
        .collect::<Vec<SymbolInformation>>();

    Ok(function_definitions)
}

#[cfg(test)]
mod tests {}
