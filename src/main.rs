use std::io::{self, BufRead, Write};
use ultraviewer_core::mcp::Server;

fn main() {
    if std::env::args().any(|argument| argument == "--version") {
        println!("ultraviewer-mcp {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut server = Server::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            if writeln!(stdout, "{response}")
                .and_then(|_| stdout.flush())
                .is_err()
            {
                break;
            }
        }
    }
}
