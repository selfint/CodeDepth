use std::{collections::HashSet, env, path::Path, process::Stdio, time::Duration};

use code_depth::lsp::LspClient;
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

    let depths = code_depth::get_function_depths(calls);

    let mut results_json = json!({});

    // find all items with different depths
    for (item_name, item_depths_from_roots) in
        code_depth::build_short_fn_depths(&project_url, &depths)
    {
        // ignore test items
        if test_re.captures(&item_name).is_some() {
            continue;
        }

        let mut depths = HashSet::new();

        let mut non_test_paths = vec![];

        for path in &item_depths_from_roots {
            let mut is_test_path = false;

            // ignore test paths
            for hop in path {
                if test_re.captures(hop).is_some() {
                    is_test_path = true;
                    break;
                }
            }

            if is_test_path {
                continue;
            }

            non_test_paths.push(path);
            depths.insert(path.len());
        }

        if depths.len() > 1 {
            results_json[item_name] = serde_json::to_value(non_test_paths).unwrap();
        }
    }

    println!("{}", serde_json::to_string_pretty(&results_json).unwrap());
}
