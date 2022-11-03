use std::{env, path::Path, process::Stdio, time::Duration};

use lsp_types::Url;
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
};

// async fn start_lang_server(exe: &str) -> Child {
//     Command::new(exe)
//         .stdin(Stdio::piped())
//         .stdout(Stdio::piped())
//         .stderr(Stdio::piped())
//         .spawn()
//         .expect("failed to start rust-analyzer")
// }

#[tokio::main(flavor = "current_thread")]
async fn main() {
    //     let args: Vec<String> = env::args().collect();

    //     let project_path = Path::new(args.get(1).expect("missing argument <project_path>"));
    //     let lang_server_exe = args.get(2).expect("missing argument <lang_server_exe>");

    //     let mut server = start_lang_server(&lang_server_exe).await;

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

    //     tokio::spawn(async move {
    //         let stderr = server
    //             .stderr
    //             .as_mut()
    //             .take()
    //             .expect("failed to acquire stderr of server process");

    //         let mut buf = vec![];
    //         while let Ok(byte) = stderr.read_u8().await {
    //             buf.push(byte);

    //             if buf.len() > 100 {
    //                 buf.clear();
    //             }

    //             eprintln!(
    //                 "stderr {:?}",
    //                 std::str::from_utf8(&buf)
    //                     .unwrap()
    //                     .split('\n')
    //                     .collect::<Vec<_>>()
    //             );
    //         }
    //     });

    //     let project_path = project_path.canonicalize().unwrap();

    //     let project_url =
    //         Url::from_file_path(project_path).expect("failed to convert project path to URL");

    //     let response = code_depth::init(stdin, stdout, project_url.clone()).await;

    //     response.expect("failed to init lang server");

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
    //                     "{}:{}",
    //                     Path::new(s.uri.path())
    //                         .file_name()
    //                         .unwrap()
    //                         .to_str()
    //                         .unwrap(),
    //                     s.name,
    //                 ),
    //                 t.iter()
    //                     .map(|(r, d)| {
    //                         (
    //                             format!(
    //                                 "{}:{}",
    //                                 Path::new(r.uri.path())
    //                                     .file_name()
    //                                     .unwrap()
    //                                     .to_str()
    //                                     .unwrap(),
    //                                 r.name
    //                             ),
    //                             d,
    //                         )
    //                     })
    //                     .collect::<Vec<_>>(),
    //             )
    //         })
    //         .collect();

    //     println!("{:?}", short_depths);
}
