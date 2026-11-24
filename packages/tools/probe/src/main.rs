//! `multideck-probe`: a dependency-free reconnaissance helper.
//!
//! It exists so the Stream Deck Mobile protocol work can answer three questions
//! without installing anything: which strings a binary carries, which TCP
//! sockets the desktop app opens, and what this machine looks like to the
//! adapter layer.

mod args;
mod env;
mod ports;
mod strings;

use std::process::ExitCode;

use args::{Command, StringsArgs};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();

    match args::parse(&argv) {
        Ok(Command::Help) => {
            println!("{}", args::USAGE);
            ExitCode::SUCCESS
        }
        Ok(Command::Strings(command)) => run_strings(&command),
        Ok(Command::Ports(command)) => run_ports(command.pid),
        Ok(Command::Env) => {
            run_env();
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            eprintln!("{}", args::USAGE);
            ExitCode::from(2)
        }
    }
}

/// Prints every matching printable run, capped at `--limit` lines.
fn run_strings(command: &StringsArgs) -> ExitCode {
    let bytes = match std::fs::read(&command.path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", command.path.display());
            return ExitCode::from(1);
        }
    };

    let found = strings::extract(&bytes, command.min);
    let label = command.path.display();
    let needle = command.grep.as_deref().unwrap_or("");
    let mut total = 0usize;
    let mut printed = 0usize;

    for hit in &found {
        if !strings::matches_grep(&hit.text, needle) {
            continue;
        }
        total += 1;
        if printed < command.limit {
            println!("{label}:{}: {}", hit.offset, hit.text);
            printed += 1;
        }
    }

    println!("# {total} match(es) in {label}, {printed} shown");

    if total == 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints TCP listeners, optionally filtered to one process id.
fn run_ports(filter_pid: Option<u32>) -> ExitCode {
    match ports::collect(filter_pid) {
        Ok(listeners) => {
            for listener in &listeners {
                match listener.pid {
                    Some(pid) => println!(
                        "{:<5} {}:{}  pid {pid}",
                        listener.proto, listener.address, listener.port
                    ),
                    None => println!(
                        "{:<5} {}:{}",
                        listener.proto, listener.address, listener.port
                    ),
                }
            }
            println!("# {} listener(s)", listeners.len());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}

/// Prints the host, build and data directory report.
fn run_env() {
    println!("os:        {}", std::env::consts::OS);
    println!(
        "arch:      {} ({})",
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    );
    println!(
        "profile:   {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!(
        "rust msrv: {}",
        option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown")
    );

    match env::data_dir(std::env::consts::OS, |key| std::env::var(key).ok()) {
        Some(dir) => println!("data dir:  {dir}"),
        None => println!("data dir:  unresolved (set MULTIDECK_DATA_DIR)"),
    }
}
