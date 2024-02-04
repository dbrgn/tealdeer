//! Definition of the CLI arguments and options.

use std::path::PathBuf;

use clap::{
    arg,
    builder::{ArgAction, EnumValueParser},
    command, ArgGroup, Parser,
};

use crate::types::{ColorOptions, PlatformType};

// Note: flag names are specified explicitly in clap attributes
// to improve readability and allow contributors to grep names like "clear-cache"
#[derive(Parser, Debug)]
#[command(
    about = "A fast TLDR client",
    version = "1.2.3",
    disable_version_flag = true,
    author,
    help_template = "{before-help}{name} {version}: {about-with-newline}{author-with-newline}
{usage-heading} {usage}

{all-args}{after-help}",
    after_help = "To view the user documentation, please visit https://dbrgn.github.io/tealdeer/.",
    arg_required_else_help = true,
    help_expected = true,
    group = ArgGroup::new("command_or_file").args(&["command", "render"]),
)]
pub(crate) struct Cli {
    /// The command to show (e.g. `tar` or `git log`)
    #[arg(num_args(1..))]
    pub command: Vec<String>,

    /// List all commands in the cache
    #[arg(short = 'l', long = "list")]
    pub list: bool,

    /// Render a specific markdown file
    #[arg(
        short = 'f',
        long = "render",
        value_name = "FILE",
        conflicts_with = "command"
    )]
    pub render: Option<PathBuf>,

    /// Override the operating system
    #[arg(
        short = 'p',
        long = "platform",
        action = ArgAction::Append,
        value_parser = EnumValueParser::<PlatformType>::new(),
    )]
    pub platforms: Option<Vec<PlatformType>>,

    /// Override the language
    #[arg(short = 'L', long = "language")]
    pub language: Option<String>,

    /// Update the local cache
    #[arg(short = 'u', long = "update")]
    pub update: bool,

    /// If auto update is configured, disable it for this run
    #[arg(long = "no-auto-update", requires = "command_or_file")]
    pub no_auto_update: bool,

    /// Clear the local cache
    #[arg(short = 'c', long = "clear-cache")]
    pub clear_cache: bool,

    /// Use a pager to page output
    #[arg(long = "pager", requires = "command_or_file")]
    pub pager: bool,

    /// Display the raw markdown instead of rendering it
    #[arg(short = 'r', long = "raw", requires = "command_or_file")]
    pub raw: bool,

    /// Suppress informational messages
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Show file and directory paths used by tealdeer
    #[arg(long = "show-paths")]
    pub show_paths: bool,

    /// Create a basic config
    #[arg(long = "seed-config")]
    pub seed_config: bool,

    /// Control whether to use color
    #[arg(
        long = "color",
        value_name = "WHEN",
        value_parser = EnumValueParser::<ColorOptions>::new(),
    )]
    //possible_values = ["always", "auto", "never"]
    pub color: Option<ColorOptions>,

    /// Print the version
    // Note: We override the version flag because clap uses `-V` by default,
    // while TLDR specification requires `-v` to be used.
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    pub version: Option<bool>,
}
