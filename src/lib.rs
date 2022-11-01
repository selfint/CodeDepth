pub mod json_rpc;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::{collections::HashSet, error::Error, time::Duration};

use lsp_types::{
    request::Request, CallHierarchyItem, InitializeResult, SymbolInformation, Url,
    WorkspaceSymbolParams,
};
use petgraph::Graph;
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

pub async fn get_function_calls<I, O>(
    input: &mut I,
    output: &mut O,
    definitions: &Vec<SymbolInformation>,
) -> Result<Vec<(CallHierarchyItem, CallHierarchyItem)>, Box<dyn Error>>
where
    I: tokio::io::AsyncWrite + std::marker::Unpin,
    O: tokio::io::AsyncRead + std::marker::Unpin,
{
    // get exact location of each definition's name
    let mut definition_files = HashSet::new();

    // build CallHierarchyItems from definition symbols
    for definition in definitions {
        definition_files.insert(definition.location.uri.clone());
    }

    let mut exact_definitions = vec![];

    let mut request_id = 2;
    for file in definition_files.iter() {
        // get file symbols
        let params = Some(lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: file.clone() },
            partial_result_params: lsp_types::PartialResultParams::default(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        });

        let request = build_request(
            request_id,
            lsp_types::request::DocumentSymbolRequest::METHOD,
            &params,
        );

        input.write_all(&request).await?;

        let response = get_next_response(output).await?;

        let result = get_response_result::<lsp_types::DocumentSymbolResponse>(&response)?
            .response
            .unwrap();

        match result {
            // we need DocumentSymbol for the precise location of the function name
            lsp_types::DocumentSymbolResponse::Flat(_) => return Err("got flat".into()),
            lsp_types::DocumentSymbolResponse::Nested(symbols) => {
                update_exact_definitions(symbols, file, &mut exact_definitions);
            }
        }

        request_id += 1;
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
        let params = Some(lsp_types::CallHierarchyIncomingCallsParams {
            item: target_item.clone(),
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        });

        let request = build_request(
            request_id,
            lsp_types::request::CallHierarchyIncomingCalls::METHOD,
            &params,
        );

        input.write_all(&request).await?;

        let response = get_next_response(output).await?;

        let result = get_response_result::<Vec<lsp_types::CallHierarchyIncomingCall>>(&response)?
            .response
            .unwrap();

        for source_item in result {
            calls.push((source_item.from, target_item.clone()));
        }

        request_id += 1;
    }

    Ok(calls)
}

fn update_exact_definitions(
    symbols: Vec<lsp_types::DocumentSymbol>,
    file: &Url,
    exact_definitions: &mut Vec<(Url, lsp_types::DocumentSymbol)>,
) {
    for symbol in symbols {
        if symbol.kind == lsp_types::SymbolKind::FUNCTION {
            exact_definitions.push((file.to_owned(), symbol.clone()));
        }

        if let Some(children) = symbol.children {
            update_exact_definitions(children, file, exact_definitions);
        }
    }
}

#[derive(Clone)]
struct HashableCallHierarchyItem(CallHierarchyItem);

impl std::fmt::Debug for HashableCallHierarchyItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("HashableCallHierarchyItem({})", self.0.name))
    }
}

impl Hash for HashableCallHierarchyItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.name.hash(state);
        self.0.uri.hash(state);
        serde_json::to_string(&self.0.selection_range)
            .expect("failed to serialize call item")
            .hash(state);
    }
}

impl PartialEq for HashableCallHierarchyItem {
    fn eq(&self, other: &Self) -> bool {
        let mut s1 = DefaultHasher::new();
        self.hash(&mut s1);
        let h1 = s1.finish();

        let mut s2 = DefaultHasher::new();
        other.hash(&mut s2);
        let h2 = s2.finish();

        h1 == h2
    }
}

impl Eq for HashableCallHierarchyItem {}

impl From<CallHierarchyItem> for HashableCallHierarchyItem {
    fn from(call_hierarchy_item: CallHierarchyItem) -> Self {
        Self(call_hierarchy_item)
    }
}

impl From<HashableCallHierarchyItem> for CallHierarchyItem {
    fn from(hashable_call_hierarchy_item: HashableCallHierarchyItem) -> Self {
        hashable_call_hierarchy_item.0
    }
}

