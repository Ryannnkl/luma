use std::{
    fmt,
    io::{self, BufRead, BufReader},
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const READY_MESSAGE: &str = "LUMA_LOCK_READY";

pub fn spawn_lock(config_path: Option<&Path>) -> Result<(), DaemonError> {
    let executable = std::env::current_exe().map_err(DaemonError::CurrentExecutable)?;
    let mut command = Command::new(executable);
    command
        .arg("--lock")
        .arg("--notify-ready")
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    if let Some(config_path) = config_path {
        command.arg("--config").arg(config_path);
    }

    let mut child = command.spawn().map_err(DaemonError::Spawn)?;
    let stdout = child.stdout.take().ok_or(DaemonError::MissingReadyPipe)?;
    let mut reader = BufReader::new(stdout);
    if read_ready_message(&mut reader).map_err(DaemonError::ReadReady)? {
        return Ok(());
    }

    let status = child.wait().map_err(DaemonError::Wait)?;
    Err(DaemonError::ExitedBeforeReady(status))
}

fn read_ready_message(reader: &mut impl BufRead) -> io::Result<bool> {
    let mut message = String::new();
    reader.read_line(&mut message)?;
    Ok(message.trim_end() == READY_MESSAGE)
}

pub const fn ready_message() -> &'static str {
    READY_MESSAGE
}

#[derive(Debug)]
pub enum DaemonError {
    CurrentExecutable(io::Error),
    Spawn(io::Error),
    MissingReadyPipe,
    ReadReady(io::Error),
    Wait(io::Error),
    ExitedBeforeReady(ExitStatus),
}

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentExecutable(source) => {
                write!(formatter, "could not resolve the Luma executable: {source}")
            }
            Self::Spawn(source) => write!(formatter, "could not start the lock child: {source}"),
            Self::MissingReadyPipe => formatter.write_str("lock child has no readiness pipe"),
            Self::ReadReady(source) => {
                write!(formatter, "could not read lock readiness: {source}")
            }
            Self::Wait(source) => write!(formatter, "could not wait for the lock child: {source}"),
            Self::ExitedBeforeReady(status) => {
                write!(
                    formatter,
                    "lock child exited before covering every output ({status})"
                )
            }
        }
    }
}

impl std::error::Error for DaemonError {}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::read_ready_message;

    #[test]
    fn accepts_only_the_complete_ready_message() {
        assert!(
            read_ready_message(&mut Cursor::new(b"LUMA_LOCK_READY\n"))
                .expect("ready message should be readable")
        );
        assert!(
            !read_ready_message(&mut Cursor::new(b"LUMA_LOCK\n"))
                .expect("invalid message should be readable")
        );
    }

    #[test]
    fn rejects_eof_before_readiness() {
        assert!(
            !read_ready_message(&mut Cursor::new(b""))
                .expect("empty readiness pipe should be readable")
        );
    }
}
