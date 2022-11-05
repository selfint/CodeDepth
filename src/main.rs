use std::{env, path::Path, process::Stdio, time::Duration};

use code_depth::lsp_client::LspClient;
use lsp_types::Url;
use tokio::process::Command;

async fn start_lang_server(exe: &str) -> LspClient {
    let server = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rust-analyzer");

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

    let short_depths: Vec<_> = depths
        .iter()
        .map(|(s, t)| {
            (
                format!(
                    "{}:{}",
                    Path::new(s.uri.path())
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    s.name,
                ),
                t.iter()
                    .map(|(r, d)| {
                        (
                            format!(
                                "{}:{}",
                                Path::new(r.uri.path())
                                    .file_name()
                                    .unwrap()
                                    .to_str()
                                    .unwrap(),
                                r.name
                            ),
                            d,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    println!("{:?}", short_depths);
}
