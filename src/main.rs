mod cli;

use std::{env, process::ExitCode};

use cli::Command;

fn main() -> ExitCode {
    match cli::parse(env::args().skip(1)) {
        Ok(Command::Demo) => {
            println!("Luma demo mode is not rendered yet; session locking is disabled.");
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
