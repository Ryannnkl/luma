mod cli;
pub mod config;
mod demo;
pub mod wayland;

use std::{env, process::ExitCode};

use cli::Command;

fn main() -> ExitCode {
    match cli::parse(env::args().skip(1)) {
        Ok(Command::Demo {
            config: config_path,
        }) => {
            let config = match config::Config::load(config_path.as_deref()) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("luma: {error}");
                    return ExitCode::FAILURE;
                }
            };

            if let Err(error) = demo::run(config) {
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
