use std::{path::Path, process::Stdio};

use lsp_types::Url;
use tokio::process::Command;

use code_depth;

#[tokio::test]
async fn connect_to_rust_analyzer() {
    // start a rust-analyzer server inside our project directory
    let mut server = Command::new("rust-analyzer")
        .current_dir("..")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rust-analyzer");

    let project_path = Path::new("..")
        .canonicalize()
        .expect("failed to canonicalize project path");
    let project_path = project_path.as_os_str();

    let project_url =
        Url::from_file_path(project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(&mut server, project_url).await;

    server.kill().await.expect("failed to stop rust-analyzer");

    assert!(response.is_ok());
}
