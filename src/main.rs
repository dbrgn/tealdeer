//! An implementation of [tldr](https://github.com/tldr-pages/tldr) in Rust.
//
// Copyright (c) 2015-2021 tealdeer developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be
// copied, modified, or distributed except according to those terms.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_debug_formatting)]
#![allow(clippy::while_let_loop)]

#[cfg(not(any(
    feature = "native-tls",
    feature = "rustls-with-webpki-roots",
    feature = "rustls-with-native-roots",
)))]
compile_error!(
    "at least one of the features \"native-tls\", \"rustls-with-webpki-roots\" or \"rustls-with-native-roots\" must be enabled"
);

use std::{
    env,
    fs::create_dir_all,
    io::{self, IsTerminal},
    path::Path,
    process::{Command, ExitCode},
};

use anyhow::{Context, Result, anyhow};
use cache::CacheConfig;
use clap::Parser;
use config::{ConfigLoader, Language, StyleConfig, TlsBackend};
use log::debug;
use types::PlatformType;

mod cache;
mod cli;
mod config;
pub mod extensions;
mod formatter;
mod line_iterator;
mod output;
mod types;
mod utils;

use crate::{
    cache::{Cache, PageLookupResult, TLDR_PAGES_DIR},
    cli::Cli,
    config::{
        Config, PathWithSource, PlaceholderFormat, get_config_dir, make_default_config,
        supported_tls_backends_string,
    },
    output::print_page,
    types::ColorOptions,
    utils::{print_error, print_warning},
};

const NAME: &str = "tealdeer";
static TEALDEER_PAGE: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/pages/tealdeer.md"));

/// Clear the cache
fn clear_cache(cache: Cache, quietly: bool) -> Result<()> {
    let cache_dir = cache.config().pages_directory.display();
    cache.clear().context("Could not clear cache")?;
    if !quietly {
        eprintln!("Successfully cleared cache at `{cache_dir}`.");
    }
    Ok(())
}

/// Update the cache
fn update_cache(
    cache: &mut Cache,
    archive_source: &str,
    tls_backend: TlsBackend,
    quietly: bool,
) -> Result<()> {
    let downloaded_languages = cache
        .update(archive_source, tls_backend)
        .context("Could not update cache")?;
    if !quietly {
        eprintln!("Successfully updated cache.");
        eprint!("Pages for the following languages were downloaded: ");
        let language_strings: Vec<_> = downloaded_languages
            .into_iter()
            .map(|lang| lang.0)
            .collect();
        if language_strings.is_empty() {
            eprintln!("(none)");
        } else {
            eprintln!("{}", language_strings.join(", "));
        }
    }
    Ok(())
}

/// Show file paths
fn show_paths(config: &Config) {
    let config_dir = {
        let (mut path, source) = get_config_dir();
        path.push(""); // Trailing path separator
        match path.to_str() {
            Some(path) => format!("{path} ({source})"),
            None => "[Invalid]".to_string(),
        }
    };
    let config_path = config.file_path.to_string();
    let cache_dir = config.directories.cache_dir.to_string();
    let pages_dir = {
        let mut path = config.directories.cache_dir.path.clone();
        path.push(TLDR_PAGES_DIR);
        path.push(""); // Trailing path separator
        path.display().to_string()
    };
    let custom_pages_dir = match config.directories.custom_pages_dir {
        Some(ref path_with_source) => path_with_source.to_string(),
        None => "[None]".to_string(),
    };
    println!("Config dir:       {config_dir}");
    println!("Config path:      {config_path}");
    println!("Cache dir:        {cache_dir}");
    println!("Pages dir:        {pages_dir}");
    println!("Custom pages dir: {custom_pages_dir}");
}

fn create_config(path: Option<&Path>) -> Result<()> {
    let config_file_path = make_default_config(path).context("Could not create seed config")?;
    eprintln!(
        "Successfully created seed config file here: {}",
        config_file_path.to_str().unwrap()
    );
    Ok(())
}

#[cfg(feature = "logging")]
fn init_log() {
    env_logger::init();
}

#[cfg(not(feature = "logging"))]
fn init_log() {}

