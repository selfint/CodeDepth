use std::{path::Path, process::Stdio, time::Duration};

use lsp_types::Url;
use tokio::process::Command;

use code_depth::{self, lsp::LspClient};

const SAMPLE_PROJECT_PATH: &str = "tests/jdtls/sample_java_project";

fn start_std_io_lsp_client() -> LspClient {
    let metadata_dir = std::env::temp_dir().join(".metadata");
    let data_dir = metadata_dir.to_str().unwrap();

    let config_dir = std::env::temp_dir().join("jdt.ls-java-project");
    let config_dir = config_dir.to_str().unwrap();

    let server = Command::new("jdtls")
        .arg("-data")
        .arg(data_dir)
        .arg("-configuration")
        .arg(config_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start jdtls");

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

    let workspace_files =
        code_depth::get_workspace_files(&mut client, &root, Duration::from_secs(5))
            .await
            .expect("get_function_definitions failed");

    let calls = code_depth::get_function_calls(&mut client, &workspace_files, &root)
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
                s.name.split('(').next().unwrap(),
                Path::new(t.uri.path())
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                t.name.split('(').next().unwrap()
            )
        })
        .collect();

    short_calls.sort();

    assert_eq!(
        short_calls,
        vec![
            "App.java:foo->App.java:method",
            "App.java:main->App.java:foo",
            "App.java:main->App.java:method",
            "App.java:method->OtherFile.java:otherFileMethod",
        ],
        "didn't find all function calls"
    );

    let depths = code_depth::get_function_depths(calls);

    let short_item_depths = code_depth::build_short_fn_depths(&root, &depths);

    assert!(short_item_depths.contains(&(
        "/src/main/java/sample/OtherFile.java:otherFileMethod".into(),
        vec![vec![
            "/src/main/java/sample/App.java:main".into(),
            "/src/main/java/sample/App.java:method".into(),
            "/src/main/java/sample/OtherFile.java:otherFileMethod".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main/java/sample/App.java:method".into(),
        vec![vec![
            "/src/main/java/sample/App.java:main".into(),
            "/src/main/java/sample/App.java:method".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main/java/sample/App.java:foo".into(),
        vec![vec![
            "/src/main/java/sample/App.java:main".into(),
            "/src/main/java/sample/App.java:foo".into(),
        ],],
    )));
    assert!(short_item_depths.contains(&(
        "/src/main/java/sample/App.java:main".into(),
        vec![vec!["/src/main/java/sample/App.java:main".into(),],],
    )));
}
