mod graph_util;
mod hashable_call_hierarchy_item;
pub mod lsp_client;

use std::collections::HashMap;
use std::{collections::HashSet, error::Error, time::Duration};

use graph_util::get_depths;
use hashable_call_hierarchy_item::HashableCallHierarchyItem;
use lsp_types::{
    request::Request, CallHierarchyItem, InitializeResult, SymbolInformation, Url,
    WorkspaceSymbolParams,
};
use tokio::io::AsyncWriteExt;

// use json_rpc::{build_notification, build_request, get_next_response, get_response_result};

// pub async fn init<I, O>(
//     input: &mut I,
//     output: &mut O,
//     root_uri: Url,
// ) -> Result<InitializeResult, Box<dyn Error>>
// where
//     I: tokio::io::AsyncWrite + std::marker::Unpin,
//     O: tokio::io::AsyncRead + std::marker::Unpin,
// {
//     let initialize_params = lsp_types::InitializeParams {
//         root_uri: Some(root_uri),
//         capabilities: lsp_types::ClientCapabilities {
//             text_document: Some(lsp_types::TextDocumentClientCapabilities {
//                 document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
//                     hierarchical_document_symbol_support: Some(true),
//                     ..Default::default()
//                 }),
//                 ..Default::default()
//             }),
//             ..Default::default()
//         },
//         ..Default::default()
//     };
//     let initialize_request = build_request(0, "initialize", &Some(initialize_params));

//     input.write_all(&initialize_request).await?;

//     let response = get_next_response(output).await?;

//     let msg = get_response_result::<InitializeResult>(&response)?
//         .response
//         .unwrap();

//     let initialized_params = lsp_types::InitializedParams {};
//     let initialized_request = build_notification("initialized", &Some(initialized_params));
//     input.write_all(&initialized_request).await?;

//     Ok(msg)
// }

// pub async fn get_function_definitions<I, O>(
//     input: &mut I,
//     output: &mut O,
//     project_root: &Url,
//     max_duration: Duration,
// ) -> Result<Vec<SymbolInformation>, Box<dyn Error>>
// where
//     I: tokio::io::AsyncWrite + std::marker::Unpin,
//     O: tokio::io::AsyncRead + std::marker::Unpin,
// {
//     let retry_sleep_duration = 100;
//     let retry_amount = max_duration.as_millis() / retry_sleep_duration;
//     let mut retries_left = retry_amount;

//     let params = Some(WorkspaceSymbolParams {
//         // for rust-analyzer we need to append '#' to get function definitions
//         // this might not be good for all LSP servers
//         // TODO: add option to set query string by lsp server, and maybe this is the default?
//         query: "#".into(),
//         ..Default::default()
//     });

//     let request = build_request(1, "workspace/symbol", &params);

//     input.write_all(&request).await?;
//     let response = get_next_response(output).await?;
//     let mut result = get_response_result::<Vec<lsp_types::SymbolInformation>>(&response)
//         .unwrap()
//         .response;

//     // wait for server to index project
//     // TODO: add 'lsp-server-ready' check instead of this hack
//     while let Err(e) = result {
//         // make sure the error just means the server is still indexing
//         assert_eq!(e.code, -32801, "got unexpected error from lsp server");
//         retries_left -= 1;
//         if retries_left == 0 {
//             return Err(format!("max retries exceeded: {:?}", e).into());
//         }

//         std::thread::sleep(Duration::from_millis(retry_sleep_duration as u64));

//         input.write_all(&request).await?;
//         let response = get_next_response(output).await?;
//         result = get_response_result::<Vec<lsp_types::SymbolInformation>>(&response)
//             .unwrap()
//             .response;
//     }

//     let symbols = result.unwrap();

//     let function_definitions = symbols
//         .iter()
//         .filter(|&s| s.kind == lsp_types::SymbolKind::FUNCTION)
//         .filter(|&s| s.location.uri.as_str().starts_with(project_root.as_str()))
//         .map(|s| s.to_owned())
//         .collect::<Vec<SymbolInformation>>();

//     Ok(function_definitions)
// }

// pub async fn get_function_calls<I, O>(
//     input: &mut I,
//     output: &mut O,
//     definitions: &Vec<SymbolInformation>,
//     project_root: &Url,
// ) -> Result<Vec<(CallHierarchyItem, CallHierarchyItem)>, Box<dyn Error>>
// where
//     I: tokio::io::AsyncWrite + std::marker::Unpin,
//     O: tokio::io::AsyncRead + std::marker::Unpin,
// {
//     // get exact location of each definition's name
//     let mut definition_files = HashSet::new();

//     // build CallHierarchyItems from definition symbols
//     for definition in definitions {
//         definition_files.insert(definition.location.uri.clone());
//     }

