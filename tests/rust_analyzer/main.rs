use std::{path::Path, process::Stdio, time::Duration};

use lsp_types::{InitializeParams, Url};
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
    let server = start_rust_analyzer().await;

    let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

    let root =
        Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

    let mut client = code_depth::lsp_client::lsp_client::StdIOLspClient::new(server, root);

    let response = client.initialize(&Some(InitializeParams::default())).await;

    // fail if response is err, but with nice debug info
    response.unwrap();

    assert!(true);
}

// #[tokio::test]
// async fn test_get_function_definitions() {
//     let mut server = start_rust_analyzer().await;

//     let stdin = server
//         .stdin
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdin of server process");

//     let stdout = server
//         .stdout
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdout of server process");

//     let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

//     let project_url =
//         Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

//     let response = code_depth::init(stdin, stdout, project_url.clone()).await;

//     // fail if response is err, but with nice debug info
//     response.unwrap();

//     let definitions =
//         code_depth::get_function_definitions(stdin, stdout, &project_url, Duration::from_secs(5))
//             .await
//             .unwrap();

//     for definition in &definitions {
//         assert_eq!(
//             definition.kind,
//             lsp_types::SymbolKind::FUNCTION,
//             "got non-function symbol"
//         );
//     }

//     let mut function_names = definitions
//         .iter()
//         .map(|s| s.name.clone())
//         .collect::<Vec<String>>();

//     function_names.sort();

//     let expected_function_names = vec![
//         "fmt",
//         "foo",
//         "impl_method",
//         "in_foo",
//         "main",
//         "other_file_method",
//     ];

//     assert_eq!(function_names, expected_function_names);

//     server.kill().await.expect("failed to stop rust-analyzer");
// }

// #[tokio::test]
// async fn test_get_function_calls() {
//     let mut server = start_rust_analyzer().await;

//     let stdin = server
//         .stdin
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdin of server process");

//     let stdout = server
//         .stdout
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdout of server process");

//     let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

//     let project_url =
//         Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

//     let response = code_depth::init(stdin, stdout, project_url.clone()).await;

//     // fail if response is err, but with nice debug info
//     response.unwrap();

//     let definitions =
//         code_depth::get_function_definitions(stdin, stdout, &project_url, Duration::from_secs(5))
//             .await
//             .unwrap();

//     let calls = code_depth::get_function_calls(stdin, stdout, &definitions, &project_url)
//         .await
//         .unwrap();

//     let mut short_calls: Vec<String> = calls
//         .iter()
//         .map(|(s, t)| {
//             format!(
//                 "{}:{}->{}:{}",
//                 Path::new(s.uri.path())
//                     .file_name()
//                     .unwrap()
//                     .to_str()
//                     .unwrap(),
//                 s.name,
//                 Path::new(t.uri.path())
//                     .file_name()
//                     .unwrap()
//                     .to_str()
//                     .unwrap(),
//                 t.name
//             )
//         })
//         .collect();

//     short_calls.sort();

//     assert_eq!(
//         short_calls,
//         vec![
//             "main.rs:foo->main.rs:in_foo",
//             "main.rs:impl_method->other_file.rs:other_file_method",
//             "main.rs:in_foo->main.rs:impl_method",
//             "main.rs:main->main.rs:foo",
//             "main.rs:main->main.rs:impl_method",
//         ]
//     );
// }

// #[tokio::test]
// async fn test_get_function_depths() {
//     let mut server = start_rust_analyzer().await;

//     let stdin = server
//         .stdin
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdin of server process");

//     let stdout = server
//         .stdout
//         .as_mut()
//         .take()
//         .expect("failed to acquire stdout of server process");

//     let sample_project_path = Path::new(SAMPLE_PROJECT_PATH).canonicalize().unwrap();

//     let project_url =
//         Url::from_file_path(sample_project_path).expect("failed to convert project path to URL");

//     let response = code_depth::init(stdin, stdout, project_url.clone()).await;

//     // fail if response is err, but with nice debug info
//     response.unwrap();

//     let definitions =
//         code_depth::get_function_definitions(stdin, stdout, &project_url, Duration::from_secs(5))
//             .await
//             .unwrap();

//     let calls = code_depth::get_function_calls(stdin, stdout, &definitions, &project_url)
//         .await
//         .unwrap();

//     let depths = code_depth::get_function_depths(calls);

//     let short_depths: Vec<_> = depths
//         .iter()
//         .map(|(s, t)| {
//             (
//                 format!(
//                     "method {}:{} depths",
//                     Path::new(s.uri.path())
//                         .file_name()
//                         .unwrap()
//                         .to_str()
//                         .unwrap(),
//                     s.name,
//                 ),
//                 t.iter().fold("".to_string(), |mut a, b| {
//                     a += "root ";
//                     a += Path::new(b.0.uri.path())
//                         .file_name()
//                         .unwrap()
//                         .to_str()
//                         .unwrap();
//                     a += &format!(":{} - {}", b.0.name, b.1);

//                     a
//                 }),
//             )
//         })
//         .collect();

//     assert!(short_depths.contains(&(
//         "method other_file.rs:other_file_method depths".into(),
//         "root main.rs:main - 2".into(),
//     ),));

//     assert!(short_depths.contains(&(
//         "method main.rs:in_foo depths".into(),
//         "root main.rs:main - 2".into(),
//     ),));

//     assert!(short_depths.contains(&(
//         "method main.rs:impl_method depths".into(),
//         "root main.rs:main - 1".into(),
//     ),));

//     assert!(short_depths.contains(&(
//         "method main.rs:foo depths".into(),
//         "root main.rs:main - 1".into(),
//     ),));

//     assert!(short_depths.contains(&(
//         "method main.rs:main depths".into(),
//         "root main.rs:main - 0".into(),
//     ),));
// }
