use std::{path::Path, process::Stdio, time::Duration};

use lsp_types::Url;
use tokio::process::Command;

use code_depth::{self, lsp_client::LspClient};

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
async fn test_initialize() {
    let mut client = start_std_io_lsp_client();
    let root = get_sample_root();

    code_depth::init(&mut client, root).await.unwrap();

    assert!(true);
}

#[tokio::test]
async fn test_get_function_definitions() {
    let mut client = start_std_io_lsp_client();
    let root = get_sample_root();

    code_depth::init(&mut client, root.clone()).await.unwrap();

    let definitions =
        code_depth::get_function_definitions(&mut client, &root, Duration::from_secs(5))
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

    let expected_function_names = vec![
        "fmt",
        "foo",
        "impl_method",
        "in_foo",
        "main",
        "other_file_method",
    ];

    assert_eq!(function_names, expected_function_names);
}

#[tokio::test]
async fn test_get_function_calls() {
    let mut client = start_std_io_lsp_client();
    let root = get_sample_root();

    code_depth::init(&mut client, root.clone()).await.unwrap();

    let definitions =
        code_depth::get_function_definitions(&mut client, &root, Duration::from_secs(5))
            .await
            .unwrap();

    let calls = code_depth::get_function_calls(&mut client, &definitions, &root)
        .await
        .unwrap();

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
        ]
    );
}

#[tokio::test]
async fn test_get_function_depths() {
    let mut client = start_std_io_lsp_client();
    let root = get_sample_root();

    code_depth::init(&mut client, root.clone()).await.unwrap();

    let definitions =
        code_depth::get_function_definitions(&mut client, &root, Duration::from_secs(5))
            .await
            .unwrap();

    let calls = code_depth::get_function_calls(&mut client, &definitions, &root)
        .await
        .unwrap();

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