//     let mut exact_definitions = vec![];

//     let mut request_id = 2;
//     for file in definition_files.iter() {
//         // get file symbols
//         let params = Some(lsp_types::DocumentSymbolParams {
//             text_document: lsp_types::TextDocumentIdentifier { uri: file.clone() },
//             partial_result_params: lsp_types::PartialResultParams::default(),
//             work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
//         });

//         let request = build_request(
//             request_id,
//             lsp_types::request::DocumentSymbolRequest::METHOD,
//             &params,
//         );

//         input.write_all(&request).await?;

//         let response = get_next_response(output).await?;

//         let result = get_response_result::<lsp_types::DocumentSymbolResponse>(&response)?
//             .response
//             .unwrap();

//         match result {
//             // we need DocumentSymbol for the precise location of the function name
//             lsp_types::DocumentSymbolResponse::Flat(_) => return Err("got flat".into()),
//             lsp_types::DocumentSymbolResponse::Nested(symbols) => {
//                 update_exact_definitions(symbols, file, &mut exact_definitions);
//             }
//         }

//         request_id += 1;
//     }

//     let mut calls = vec![];
//     for (file, definition) in exact_definitions {
//         // get definition call hierarchy item
//         let target_item = lsp_types::CallHierarchyItem {
//             name: definition.name,
//             kind: definition.kind,
//             tags: definition.tags,
//             detail: definition.detail,
//             uri: file.clone(),
//             range: definition.range,
//             selection_range: definition.selection_range,
//             data: None,
//         };
//         let params = Some(lsp_types::CallHierarchyIncomingCallsParams {
//             item: target_item.clone(),
//             work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
//             partial_result_params: lsp_types::PartialResultParams::default(),
//         });

//         let request = build_request(
//             request_id,
//             lsp_types::request::CallHierarchyIncomingCalls::METHOD,
//             &params,
//         );

//         input.write_all(&request).await?;

//         let response = get_next_response(output).await?;

//         let result = get_response_result::<Vec<lsp_types::CallHierarchyIncomingCall>>(&response);

//         match result {
//             Ok(result) => match result.response {
//                 Ok(response) => {
//                     for source_item in response {
//                         // filter out calls from outside our project
//                         if source_item
//                             .from
//                             .uri
//                             .as_str()
//                             .starts_with(project_root.as_str())
//                         {
//                             calls.push((source_item.from, target_item.clone()));
//                         }
//                     }
//                 }
//                 Err(e) => {
//                     dbg!(format!(
//                         "got jsonRpcError for {:?}: {:?} {:?}",
//                         (
//                             file.as_str(),
//                             &target_item.name,
//                             &target_item.selection_range.start
//                         ),
//                         e.code,
//                         e.message.chars().take(100).collect::<String>()
//                     ));
//                 }
//             },
//             Err(e) => {
//                 dbg!(format!(
//                     "got error for {:?}: {:?}",
//                     (
//                         file.as_str(),
//                         &target_item.name,
//                         &target_item.selection_range.start
//                     ),
//                     e,
//                 ));
//             }
//         }

//         request_id += 1;
//     }

//     Ok(calls)
// }

// fn update_exact_definitions(
//     symbols: Vec<lsp_types::DocumentSymbol>,
//     file: &Url,
//     exact_definitions: &mut Vec<(Url, lsp_types::DocumentSymbol)>,
// ) {
//     for symbol in symbols {
//         if symbol.kind == lsp_types::SymbolKind::FUNCTION {
//             exact_definitions.push((file.to_owned(), symbol.clone()));
//         }

//         if let Some(children) = symbol.children {
//             update_exact_definitions(children, file, exact_definitions);
//         }
//     }
// }

// pub fn get_function_depths(
//     calls: Vec<(CallHierarchyItem, CallHierarchyItem)>,
// ) -> Vec<(CallHierarchyItem, Vec<(CallHierarchyItem, usize)>)> {
//     let hashable_calls = calls
//         .iter()
//         .map(|(s, t)| (s.clone().into(), t.clone().into()))
//         .collect();

//     let depths_by_root = get_depths(&hashable_calls);

//     let mut depths_from_roots: HashMap<HashableCallHierarchyItem, Vec<(CallHierarchyItem, usize)>> =
//         HashMap::new();

//     for (root, root_depths) in depths_by_root {
//         for (child, depth) in root_depths {
//             depths_from_roots
//                 .entry(child)
//                 .or_insert(vec![])
//                 .push((root.clone().into(), depth));
//         }
//     }

//     // TODO: what is the functional way to implement this (without clone)?
//     let mut r = vec![];
//     for (k, v) in depths_from_roots {
//         r.push((k.into(), v));
//     }

//     r
// }