fn spawn_editor(custom_pages_dir: &Path, file_name: &str) -> Result<()> {
    create_dir_all(custom_pages_dir).context("Failed to create custom pages directory")?;

    let custom_page_path = custom_pages_dir.join(file_name);
    let Some(custom_page_path) = custom_page_path.to_str() else {
        return Err(anyhow!("`custom_page_path.to_str()` failed"));
    };
    let Ok(editor) = env::var("EDITOR") else {
        return Err(anyhow!(
            "To edit a custom page, please set the `EDITOR` environment variable."
        ));
    };
    println!("Editing {custom_page_path:?}");

    let status = Command::new(&editor).arg(custom_page_path).status()?;
    if !status.success() {
        return Err(anyhow!("{editor} exit with code {:?}", status.code()));
    }
    Ok(())
}

fn main() -> ExitCode {
    // Initialize logger
    init_log();

    // Parse arguments
    let args = Cli::parse();

    // Determine the usage of styles
    let enable_styles = match args.color.unwrap_or_default() {
        // Attempt to use styling if instructed
        ColorOptions::Always => {
            yansi::enable(); // disable yansi's automatic detection for ANSI support on Windows
            true
        }
        // Enable styling if:
        // * NO_COLOR env var isn't set: https://no-color.org/
        // * The output stream is stdout (not being piped)
        ColorOptions::Auto => env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal(),
        // Disable styling
        ColorOptions::Never => false,
    };

    try_main(args, enable_styles).unwrap_or_else(|error| {
        print_error(enable_styles, &error);
        ExitCode::FAILURE
    })
}

