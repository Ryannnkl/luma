use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{Config, ValidationError};

impl Config {
    /// Loads an explicit TOML file or the default user configuration.
    ///
    /// A missing default file produces the built-in configuration. A missing
    /// explicit file is considered an error.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, contains invalid TOML, has
    /// unknown fields, or fails value validation.
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, LoadError> {
        if let Some(path) = explicit_path {
            return load_path(path);
        }

        let Some(path) = default_path() else {
            return Ok(Self::default());
        };

        match fs::read_to_string(&path) {
            Ok(contents) => parse(&contents, path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(LoadError::Read { path, source }),
        }
    }
}

#[must_use]
pub fn default_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        && path.is_absolute()
    {
        return Some(path.join("luma/config.toml"));
    }

    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".config/luma/config.toml"))
}

#[derive(Debug)]
pub enum LoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    Invalid {
        path: PathBuf,
        source: ValidationError,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(formatter, "invalid TOML in {}: {source}", path.display())
            }
            Self::Invalid { path, source } => {
                write!(formatter, "invalid value in {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Invalid { source, .. } => Some(source),
        }
    }
}

fn load_path(path: &Path) -> Result<Config, LoadError> {
    let path = path.to_owned();
    let contents = fs::read_to_string(&path).map_err(|source| LoadError::Read {
        path: path.clone(),
        source,
    })?;
    parse(&contents, path)
}

fn parse(contents: &str, path: PathBuf) -> Result<Config, LoadError> {
    let config: Config = toml::from_str(contents).map_err(|source| LoadError::Parse {
        path: path.clone(),
        source,
    })?;
    config
        .validate()
        .map_err(|source| LoadError::Invalid { path, source })?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Config, LoadError, parse};

    #[test]
    fn parses_and_validates_configuration() {
        let config = parse(
            r#"
                [date]
                enabled = true
                format = "%Y-%m-%d"
            "#,
            PathBuf::from("test.toml"),
        )
        .expect("valid configuration should load");

        assert!(config.date.enabled);
        assert_eq!(config.date.format, "%Y-%m-%d");
    }

    #[test]
    fn rejects_invalid_values_after_parsing() {
        let error = parse(
            r"
                [input]
                max_characters = 0
            ",
            PathBuf::from("test.toml"),
        )
        .expect_err("invalid values must fail during loading");

        assert!(matches!(error, LoadError::Invalid { .. }));
    }

    #[test]
    fn missing_explicit_file_is_an_error() {
        let error = Config::load(Some(Path::new("/luma/does-not-exist.toml")))
            .expect_err("explicit files must exist");

        assert!(matches!(error, LoadError::Read { .. }));
    }

    #[test]
    fn example_configuration_stays_valid() {
        parse(
            include_str!("../../config.example.toml"),
            PathBuf::from("config.example.toml"),
        )
        .expect("the documented example must remain valid");
    }
}
