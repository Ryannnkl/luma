use std::fmt;

const HELP: &str = "Luma — a secure Wayland session locker\n\nUsage: luma [OPTIONS]\n\nOptions:\n  --demo     Start the harmless visual demo\n  -h, --help Show this help\n  -V, --version Show version information";

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Demo,
    Help,
    Version,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ParseError {
    argument: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown argument `{}`; run `luma --help` for usage",
            self.argument
        )
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

    let command = match argument.as_str() {
        "--demo" => Command::Demo,
        "-h" | "--help" => Command::Help,
        "-V" | "--version" => Command::Version,
        _ => return Err(ParseError { argument }),
    };

    if let Some(argument) = arguments.next() {
        return Err(ParseError { argument });
    }

    Ok(command)
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
        assert_eq!(parse(["--demo".to_owned()]), Ok(Command::Demo));
    }

    #[test]
    fn rejects_unknown_arguments() {
        let error = parse(["--lock".to_owned()]).expect_err("--lock must not be available yet");

        assert!(error.to_string().contains("unknown argument `--lock`"));
    }

    #[test]
    fn rejects_trailing_arguments() {
        let error = parse(["--demo".to_owned(), "extra".to_owned()])
            .expect_err("demo mode accepts no trailing arguments");

        assert!(error.to_string().contains("unknown argument `extra`"));
    }
}
