use std::{path::Path, process::Stdio};

use lsp_types::Url;
use tokio::{io::AsyncReadExt, process::Command};

use code_depth;

#[tokio::test]
async fn test_initialize() {
    // start a rust-analyzer server inside our project directory
    let sample_rust_project_path = Path::new(file!())
        .parent()
        .unwrap()
        .join("sample_rust_project");
    let sample_rust_project_path = sample_rust_project_path.as_os_str();

    let mut server = Command::new("rust-analyzer")
        .current_dir(sample_rust_project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rust-analyzer");

    let stdin = server
        .stdin
        .as_mut()
        .take()
        .expect("failed to acquire stdin of server process");

    let stdout = server
        .stdout
        .as_mut()
        .take()
        .expect("failed to acquire stdout of server process");

    let stderr = server
        .stderr
        .as_mut()
        .take()
        .expect("failed to acquire stderr of server process");

    let project_path = Path::new("..")
        .canonicalize()
        .expect("failed to canonicalize project path");
    let project_path = project_path.as_os_str();

    let project_url =
        Url::from_file_path(project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(stdin, stdout, project_url).await;

    // fail if response is err, but with nice debug info
    response.unwrap();

    server.kill().await.expect("failed to stop rust-analyzer");

    assert!(true);
}
