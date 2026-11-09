//! Dedicated server: `cc-server [--port N] [--name NAME]`.

use std::sync::Arc;

use cc_core::ContentDb;
use cc_protocol::DEFAULT_PORT;
use cc_server::{ServerConfig, start};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut port = DEFAULT_PORT;
    let mut name = "Cosmic Conflict Server".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port),
            "--name" => name = args.next().unwrap_or(name),
            "-h" | "--help" => {
                println!("cc-server [--port N] [--name NAME]   (default port {DEFAULT_PORT})");
                return;
            }
            other => eprintln!("unknown argument {other}"),
        }
    }
    let content = Arc::new(ContentDb::embedded());
    match start(ServerConfig { bind: format!("0.0.0.0:{port}"), name }, content) {
        Ok(handle) => handle.wait(),
        Err(e) => {
            eprintln!("could not start the server: {e}");
            std::process::exit(1);
        }
    }
}
