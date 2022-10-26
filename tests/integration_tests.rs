use std::process::Command;

use code_depth;

#[test]
fn connect_to_rust_analyzer() {
    // start a rust-analyzer server inside our project directory
    let mut server = Command::new("rust-analyzer")
        .current_dir("..")
        .spawn()
        .expect("failed to start rust-analyzer");

    server.kill().expect("failed to stop rust-analyzer");
}
