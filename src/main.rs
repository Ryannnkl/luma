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
        Ok(Command::Check) => check_wayland(),
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

fn check_wayland() -> ExitCode {
    let capabilities = match wayland::probe() {
        Ok(capabilities) => capabilities,
        Err(error) => {
            eprintln!("luma: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!("Wayland lock capability check");
    print_version(
        "ext_session_lock_manager_v1",
        capabilities.session_lock_version,
    );
    print_version("wl_compositor", capabilities.compositor_version);
    print_version("wl_shm", capabilities.shm_version);
    println!("wl_output: {}", capabilities.output_count);
    println!("wl_seat: {}", capabilities.seat_count);

    if capabilities.supports_lock_foundation() {
        println!("status: ready for the opaque lock-surface milestone");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "status: missing {}",
            capabilities.missing_requirements().join(", ")
        );
        ExitCode::FAILURE
    }
}

fn print_version(name: &str, version: Option<u32>) {
    match version {
        Some(version) => println!("{name}: v{version}"),
        None => println!("{name}: unavailable"),
    }
}
