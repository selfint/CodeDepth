use std::{env, path::Path, process::Stdio, time::Duration};

use code_depth::lsp_client::LspClient;
use lsp_types::Url;
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

    for (item, item_depths_from_roots) in depths {
        let item_name = format!("{}:{}", item.uri.as_str(), item.name);
        let mut root_depths = vec![];

        for (root, depth) in item_depths_from_roots {
            let root_name = format!("{}:{}", root.uri.as_str(), root.name);
            root_depths.push((root_name, depth));
        }

        results_json[item_name] = serde_json::to_value(&root_depths).unwrap();
    }

    println!("{}", serde_json::to_string_pretty(&results_json).unwrap());
}
