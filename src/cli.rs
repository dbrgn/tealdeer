//! Definition of the CLI arguments and options.

use std::path::PathBuf;

use clap::{AppSettings, ArgAction, ArgGroup, Parser};

use crate::types::{ColorOptions, PlatformType};

// Note: flag names are specified explicitly in clap attributes
// to improve readability and allow contributors to grep names like "clear-cache"
#[derive(Parser, Debug)]
#[clap(about = "A fast TLDR client", author, version)]
#[clap(
    after_help = "To view the user documentation, please visit https://dbrgn.github.io/tealdeer/."
)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(arg_required_else_help(true))]
#[clap(disable_colored_help(true))]
#[clap(group = ArgGroup::new("command_or_file").args(&["command", "render"]))]
pub(crate) struct Args {
    /// The command to show (e.g. `tar` or `git log`)
    #[clap(min_values = 1)]
    pub command: Vec<String>,

    /// List all commands in the cache
    #[clap(short = 'l', long = "list")]
    pub list: bool,

    /// Render a specific markdown file
    #[clap(
        short = 'f',
        long = "render",
        value_name = "FILE",
        conflicts_with = "command"
    )]
    pub render: Option<PathBuf>,

    /// Override the operating system
    #[clap(
        short = 'p',
        long = "platform",
        action = ArgAction::Append,
        possible_values = ["linux", "macos", "windows", "sunos", "osx", "android", "freebsd", "netbsd", "openbsd"],
    )]
    pub platforms: Option<Vec<PlatformType>>,

    /// Override the language
    #[clap(short = 'L', long = "language")]
    pub language: Option<String>,

    /// Update the local cache
    #[clap(short = 'u', long = "update")]
    pub update: bool,

    /// If auto update is configured, disable it for this run
    #[clap(long = "no-auto-update", requires = "command_or_file")]
    pub no_auto_update: bool,

    /// Clear the local cache
    #[clap(short = 'c', long = "clear-cache")]
    pub clear_cache: bool,

    /// Use a pager to page output
    #[clap(long = "pager", requires = "command_or_file")]
    pub pager: bool,

    /// Display the raw markdown instead of rendering it
    #[clap(short = 'r', long = "--raw", requires = "command_or_file")]
    pub raw: bool,

    /// Suppress informational messages
    #[clap(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Show file and directory paths used by tealdeer
    #[clap(long = "show-paths")]
    pub show_paths: bool,

    /// Create a basic config
    #[clap(long = "seed-config")]
    pub seed_config: bool,

    /// Control whether to use color
    #[clap(
        long = "color",
        value_name = "WHEN",
        possible_values = ["always", "auto", "never"]
    )]
    pub color: Option<ColorOptions>,

    /// Print the version
    // Note: We override the version flag because clap uses `-V` by default,
    // while TLDR specification requires `-v` to be used.
    #[clap(short = 'v', long = "version")]
    pub version: bool,
}