fn try_main(args: Cli, enable_styles: bool) -> Result<ExitCode> {
    // Look up config file, if none is found fall back to default config.
    debug!("Loading config");
    let config_loader = match &args.config_path {
        Some(path) if !args.seed_config => ConfigLoader::read(path.clone(), &args.override_config)
            .context("Could not read config from given path")?,
        _ => ConfigLoader::read_default_path(&args.override_config)
            .context("Could not read config from default path")?,
    };
    let mut config = config_loader.load()?;

    // Override styles if needed
    if !enable_styles {
        config.style = StyleConfig::default();
    }

    config.display.placeholder_format = match (args.short_options, args.long_options) {
        (false, false) => config.display.placeholder_format, // keep old value
        (true, false) => PlaceholderFormat::Short,
        (false, true) => PlaceholderFormat::Long,
        (true, true) => PlaceholderFormat::Both,
    };

    let custom_pages_dir = config
        .directories
        .custom_pages_dir
        .as_ref()
        .map(PathWithSource::path);

    // Note: According to the TLDR client spec, page names must be transparently
    // lowercased before lookup:
    // https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md#page-names
    let command = args.command.join("-").to_lowercase();

    if args.edit_patch || args.edit_page {
        let file_name = if args.edit_patch {
            format!("{command}.patch.md")
        } else {
            format!("{command}.page.md")
        };

        custom_pages_dir
            .context("To edit custom pages/patches, please specify a custom pages directory.")
            .and_then(|custom_pages_dir| spawn_editor(custom_pages_dir, &file_name))?;

        return Ok(ExitCode::SUCCESS);
    }

    // Show various paths
    if args.show_paths {
        show_paths(&config);
    }

    // Create a basic config and exit
    if args.seed_config {
        create_config(args.config_path.as_deref())?;
        return Ok(ExitCode::SUCCESS);
    }

    // If a local file was passed in, render it and exit
    if let Some(file) = args.render {
        let reader = PageLookupResult::with_page(file).reader()?;
        print_page(reader, args.raw, enable_styles, args.pager, &config)?;
        return Ok(ExitCode::SUCCESS);
    }

    // The tealdeer page is embedded in the binary, no cache needed
    if command == "tealdeer" {
        print_page(
            TEALDEER_PAGE.as_bytes(),
            args.raw,
            enable_styles,
            args.pager,
            &config,
        )?;
        return Ok(ExitCode::SUCCESS);
    }

    if let Some(platforms) = args.platforms {
        config.search.platforms = platforms;
        if !config.search.platforms.contains(&PlatformType::Common) {
            config.search.platforms.push(PlatformType::Common);
        }
    }

    let (search_languages, download_languages): (&[_], &[_]) = match args.language.as_deref() {
        Some(lang) => (&[Language(lang)], &[Language(lang)]),
        None => (&config.search.languages, &config.updates.download_languages),
    };

    let cache_config = CacheConfig {
        pages_directory: &config.directories.cache_dir.path().join(TLDR_PAGES_DIR),
        custom_pages_directory: config
            .directories
            .custom_pages_dir
            .as_ref()
            .map(PathWithSource::path),
        platforms: &config.search.platforms,
        search_languages,
        download_languages,
    };

    if args.clear_cache {
        if let Some(cache) = Cache::open(cache_config)? {
            clear_cache(cache, args.quiet)?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    let cache = if args.update || config.updates.auto_update && !args.no_auto_update {
        let (mut cache, was_created) = Cache::open_or_create(cache_config, args.quiet)?;
        if was_created || args.update || cache.age()? >= config.updates.auto_update_interval {
            let result = update_cache(
                &mut cache,
                config.updates.archive_source,
                config.updates.tls_backend,
                args.quiet,
            );

            if let Err(e) = result {
                print_error(enable_styles, &e);

                eprintln!();
                eprintln!(
                    "Note: Update errors are often caused by unexpected or missing TLS certificates."
                );
                eprintln!(
                    "You are currently using the following TLS backend: {}",
                    config.updates.tls_backend,
                );
                eprintln!(
                    "Try changing the updates.tls_backend setting in the config file, for example:"
                );
                eprintln!();
                eprintln!("  [updates]");
                eprintln!("  tls_backend = \"rustls-with-native-roots\"");
                eprintln!();
                eprintln!(
                    "This build of tealdeer has support for the following options: {}",
                    supported_tls_backends_string(),
                );

                return Ok(ExitCode::FAILURE);
            }
        }

        cache
    } else if args.list || !command.is_empty() {
        // Cache is needed for these commands to work
        let Some(cache) = Cache::open(cache_config)? else {
            if !args.quiet {
                print_error(
                    enable_styles,
                    &anyhow::anyhow!(
                        "Page cache not found. Please run `tldr --update` to download the cache."
                    ),
                );
                println!("\nNote: You can optionally enable automatic cache updates by adding the");
                println!("following config to your config file:\n");
                println!("  [updates]");
                println!("  auto_update = true\n");
                println!("The path to your config file can be looked up with `tldr --show-paths`.");
                println!("To create an initial config file, use `tldr --seed-config`.\n");
                println!("You can find more tips and tricks in our docs:\n");
                println!("  https://docs.tealdeer.org");
            }

            return Ok(ExitCode::FAILURE);
        };

        if let Some(max_cache_age) = config.updates.warn_cache_age {
            let age = cache.age()?;
            if age > max_cache_age && !args.quiet {
                print_warning(
                    enable_styles,
                    &format!(
                        "The cache hasn't been updated for {} days.\n\
                         You should probably run `tldr --update` soon.",
                        age.as_secs() / 24 / 3600
                    ),
                );
            }
        }

        cache
    } else {
        // There is nothing left to do
        return Ok(ExitCode::SUCCESS);
    };

    if args.list {
        for page in cache.list_pages()? {
            println!("{page}");
        }

        return Ok(ExitCode::SUCCESS);
    }

    // Show command from cache
    if !command.is_empty() {
        // TODO: remove in tealdeer 1.10
        if !args.quiet && cache.old_custom_pages_exist()? {
            print_warning(
                enable_styles,
                &format!(
                    "Custom pages using the old naming convention were found in {}.\n\
                     Please rename them to follow the new convention:\n\
                     - `<name>.page` → `<name>.page.md`\n\
                     - `<name>.patch` → `<name>.patch.md`",
                    cache
                        .config()
                        .custom_pages_directory
                        .expect("Old custom pages can only exist in custom pages directory")
                        .display(),
                ),
            );
        }

        let Some(result) = cache.find_page(&command) else {
            if !args.quiet {
                print_warning(
                    enable_styles,
                    &format!(
                        "Page `{command}` not found in cache.\n\
                         Try updating with `tldr --update`, or submit a pull request to:\n\
                         https://github.com/tldr-pages/tldr"
                    ),
                );
            }
            return Ok(ExitCode::FAILURE);
        };

        print_page(
            result.reader()?,
            args.raw,
            enable_styles,
            args.pager,
            &config,
        )?;
    }

    Ok(ExitCode::SUCCESS)
}
