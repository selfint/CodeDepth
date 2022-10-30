use std::{path::Path, process::Stdio, time::Duration};

use lsp_types::Url;
use tokio::process::{Child, Command};

use code_depth;

const SAMPLE_PROJECT_PATH: &str = "tests/rust_analyzer/sample_rust_project";

async fn start_rust_analyzer() -> Child {
    Command::new("rust-analyzer")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rust-analyzer")
}

#[tokio::test]
async fn test_initialize() {
    let mut server = start_rust_analyzer().await;

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

    let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

    let project_url =
        Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(stdin, stdout, project_url).await;

    // fail if response is err, but with nice debug info
    response.unwrap();

    server.kill().await.expect("failed to stop rust-analyzer");

    assert!(true);
}

#[tokio::test]
async fn test_get_function_definitions() {
    let mut server = start_rust_analyzer().await;

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

    let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

    let project_url =
        Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(stdin, stdout, project_url).await;

    // fail if response is err, but with nice debug info
    response.unwrap();

    let definitions = code_depth::get_function_definitions(stdin, stdout, Duration::from_secs(5))
        .await
        .unwrap();

    for definition in &definitions {
        assert_eq!(
            definition.kind,
            lsp_types::SymbolKind::FUNCTION,
            "got non-function symbol"
        );
    }

    let mut function_names = definitions
        .iter()
        .map(|s| s.name.clone())
        .collect::<Vec<String>>();

    function_names.sort();

    let expected_function_names = vec!["foo", "impl_method", "in_foo", "main", "other_file_method"];

    assert_eq!(function_names, expected_function_names);

    server.kill().await.expect("failed to stop rust-analyzer");
}
