//! Shared types used in tealdeer.

use std::{fmt, str};

use anyhow::{anyhow, Result};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum PlatformType {
    Linux,
    OsX,
    SunOs,
    Windows,
    Android,
    FreeBSD,
    NetBSD,
    OpenBSD,
}

impl fmt::Display for PlatformType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Linux => write!(f, "Linux"),
            Self::OsX => write!(f, "macOS / BSD"),
            Self::SunOs => write!(f, "SunOS"),
            Self::Windows => write!(f, "Windows"),
            Self::Android => write!(f, "Android"),
            Self::FreeBSD => write!(f, "FreeBSD"),
            Self::NetBSD => write!(f, "NetBSD"),
            Self::OpenBSD => write!(f, "OpenBSD"),
        }
    }
}

impl str::FromStr for PlatformType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux" => Ok(Self::Linux),
            "osx" | "macos" => Ok(Self::OsX),
            "sunos" => Ok(Self::SunOs),
            "windows" => Ok(Self::Windows),
            "android" => Ok(Self::Android),
            "freebsd" => Ok(Self::FreeBSD),
            "netbsd" => Ok(Self::NetBSD),
            "openbsd" => Ok(Self::OpenBSD),
            other => Err(anyhow!(
                "Unknown OS: {}. Possible values: linux, macos, osx, sunos, windows, android, freebsd, netbsd, openbsd",
                other
            )),
        }
    }
}

impl PlatformType {
    #[cfg(target_os = "linux")]
    pub fn current() -> Self {
        Self::Linux
    }

    #[cfg(any(target_os = "macos", target_os = "dragonfly"))]
    pub fn current() -> Self {
        Self::OsX
    }

    #[cfg(target_os = "windows")]
    pub fn current() -> Self {
        Self::Windows
    }

    #[cfg(target_os = "android")]
    pub fn current() -> Self {
        Self::Android
    }

    #[cfg(any(target_os = "freebsd",))]
    pub fn current() -> Self {
        Self::FreeBSD
    }

    #[cfg(any(target_os = "netbsd",))]
    pub fn current() -> Self {
        Self::NetBSD
    }

    #[cfg(any(target_os = "openbsd",))]
    pub fn current() -> Self {
        Self::OpenBSD
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "windows",
        target_os = "android",
    )))]
    pub fn current() -> Self {
        Self::Other
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorOptions {
    Always,
    Auto,
    Never,
}

impl str::FromStr for ColorOptions {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "always" => Ok(Self::Always),
            "auto" => Ok(Self::Auto),
            "never" => Ok(Self::Never),
            other => Err(anyhow!(
                "Unknown color option: {}. Possible values: always, auto, never",
                other
            )),
        }
    }
}

impl Default for ColorOptions {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LineType {
    Empty,
    Title(String),
    Description(String),
    ExampleText(String),
    ExampleCode(String),
    Other(String),
}

impl<'a> From<&'a str> for LineType {
    /// Convert a string slice to a `LineType`. Newlines and trailing whitespace are trimmed.
    fn from(line: &'a str) -> Self {
        let trimmed: &str = line.trim_end();
        let mut chars = trimmed.chars();
        match chars.next() {
            None => Self::Empty,
            Some('#') => Self::Title(
                trimmed
                    .trim_start_matches(|chr: char| chr == '#' || chr.is_whitespace())
                    .into(),
            ),
            Some('>') => Self::Description(
                trimmed
                    .trim_start_matches(|chr: char| chr == '>' || chr.is_whitespace())
                    .into(),
            ),
            Some(' ') => Self::ExampleCode(trimmed.trim_start_matches(char::is_whitespace).into()),
            Some(_) => Self::ExampleText(trimmed.into()),
        }
    }
}

impl LineType {
    /// Support for old format.
    /// TODO: Remove once old format has been phased out!
    pub fn from_v1(line: &str) -> Self {
        let trimmed = line.trim();
        let mut chars = trimmed.chars();
        match chars.next() {
            None => Self::Empty,
            Some('#') => Self::Title(
                trimmed
                    .trim_start_matches(|chr: char| chr == '#' || chr.is_whitespace())
                    .into(),
            ),
            Some('>') => Self::Description(
                trimmed
                    .trim_start_matches(|chr: char| chr == '>' || chr.is_whitespace())
                    .into(),
            ),
            Some('-') => Self::ExampleText(
                trimmed
                    .trim_start_matches(|chr: char| chr == '-' || chr.is_whitespace())
                    .into(),
            ),
            Some('`') if chars.last() == Some('`') => Self::ExampleCode(
                trimmed
                    .trim_matches(|chr: char| chr == '`' || chr.is_whitespace())
                    .into(),
            ),
            Some(_) => Self::Other(trimmed.into()),
        }
    }
}

/// The reason why a certain path (e.g. config path or cache dir) was chosen.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum PathSource {
    /// OS convention (e.g. XDG on Linux)
    OsConvention,
    /// Env variable (TEALDEER_*)
    EnvVar,
    /// Config file
    ConfigFile,
}

impl fmt::Display for PathSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::OsConvention => "OS convention",
                Self::EnvVar => "env variable",
                Self::ConfigFile => "config file",
            }
        )
    }
}

#[cfg(test)]
mod test {
    use super::LineType;

    #[test]
    fn test_linetype_from_str() {
        assert_eq!(LineType::from(""), LineType::Empty);
        assert_eq!(LineType::from(" \n \r"), LineType::Empty);
        assert_eq!(
            LineType::from("# Hello there"),
            LineType::Title("Hello there".into())
        );
        assert_eq!(
            LineType::from("> tis a description \n"),
            LineType::Description("tis a description".into())
        );
        assert_eq!(
            LineType::from("some command "),
            LineType::ExampleText("some command".into())
        );
        assert_eq!(
            LineType::from("    $ cargo run "),
            LineType::ExampleCode("$ cargo run".into())
        );
    }
}
