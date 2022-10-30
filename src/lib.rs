pub mod json_rpc;

use std::{collections::HashSet, error::Error, hash::Hash, time::Duration};

use lsp_types::{InitializeResult, SymbolInformation, Url, WorkspaceSymbolParams};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use json_rpc::{build_notification, build_request, get_next_response, get_response_result};

const ABC: &str = "abcdefghijklmnopqrstuvwxyz";

pub async fn init<R, W>(
    reader: &mut R,
    writer: &mut W,
    root_uri: Url,
) -> Result<InitializeResult, Box<dyn Error>>
where
    R: tokio::io::AsyncWrite + std::marker::Unpin,
    W: tokio::io::AsyncRead + std::marker::Unpin,
{
    let initialize_params = lsp_types::InitializeParams {
        root_uri: Some(root_uri),
        capabilities: lsp_types::ClientCapabilities {
            workspace: Some(lsp_types::WorkspaceClientCapabilities {
                symbol: Some(lsp_types::WorkspaceSymbolClientCapabilities {
                    symbol_kind: Some(lsp_types::SymbolKindCapability {
                        value_set: Some(vec![lsp_types::SymbolKind::FUNCTION]),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let initialize_request = build_request(0, "initialize", &Some(initialize_params));

    reader.write_all(&initialize_request).await?;

    let response = get_next_response(writer).await?;

    let msg = get_response_result::<InitializeResult>(&response)?;

    let initialized_params = lsp_types::InitializedParams {};
    let initialized_request = build_notification("initialized", &Some(initialized_params));
    reader.write_all(&initialized_request).await?;

    Ok(msg)
}

pub async fn get_function_definitions<R, W>(
    reader: &mut R,
    writer: &mut W,
) -> Result<Vec<SymbolInformation>, Box<dyn Error>>
where
    R: tokio::io::AsyncWrite + std::marker::Unpin,
    W: tokio::io::AsyncRead + std::marker::Unpin,
{
    // iterate over abc to get all symbols
    // TODO: this is stupid, there must be a better way

    let mut symbols = HashSet::new();

    for letter in ABC.chars() {
        dbg!(letter);
        let params = Some(WorkspaceSymbolParams {
            query: letter.into(),
            ..Default::default()
        });

        let request = build_request(1, "workspace/symbol", &params);

        dbg!(std::str::from_utf8(&request));
        std::thread::sleep(Duration::from_millis(1000));

        reader.write_all(&request).await?;

        dbg!("sent");

        let response = get_next_response(writer).await?;

        dbg!(std::str::from_utf8(&response));

        let result = get_response_result::<Vec<lsp_types::SymbolInformation>>(&response).unwrap();

        let letter_symbols = result
            .iter()
            .filter(|&s| s.kind == lsp_types::SymbolKind::FUNCTION)
            .map(|s| s.to_owned())
            .collect::<Vec<SymbolInformation>>();

        dbg!(&letter_symbols);

        for symbol in letter_symbols {
            symbols.insert(HashableSymbolInformation(symbol));
        }
    }

    let symbols = symbols.iter().map(|s| s.0.to_owned()).collect();

    Ok(symbols)
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct HashableSymbolInformation(SymbolInformation);

impl Hash for HashableSymbolInformation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        serde_json::to_string(&self.0).unwrap().hash(state);
    }
}

#[cfg(test)]
mod tests {}
