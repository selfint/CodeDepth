use std::{path::Path, process::Stdio, time::Duration};

use lsp_types::Url;
use tokio::process::Command;

use code_depth::{self, lsp::LspClient};

const SAMPLE_PROJECT_PATH: &str = "tests/rust_analyzer/sample_rust_project";

fn start_std_io_lsp_client() -> LspClient {
    let server = Command::new("rust-analyzer")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start rust-analyzer");

    LspClient::stdio_client(server)
}

fn get_sample_root() -> Url {
    let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

    Url::from_file_path(sample_project_path).expect("failed to convert project path to URL")
}

#[tokio::test]
async fn test_lsp_client() {
    let mut client = start_std_io_lsp_client();
    let root = get_sample_root();

    code_depth::init(&mut client, root.clone())
        .await
        .expect("init failed");

    let definitions =
        code_depth::get_function_definitions(&mut client, &root, Duration::from_secs(5))
            .await
            .expect("get_function_definitions failed");

    let calls = code_depth::get_function_calls(&mut client, &definitions, &root)
        .await
        .expect("get_function_calls failed");

    let mut short_calls: Vec<String> = calls
        .iter()
        .map(|(s, t)| {
            format!(
                "{}:{}->{}:{}",
                Path::new(s.uri.path())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                s.name,
                Path::new(t.uri.path())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                t.name
            )
        })
        .collect();

    short_calls.sort();

    assert_eq!(
        short_calls,
        vec![
            "main.rs:foo->main.rs:in_foo",
            "main.rs:impl_method->other_file.rs:other_file_method",
            "main.rs:in_foo->main.rs:impl_method",
            "main.rs:main->main.rs:foo",
            "main.rs:main->main.rs:impl_method",
        ],
        "didn't find all function calls"
    );

    let depths = code_depth::get_function_depths(calls);

    let short_item_depths = code_depth::build_short_fn_depths(&root, &depths);

    assert!(short_item_depths.contains(&(
        "/src/other_file.rs:other_file_method".into(),
        vec![vec![
            "/src/main.rs:main".into(),
            "/src/main.rs:impl_method".into(),
            "/src/other_file.rs:other_file_method".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main.rs:in_foo".into(),
        vec![vec![
            "/src/main.rs:main".into(),
            "/src/main.rs:foo".into(),
            "/src/main.rs:in_foo".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main.rs:impl_method".into(),
        vec![vec![
            "/src/main.rs:main".into(),
            "/src/main.rs:impl_method".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main.rs:foo".into(),
        vec![vec!["/src/main.rs:main".into(), "/src/main.rs:foo".into(),],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main.rs:main".into(),
        vec![vec!["/src/main.rs:main".into(),],],
    )));
}
