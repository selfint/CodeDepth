use std::{collections::HashSet, path::PathBuf, process::Stdio, time::Duration};

use clap::Parser;
use lsp_types::{CallHierarchyItem, Url};
use regex::Regex;
use serde_json::{json, Value};
use tokio::process::Command;

use code_depth::{
    build_call_hierarchy_item_name, hashable_call_hierarchy_item::HashableCallHierarchyItem,
    lsp::LspClient, Depths,
};

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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    project_path: PathBuf,

    #[arg(short, long)]
    lang_server_exe: String,

    #[arg(short, long, default_value = ".*test.*")]
    ignore_re: Option<String>,
}

fn parse_args() -> (PathBuf, String, Regex) {
    let args = Args::parse();

    let project_path = args
        .project_path
        .canonicalize()
        .expect("given <project_path> couldn't be canonicalized");

    let lang_server_exe = args.lang_server_exe;

    let test_re = if let Some(test_str) = args.ignore_re {
        Regex::new(&test_str).unwrap_or_else(|_| panic!("invalid regex: '{}'", test_str))
    } else {
        Regex::new(".*test.*").unwrap()
    };

    (project_path, lang_server_exe, test_re)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let (project_path, lang_server_exe, test_re) = parse_args();

    let mut client = start_lang_server(&lang_server_exe).await;

    let project_url =
        Url::from_file_path(project_path).expect("failed to convert project path to URL");

    let response = code_depth::init(&mut client, project_url.clone()).await;

    response.expect("failed to init lang server");

    let workspace_files =
        code_depth::get_workspace_files(&mut client, &project_url, Duration::from_secs(5))
            .await
            .unwrap();

    let calls = code_depth::get_function_calls(&mut client, &workspace_files, &project_url)
        .await
        .unwrap();

    let non_test_calls = filter_calls(calls, &test_re, |call: &CallHierarchyItem| {
        code_depth::build_call_hierarchy_item_name(call, &project_url)
    });

    let depths = code_depth::get_function_depths(non_test_calls);
    let results_json = build_results_json(&depths, &project_url);

    println!("{}", serde_json::to_string_pretty(&results_json).unwrap());
}

fn build_results_json(depths: &Depths<CallHierarchyItem>, project_url: &Url) -> Value {
    let mut results_json = json!({});

    results_json["ok"] = json!({});
    results_json["problems"] = json!({});

    // find all items with different depths
    let problem_items =
        code_depth::find_items_with_different_depths::<_, HashableCallHierarchyItem>(depths)
            .iter()
            .map(|item| build_call_hierarchy_item_name(&item.0, project_url))
            .collect::<HashSet<_>>();

    code_depth::build_short_fn_depths(project_url, depths)
        .iter()
        .for_each(|(item_name, item_depths_from_roots)| {
            let item_depths_from_roots = serde_json::to_value(item_depths_from_roots).unwrap();

            if problem_items.contains(item_name) {
                results_json["problems"][item_name] = item_depths_from_roots;
            } else {
                results_json["ok"][item_name] = item_depths_from_roots;
            }
        });

    results_json
}

fn filter_calls<F: Fn(&CallHierarchyItem) -> String>(
    calls: Vec<(CallHierarchyItem, CallHierarchyItem)>,
    test_re: &Regex,
    item_to_str: F,
) -> Vec<(CallHierarchyItem, CallHierarchyItem)> {
    calls
        .into_iter()
        .filter(|(to, from)| {
            !(test_re.is_match(&item_to_str(to)) || test_re.is_match(&item_to_str(from)))
        })
        .collect::<Vec<_>>()
}
