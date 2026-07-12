use std::{fmt, path::PathBuf};

const HELP: &str = "Luma — a secure Wayland session locker\n\nUsage: luma --demo [--config PATH]\n       luma [OPTIONS]\n\nOptions:\n  --lock         Lock the session and authenticate through PAM\n  --demo         Start the harmless visual demo\n  --check        Check Wayland lock capabilities without locking\n  --outputs      List Wayland outputs without locking\n  --lock-smoke   Lock for five seconds (nested compositor only)\n  --config PATH  Use a specific TOML configuration with --demo\n  -h, --help     Show this help\n  -V, --version  Show version information";

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Demo { config: Option<PathBuf> },
    Lock,
    Check,
    Outputs,
    LockSmoke,
    Help,
    Version,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ParseError {
    message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; run `luma --help` for usage", self.message)
    }
}

pub fn parse<I>(arguments: I) -> Result<Command, ParseError>
where
    I: IntoIterator<Item = String>,
{
    let mut arguments = arguments.into_iter();
    let Some(argument) = arguments.next() else {
        return Ok(Command::Help);
    };

    if argument == "--demo" {
        return parse_demo_options(arguments);
    }

    let command = match argument.as_str() {
        "--lock" => Command::Lock,
        "--check" => Command::Check,
        "--outputs" => Command::Outputs,
        "--lock-smoke" => Command::LockSmoke,
        "-h" | "--help" => Command::Help,
        "-V" | "--version" => Command::Version,
        _ => return Err(ParseError::unknown(&argument)),
    };

    if let Some(argument) = arguments.next() {
        return Err(ParseError::unknown(&argument));
    }

    Ok(command)
}

fn parse_demo_options<I>(mut arguments: I) -> Result<Command, ParseError>
where
    I: Iterator<Item = String>,
{
    let mut config = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--config" if config.is_none() => {
                let path = arguments.next().ok_or_else(|| ParseError {
                    message: "missing path after `--config`".to_owned(),
                })?;
                config = Some(PathBuf::from(path));
            }
            "--config" => {
                return Err(ParseError {
                    message: "`--config` may only be provided once".to_owned(),
                });
            }
            _ => return Err(ParseError::unknown(&argument)),
        }
    }

    Ok(Command::Demo { config })
}

impl ParseError {
    fn unknown(argument: &str) -> Self {
        Self {
            message: format!("unknown argument `{argument}`"),
        }
    }
}

pub const fn help() -> &'static str {
    HELP
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};

    #[test]
    fn defaults_to_help_without_arguments() {
        assert_eq!(parse([]), Ok(Command::Help));
    }

    #[test]
    fn recognizes_demo_mode() {
        assert_eq!(
            parse(["--demo".to_owned()]),
            Ok(Command::Demo { config: None })
        );
    }

    #[test]
    fn recognizes_capability_check() {
        assert_eq!(parse(["--check".to_owned()]), Ok(Command::Check));
    }

    #[test]
    fn recognizes_output_listing() {
        assert_eq!(parse(["--outputs".to_owned()]), Ok(Command::Outputs));
    }

    #[test]
    fn recognizes_lock_smoke() {
        assert_eq!(parse(["--lock-smoke".to_owned()]), Ok(Command::LockSmoke));
    }

    #[test]
    fn recognizes_authenticated_lock() {
        assert_eq!(parse(["--lock".to_owned()]), Ok(Command::Lock));
    }

    #[test]
    fn accepts_custom_config_path() {
        assert_eq!(
            parse([
                "--demo".to_owned(),
                "--config".to_owned(),
                "/tmp/luma.toml".to_owned(),
            ]),
            Ok(Command::Demo {
                config: Some("/tmp/luma.toml".into()),
            })
        );
    }

    #[test]
    fn rejects_unknown_arguments() {
        let error = parse(["--unknown".to_owned()]).expect_err("argument should be rejected");

        assert!(error.to_string().contains("unknown argument `--unknown`"));
    }

    #[test]
    fn rejects_trailing_arguments() {
        let error = parse(["--demo".to_owned(), "extra".to_owned()])
            .expect_err("demo mode accepts no trailing arguments");

        assert!(error.to_string().contains("unknown argument `extra`"));
    }

    #[test]
    fn rejects_config_without_path() {
        let error = parse(["--demo".to_owned(), "--config".to_owned()])
            .expect_err("config requires a path");

        assert!(error.to_string().contains("missing path after `--config`"));
    }
}
