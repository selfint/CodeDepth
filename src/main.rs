use std::{collections::HashSet, env, path::Path, process::Stdio, time::Duration};

use code_depth::{hashable_call_hierarchy_item::HashableCallHierarchyItem, lsp::LspClient};
use lsp_types::Url;
use regex::Regex;
use serde_json::json;
use tokio::process::Command;

async fn start_lang_server(exe: &str) -> LspClient {
    let parts = exe.split_ascii_whitespace().collect::<Vec<_>>();

    let mut server = Command::new(parts[0]);

    let mut server = server
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if parts.len() > 1 {
        server = server.args(parts.iter().skip(1).collect::<Vec<_>>())
    };

    let server = server.spawn().expect("failed to start rust-analyzer");

    LspClient::stdio_client(server)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let project_path = Path::new(args.get(1).expect("missing argument <project_path>"));
    let lang_server_exe = args.get(2).expect("missing argument <lang_server_exe>");
    let test_re = if let Some(test_str) = args.get(3) {
        Regex::new(test_str).unwrap_or_else(|_| panic!("invalid regex: '{}'", test_str))
    } else {
        Regex::new(".*test.*").unwrap()
    };

    let mut client = start_lang_server(lang_server_exe).await;

    let project_path = project_path.canonicalize().unwrap();

    let project_url =
        Url::from_file_path(project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(&mut client, project_url.clone()).await;

    response.expect("failed to init lang server");

    let workspace_files =
        code_depth::get_workspace_files(&mut client, &project_url, Duration::from_secs(5))
            .await
            .unwrap();

    let calls = code_depth::get_function_calls(&mut client, &workspace_files, &project_url)
        .await
        .unwrap();

    let non_test_calls = calls
        .into_iter()
        .filter(|(to, from)| {
            !(test_re.is_match(&format!(
                "{}:{}",
                to.uri.as_str().trim_start_matches(project_url.as_str()),
                to.name
            )) || test_re.is_match(&format!(
                "{}:{}",
                from.uri.as_str().trim_start_matches(project_url.as_str()),
                from.name
            )))
        })
        .collect::<Vec<_>>();

    let depths = code_depth::get_function_depths(non_test_calls);

    // find all items with different depths
    let items_with_different_depths = depths
        .into_iter()
        .filter(|(item, item_paths_from_roots)| {
            let total_unique_depths = item_paths_from_roots
                .iter()
                .map(|path| path.len())
                .collect::<HashSet<_>>()
                .len();

            let mut all_hops: HashSet<HashableCallHierarchyItem> = HashSet::new();
            let paths_are_unique = item_paths_from_roots.iter().all(|path| {
                path.iter()
                    .filter(|&hop| hop != item)
                    .all(|hop| all_hops.insert(hop.clone().into()))
            });

            total_unique_depths > 1 && paths_are_unique
        })
        .collect::<Vec<_>>();

    let mut results_json = json!({});

    for (item_name, item_depths_from_roots) in
        code_depth::build_short_fn_depths(&project_url, &items_with_different_depths)
    {
        let mut depths = HashSet::new();

        let mut non_test_paths = vec![];

        for path in &item_depths_from_roots {
            non_test_paths.push(path);
            depths.insert(path.len());
        }

        results_json[item_name] = serde_json::to_value(non_test_paths).unwrap();
    }

    println!("{}", serde_json::to_string_pretty(&results_json).unwrap());
}
