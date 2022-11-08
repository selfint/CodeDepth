mod graph_util;
pub mod hashable_call_hierarchy_item;
pub mod lsp;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};

use lsp_types::{
    CallHierarchyItem, InitializeResult, SymbolInformation, SymbolKind, Url, WorkspaceSymbolParams,
};

use graph_util::get_depths;
use hashable_call_hierarchy_item::HashableCallHierarchyItem;
use lsp::{json_rpc::JsonRpcError, LspClient};

pub async fn init(client: &mut LspClient, root_uri: Url) -> Result<InitializeResult, JsonRpcError> {
    let params = lsp_types::InitializeParams {
        root_uri: Some(root_uri),
        capabilities: lsp_types::ClientCapabilities {
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                    hierarchical_document_symbol_support: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = client.initialize(&params).await;

    // make sure server has the desired capabilities
    if let Ok(result) = &result {
        let mut required_methods = HashSet::new();

        for required_method in [
            "workspace/symbol",
            "textDocument/documentSymbol",
            "callHierarchy/incomingCalls",
        ] {
            required_methods.insert(required_method);
        }

        let mut supported_methods = HashSet::new();

        if match &result.capabilities.workspace_symbol_provider {
            Some(provider) => match provider {
                lsp_types::OneOf::Left(enabled) => *enabled,
                lsp_types::OneOf::Right(_) => true,
            },
            None => false,
        } {
            supported_methods.insert("workspace/symbol");
        }

        if match &result.capabilities.document_symbol_provider {
            Some(provider) => match provider {
                lsp_types::OneOf::Left(enabled) => *enabled,
                lsp_types::OneOf::Right(_) => true,
            },
            None => false,
        } {
            supported_methods.insert("textDocument/documentSymbol");
        }

        if match &result.capabilities.call_hierarchy_provider {
            Some(provider) => match provider {
                lsp_types::CallHierarchyServerCapability::Simple(enabled) => *enabled,
                lsp_types::CallHierarchyServerCapability::Options(_) => true,
            },
            None => false,
        } {
            supported_methods.insert("callHierarchy/incomingCalls");
        }

        assert_eq!(
            required_methods,
            supported_methods,
            "missing support for required methods {:?}",
            required_methods.difference(&supported_methods)
        );
    }

    result
}

pub async fn get_workspace_files(
    client: &mut lsp::LspClient,
    project_root: &Url,
    max_duration: Duration,
) -> Result<HashSet<Url>, Box<dyn Error>> {
    let retry_sleep_duration = 100;
    let retry_amount = max_duration.as_millis() / retry_sleep_duration;
    let mut retries_left = retry_amount;

    let params = WorkspaceSymbolParams {
        // for rust-analyzer we need to append '#' to get function definitions
        // this might not be good for all LSP servers
        // TODO: add option to set query string by lsp server, and maybe this is the default?
        query: "#".into(),
        ..Default::default()
    };

    let mut symbols: Vec<SymbolInformation> = vec![];

    let mut result = client.workspace_symbol(&params).await;

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

        result = client.workspace_symbol(&params).await;
    }

    // check if empty query works
    if let Ok(Some(result)) = &mut result {
        symbols.append(result);
    }

    // try empty query strategy
    let params = WorkspaceSymbolParams {
        query: "".into(),
        ..Default::default()
    };

    let mut result = client.workspace_symbol(&params).await;

    if let Ok(Some(result)) = &mut result {
        symbols.append(result);
    }

    // try letter by letter strategy
    for letter in "abcdefghijklmnopqrstuvwxyz".chars() {
        let params = WorkspaceSymbolParams {
            query: letter.into(),
            ..Default::default()
        };

        let mut result = client.workspace_symbol(&params).await;

        if let Ok(Some(result)) = &mut result {
            symbols.append(result);
        }
    }

    let mut workspace_files = HashSet::new();

    let project_root_str = project_root.as_str();
    for symbol in symbols {
        let symbol_file = symbol.location.uri;
        if symbol_file.as_str().starts_with(project_root_str) {
            workspace_files.insert(symbol_file);
        }
    }

    Ok(workspace_files)
}

pub async fn get_function_calls(
    client: &mut LspClient,
    workspace_files: &HashSet<Url>,
    project_root: &Url,
) -> Result<Vec<(CallHierarchyItem, CallHierarchyItem)>, Box<dyn Error>> {
    // get exact location of each definition's name
    let mut exact_definitions = vec![];

    for file in workspace_files.iter() {
        // get file symbols
        let params = lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: file.clone() },
            partial_result_params: lsp_types::PartialResultParams::default(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };

        let result = client.document_symbol(&params).await.unwrap().unwrap();

        match result {
            // we need DocumentSymbol for the precise location of the function name
            lsp_types::DocumentSymbolResponse::Flat(_) => return Err("got flat".into()),
            lsp_types::DocumentSymbolResponse::Nested(symbols) => {
                update_exact_definitions(symbols, file, &mut exact_definitions);
            }
        }
    }

    let mut calls = vec![];
    for (file, definition) in exact_definitions {
        // get definition call hierarchy item
        let target_item = lsp_types::CallHierarchyItem {
            name: definition.name,
            kind: definition.kind,
            tags: definition.tags,
            detail: definition.detail,
            uri: file.clone(),
            range: definition.range,
            selection_range: definition.selection_range,
            data: None,
        };

        let params = lsp_types::CallHierarchyIncomingCallsParams {
            item: target_item.clone(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let result = client.call_hierarchy_incoming_calls(&params).await;

        match result {
            Ok(result) => {
                if let Some(response) = result {
                    for source_item in response {
                        // filter out calls from outside our project
                        if source_item
                            .from
                            .uri
                            .as_str()
                            .starts_with(project_root.as_str())
                        {
                            calls.push((source_item.from, target_item.clone()));
                        }
                    }
                }
            }
            Err(e) => {
                dbg!(format!(
                    "got jsonRpcError for {:?}: {:?} {:?}",
                    (
                        file.as_str(),
                        &target_item.name,
                        &target_item.selection_range.start
                    ),
                    e.code,
                    e.message
                ));
            }
        }
    }

    Ok(calls)
}

fn update_exact_definitions(
    symbols: Vec<lsp_types::DocumentSymbol>,
    file: &Url,
    exact_definitions: &mut Vec<(Url, lsp_types::DocumentSymbol)>,
) {
    for symbol in symbols {
        if symbol.kind == SymbolKind::FUNCTION || symbol.kind == SymbolKind::METHOD {
            exact_definitions.push((file.to_owned(), symbol.clone()));
        }

        if let Some(children) = symbol.children {
            update_exact_definitions(children, file, exact_definitions);
        }
    }
}

pub fn get_function_depths(
    calls: Vec<(CallHierarchyItem, CallHierarchyItem)>,
) -> Vec<(CallHierarchyItem, Vec<Vec<CallHierarchyItem>>)> {
    // convert call items into hashable call items
    let hashable_calls = calls
        .iter()
        .map(|(s, t)| (s.clone().into(), t.clone().into()))
        .collect::<Vec<(HashableCallHierarchyItem, HashableCallHierarchyItem)>>();

    let depths_by_root = get_depths(&hashable_calls);

    // get item paths from each root
    let mut item_paths_from_roots = HashMap::new();
    for (_, items) in depths_by_root {
        for (item, item_path) in items {
            let item_path_from_root: &mut Vec<Vec<CallHierarchyItem>> =
                item_paths_from_roots.entry(item).or_default();

            let mut converted_item_path: Vec<CallHierarchyItem> = vec![];
            for hop in item_path {
                converted_item_path.push(hop.into());
            }

            item_path_from_root.push(converted_item_path);
        }
    }

    // TODO: what is the functional way to implement this (without clone)?
    let mut r = vec![];
    for (k, v) in item_paths_from_roots {
        r.push((k.into(), v));
    }

    r
}

pub fn build_short_fn_depths(
    root: &Url,
    depths: &Vec<(CallHierarchyItem, Vec<Vec<CallHierarchyItem>>)>,
) -> Vec<(String, Vec<Vec<String>>)> {
    let mut short_item_depths = vec![];

    for (item, paths_from_roots) in depths {
        let item_name = format!(
            "{}:{}",
            item.uri.as_str().trim_start_matches(root.as_str()),
            item.name.split('(').next().unwrap()
        );

        let mut short_paths = vec![];
        for path in paths_from_roots {
            let mut short_path = vec![];
            for hop in path {
                let hop_name = format!(
                    "{}:{}",
                    hop.uri.as_str().trim_start_matches(root.as_str()),
                    hop.name.split('(').next().unwrap()
                );
                short_path.push(hop_name);
            }

            short_paths.push(short_path);
        }

        short_item_depths.push((item_name, short_paths));
    }

    short_item_depths
}
