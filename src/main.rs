pub mod auth;
mod cli;
pub mod config;
mod demo;
pub mod input;
pub mod wayland;

use std::{env, process::ExitCode, time::Duration};

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
        Ok(Command::Outputs) => list_outputs(),
        Ok(Command::Lock) => run_lock(),
        Ok(Command::LockSmoke) => run_lock_smoke(),
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

fn list_outputs() -> ExitCode {
    let tracker = match wayland::OutputTracker::connect() {
        Ok(tracker) => tracker,
        Err(error) => {
            eprintln!("luma: {error}");
            return ExitCode::FAILURE;
        }
    };

    if tracker.snapshots().is_empty() {
        eprintln!("luma: no Wayland outputs were reported");
        return ExitCode::FAILURE;
    }

    println!("Wayland outputs: {}", tracker.snapshots().len());
    for output in tracker.snapshots() {
        let name = output.name.as_deref().unwrap_or("unnamed");
        print!("- {} (global {})", name, output.global_id);
        if let Some((width, height)) = output.logical_size {
            print!(" {width}x{height}");
        }
        print!(
            " scale {} transform {}",
            output.scale_factor, output.transform
        );
        if let Some(mode) = &output.current_mode {
            let refresh_rate = f64::from(mode.refresh_rate_millihertz) / 1000.0;
            print!(" mode {}x{} @ {refresh_rate} Hz", mode.width, mode.height);
        }
        println!();
    }

    ExitCode::SUCCESS
}

fn run_lock_smoke() -> ExitCode {
    if env::var("LUMA_ALLOW_LOCK_SMOKE").as_deref() != Ok("1") {
        eprintln!(
            "luma: refusing --lock-smoke; set LUMA_ALLOW_LOCK_SMOKE=1 only inside nested niri"
        );
        return ExitCode::FAILURE;
    }

    match wayland::run_lock_smoke(Duration::from_secs(5)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("luma: lock smoke test failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_lock() -> ExitCode {
    if let Err(error) = auth::validate_service() {
        eprintln!("luma: refusing to lock: {error}");
        return ExitCode::FAILURE;
    }

    match wayland::run_lock() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("luma: session lock failed: {error}");
            ExitCode::FAILURE
        }
    }
}
