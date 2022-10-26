use std::process::Command;

use code_depth;

#[test]
fn connect_to_rust_analyzer() {
    let mut server = Command::new("rust-analyzer")
        .spawn()
        .expect("failed to start rust-analyzer");

    server.kill().expect("failed to stop rust-analyzer");
}
