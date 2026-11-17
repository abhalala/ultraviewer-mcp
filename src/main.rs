use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;
use ultraviewer_core::mcp::Server;
use ultraviewer_core::screen::dictionary;

const USAGE: &str = "\
Usage:
  ultraviewer-mcp                            Run the MCP server on stdin/stdout
  ultraviewer-mcp install-dictionary <path>  Verify and install your 8bit.dat
  ultraviewer-mcp check-dictionary           Show which 8bit.dat will be used
  ultraviewer-mcp --version";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        None => serve(),
        Some("--version") => {
            println!("ultraviewer-mcp {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("install-dictionary") => match arguments.get(1) {
            Some(source) if arguments.len() == 2 => install(Path::new(source)),
            _ => usage_error(),
        },
        Some("check-dictionary") if arguments.len() == 1 => check(),
        Some(_) => usage_error(),
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn install(source: &Path) -> ExitCode {
    match dictionary::install(source) {
        Ok(target) => {
            println!(
                "Verified SHA-256 {} and installed {}",
                dictionary::DICTIONARY_SHA256,
                target.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn check() -> ExitCode {
    match dictionary::locate() {
        Ok((path, _)) => {
            println!("Verified {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> ExitCode {
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
    ExitCode::SUCCESS
}
