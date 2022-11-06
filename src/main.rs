use std::{collections::HashSet, env, path::Path, process::Stdio, time::Duration};

use code_depth::lsp_client::LspClient;
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
        Regex::new(test_str).expect(&format!("invalid regex: '{}'", test_str))
    } else {
        Regex::new(".*test.*").unwrap()
    };

    let mut client = start_lang_server(&lang_server_exe).await;

    let project_path = project_path.canonicalize().unwrap();

    let project_url =
        Url::from_file_path(project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(&mut client, project_url.clone()).await;

    response.expect("failed to init lang server");

    let definitions =
        code_depth::get_function_definitions(&mut client, &project_url, Duration::from_secs(5))
            .await
            .unwrap();

    let calls = code_depth::get_function_calls(&mut client, &definitions, &project_url)
        .await
        .unwrap();

    let depths = code_depth::get_function_depths(calls);

    let mut results_json = json!({});

    // find all items with different depths
    for (item, item_depths_from_roots) in depths {
        let item_name = format!(
            "{}:{}",
            item.uri.as_str().trim_start_matches(&project_url.as_str()),
            item.name
        );

        // ignore test items
        if test_re.captures(&item_name).is_some() {
            continue;
        }

        let mut root_depths = json!({});
        let mut depths = HashSet::new();

        for (root, depth) in item_depths_from_roots {
            let root_name = format!(
                "{}:{}",
                root.uri.as_str().trim_start_matches(&project_url.as_str()),
                root.name
            );

            // ignore test roots
            if test_re.captures(&root_name).is_some() {
                continue;
            }

            root_depths[root_name] = serde_json::to_value(depth).unwrap();
            depths.insert(depth);
        }

        if depths.len() > 1 {
            results_json[item_name] = serde_json::to_value(&root_depths).unwrap();
        }
    }

    println!("{}", serde_json::to_string_pretty(&results_json).unwrap());
}