pub fn get_function_depths(
    calls: Vec<(CallHierarchyItem, CallHierarchyItem)>,
) -> Vec<(CallHierarchyItem, Vec<(CallHierarchyItem, usize)>)> {
    let hashable_calls: Vec<(HashableCallHierarchyItem, HashableCallHierarchyItem)> = calls
        .iter()
        .map(|(s, t)| (s.clone().into(), t.clone().into()))
        .collect();

    let depths_by_root = get_depths(&hashable_calls);

    let mut depths_from_roots: HashMap<HashableCallHierarchyItem, Vec<(CallHierarchyItem, usize)>> =
        HashMap::new();

    for (root, root_depths) in depths_by_root {
        for (child, depth) in root_depths {
            depths_from_roots
                .entry(child)
                .or_insert(vec![])
                .push((root.clone().into(), depth));
        }
    }

    // TODO: what is the functional way to implement this (without clone)?
    let mut r = vec![];
    for (k, v) in depths_from_roots {
        r.push((k.into(), v));
    }

    r
}

fn get_depths<T>(edges: &Vec<(T, T)>) -> Vec<(T, Vec<(T, usize)>)>
where
    T: Clone + Hash + Eq + std::fmt::Debug,
{
    // find all roots and execute a bfs from each one to get depths
    // of each node from each root
    let targets = edges.iter().map(|e| e.1.clone()).collect::<HashSet<_>>();
    let mut roots = HashSet::new();
    for (s, _) in edges {
        if !targets.contains(&s) {
            roots.insert(s);
        }
    }

    roots
        .iter()
        .map(|&r| (r.clone(), get_root_depths(r, edges)))
        .collect()
}

fn get_root_depths<T>(root: &T, edges: &Vec<(T, T)>) -> Vec<(T, usize)>
where
    T: Clone + Hash + Eq + std::fmt::Debug,
{
    // build graph
    let mut to_graph_node = HashMap::new();
    let mut graph: Graph<T, ()> = Graph::new();

    for (s, t) in edges {
        if !to_graph_node.contains_key(s) {
            to_graph_node.insert(s, graph.add_node(s.clone()));
        }

        if !to_graph_node.contains_key(t) {
            to_graph_node.insert(t, graph.add_node(t.clone()));
        }

        graph.add_edge(
            *to_graph_node.get(s).unwrap(),
            *to_graph_node.get(t).unwrap(),
            (),
        );
    }

    // run bfs
    let mut graph_depths = vec![];

    let mut roots = vec![*to_graph_node.get(root).unwrap()];
    let mut depth: usize = 0;
    let mut visited = HashSet::new();
    while !roots.is_empty() {
        let mut new_roots = vec![];
        for &r in &roots {
            if !visited.contains(&r) {
                graph_depths.push((r.clone(), depth));
                visited.insert(r);

                new_roots.extend(graph.neighbors(r));
            }
        }

        roots = new_roots;
        depth += 1;
    }

    // convert graph nodes to real nodes
    graph_depths
        .iter()
        .map(|(n, d)| (graph.node_weight(*n).unwrap().clone(), *d))
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use crate::get_depths;

    #[test]
    fn test_get_depths() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (1, 2), (2, 3)])),
            vec![(0, vec![(0, 0), (1, 1), (2, 2), (3, 3),])]
        );
    }

    #[test]
    fn test_get_depths_2_roots() {
        let depths = get_depths(&vec![
            (0, 1), // root 1
            (1, 2),
            (2, 3),
            (10, 11), // root 2
            (11, 12),
            (12, 13),
        ]);

        assert!(depths.contains(&(0, vec![(0, 0), (1, 1), (2, 2), (3, 3),])));
        assert!(depths.contains(&(10, vec![(10, 0), (11, 1), (12, 2), (13, 3),])));
    }

    #[test]
    fn test_get_depths_loop() {
        assert_eq!(
            get_depths(&(vec![(0, 1), (0, 2), (1, 2), (2, 1)])),
            vec![(0, vec![(0, 0), (2, 1), (1, 1),])]
        );
    }

    #[test]
    fn test_top_level_loop() {
        assert_eq!(get_depths(&(vec![(0, 1), (1, 0)])), vec![]);
    }
}
