mod cli;
mod demo;

use std::{env, process::ExitCode};

use cli::Command;

fn main() -> ExitCode {
    match cli::parse(env::args().skip(1)) {
        Ok(Command::Demo) => {
            if let Err(error) = demo::run() {
                eprintln!("luma: could not start demo: {error}");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        Ok(Command::Help) => {
            println!("{}", cli::help());
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("luma {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("luma: {error}");
            ExitCode::FAILURE
        }
    }
}
