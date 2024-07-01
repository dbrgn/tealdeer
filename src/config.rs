use std::{
    env, fmt, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, ensure, Context, Result};
use app_dirs::{get_app_root, AppDataType};
use log::debug;
use serde_derive::{Deserialize, Serialize};
use yansi::{Color, Style};

use crate::types::PathSource;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const MAX_CACHE_AGE: Duration = Duration::from_secs(2_592_000); // 30 days
const DEFAULT_UPDATE_INTERVAL_HOURS: u64 = MAX_CACHE_AGE.as_secs() / 3600; // 30 days
const ARCHIVE_URL: &str = "https://tldr.sh/assets/tldr.zip";

fn default_underline() -> bool {
    false
}

fn default_bold() -> bool {
    false
}

fn default_italic() -> bool {
    false
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum RawColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Purple, // Backwards compatibility with ansi_term (until tealdeer 1.5.0)
    Cyan,
    White,
    Ansi(u8),
    Rgb { r: u8, g: u8, b: u8 },
}

impl From<RawColor> for Color {
    fn from(raw_color: RawColor) -> Self {
        match raw_color {
            RawColor::Black => Self::Black,
            RawColor::Red => Self::Red,
            RawColor::Green => Self::Green,
            RawColor::Yellow => Self::Yellow,
            RawColor::Blue => Self::Blue,
            RawColor::Magenta | RawColor::Purple => Self::Magenta,
            RawColor::Cyan => Self::Cyan,
            RawColor::White => Self::White,
            RawColor::Ansi(num) => Self::Fixed(num),
            RawColor::Rgb { r, g, b } => Self::RGB(r, g, b),
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RawStyle {
    pub foreground: Option<RawColor>,
    pub background: Option<RawColor>,
    #[serde(default = "default_underline")]
    pub underline: bool,
    #[serde(default = "default_bold")]
    pub bold: bool,
    #[serde(default = "default_italic")]
    pub italic: bool,
}

#[allow(clippy::derivable_impls)] // Explicitly control defaults
impl Default for RawStyle {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            underline: false,
            bold: false,
            italic: false,
        }
    }
}

impl From<RawStyle> for Style {
    fn from(raw_style: RawStyle) -> Self {
        let mut style = Self::default();

        if let Some(foreground) = raw_style.foreground {
            style = style.fg(Color::from(foreground));
        }

        if let Some(background) = raw_style.background {
            style = style.bg(Color::from(background));
        }

        if raw_style.underline {
            style = style.underline();
        }

        if raw_style.bold {
            style = style.bold();
        }

        if raw_style.italic {
            style = style.italic();
        }

        style
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawStyleConfig {
    #[serde(default)]
    pub description: RawStyle,
    #[serde(default)]
    pub command_name: RawStyle,
    #[serde(default)]
    pub example_text: RawStyle,
    #[serde(default)]
    pub example_code: RawStyle,
    #[serde(default)]
    pub example_variable: RawStyle,
}

impl From<RawStyleConfig> for StyleConfig {
    fn from(raw_style_config: RawStyleConfig) -> Self {
        Self {
            command_name: raw_style_config.command_name.into(),
            description: raw_style_config.description.into(),
            example_text: raw_style_config.example_text.into(),
            example_code: raw_style_config.example_code.into(),
            example_variable: raw_style_config.example_variable.into(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawDisplayConfig {
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub use_pager: bool,
}

impl From<RawDisplayConfig> for DisplayConfig {
    fn from(raw_display_config: RawDisplayConfig) -> Self {
        Self {
            compact: raw_display_config.compact,
            use_pager: raw_display_config.use_pager,
        }
    }
}

/// Serde doesn't support default values yet (tracking issue:
/// <https://github.com/serde-rs/serde/issues/368>), so we need to wrap
/// `DEFAULT_UPDATE_INTERVAL_HOURS` in a function to be able to use
/// `#[serde(default = ...)]`
const fn default_auto_update_interval_hours() -> u64 {
    DEFAULT_UPDATE_INTERVAL_HOURS
}

fn default_archive_url() -> String {
    ARCHIVE_URL.to_owned()
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct RawUpdatesConfig {
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default = "default_auto_update_interval_hours")]
    pub auto_update_interval_hours: u64,
    #[serde(default = "default_archive_url")]
    pub archive_url: String,
}

impl Default for RawUpdatesConfig {
    fn default() -> Self {
        Self {
            auto_update: false,
            auto_update_interval_hours: DEFAULT_UPDATE_INTERVAL_HOURS,
            archive_url: default_archive_url(),
        }
    }
}

impl From<RawUpdatesConfig> for UpdatesConfig {
    fn from(raw_updates_config: RawUpdatesConfig) -> Self {
        Self {
            auto_update: raw_updates_config.auto_update,
            auto_update_interval: Duration::from_secs(
                raw_updates_config.auto_update_interval_hours * 3600,
            ),
            archive_url: raw_updates_config.archive_url,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RawDirectoriesConfig {
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub custom_pages_dir: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct RawConfig {
    style: RawStyleConfig,
    display: RawDisplayConfig,
    updates: RawUpdatesConfig,
    directories: RawDirectoriesConfig,
}

impl RawConfig {
    fn new() -> Self {
        Self::default()
    }
}

impl Default for RawConfig {
    fn default() -> Self {
        let mut raw_config = RawConfig {
            style: RawStyleConfig::default(),
            display: RawDisplayConfig::default(),
            updates: RawUpdatesConfig::default(),
            directories: RawDirectoriesConfig::default(),
        };

        // Set default config
        raw_config.style.example_text.foreground = Some(RawColor::Green);
        raw_config.style.command_name.foreground = Some(RawColor::Cyan);
        raw_config.style.example_code.foreground = Some(RawColor::Cyan);
        raw_config.style.example_variable.foreground = Some(RawColor::Cyan);
        raw_config.style.example_variable.underline = true;

        raw_config
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StyleConfig {
    pub description: Style,
    pub command_name: Style,
    pub example_text: Style,
    pub example_code: Style,
    pub example_variable: Style,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DisplayConfig {
    pub compact: bool,
    pub use_pager: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatesConfig {
    pub auto_update: bool,
    pub auto_update_interval: Duration,
    pub archive_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathWithSource {
    pub path: PathBuf,
    pub source: PathSource,
}

impl PathWithSource {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for PathWithSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.path.display(), self.source)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoriesConfig {
    pub cache_dir: PathWithSource,
    pub custom_pages_dir: Option<PathWithSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub style: StyleConfig,
    pub display: DisplayConfig,
    pub updates: UpdatesConfig,
    pub directories: DirectoriesConfig,
}

impl Config {
    /// Convert a `RawConfig` to a high-level `Config`.
    ///
    /// For this, some values need to be converted to other types and some
    /// defaults need to be set (sometimes based on env variables).
    fn from_raw(raw_config: RawConfig) -> Result<Self> {
        let style = raw_config.style.into();
        let display = raw_config.display.into();
        let updates = raw_config.updates.into();

        // Determine directories config. For this, we need to take some
        // additional factory into account, like env variables, or the
        // user config.
        let cache_dir_env_var = "TEALDEER_CACHE_DIR";
        let cache_dir = if let Ok(env_var) = env::var(cache_dir_env_var) {
            // For backwards compatibility reasons, the cache directory can be
            // overridden using an env variable. This is deprecated and will be
            // phased out in the future.
            eprintln!("Warning: The ${cache_dir_env_var} env variable is deprecated, use the `cache_dir` option in the config file instead.");
            PathWithSource {
                path: PathBuf::from(env_var),
                source: PathSource::EnvVar,
            }
        } else if let Some(config_value) = raw_config.directories.cache_dir {
            // If the user explicitly configured a cache directory, use that.
            PathWithSource {
                path: config_value,
                source: PathSource::ConfigFile,
            }
        } else if let Ok(default_dir) = get_app_root(AppDataType::UserCache, &crate::APP_INFO) {
            // Otherwise, fall back to the default user cache directory.
            PathWithSource {
                path: default_dir,
                source: PathSource::OsConvention,
            }
        } else {
            // If everything fails, give up
            bail!("Could not determine user cache directory");
        };
        let custom_pages_dir = raw_config
            .directories
            .custom_pages_dir
            .map(|path| PathWithSource {
                path,
                source: PathSource::ConfigFile,
            })
            .or_else(|| {
                get_app_root(AppDataType::UserData, &crate::APP_INFO)
                    .map(|path| {
                        // Note: The `join("")` call ensures that there's a trailing slash
                        PathWithSource {
                            path: path.join("pages").join(""),
                            source: PathSource::OsConvention,
                        }
                    })
                    .ok()
            });
        let directories = DirectoriesConfig {
            cache_dir,
            custom_pages_dir,
        };

        Ok(Self {
            style,
            display,
            updates,
            directories,
        })
    }

    pub fn load(enable_styles: bool) -> Result<Self> {
        debug!("Loading config");

        // Determine path
        let (config_file_path, _) = get_config_path().context("Could not determine config path")?;

        // Load raw config
        let raw_config: RawConfig = if config_file_path.exists() && config_file_path.is_file() {
            let mut config_file = fs::File::open(&config_file_path).with_context(|| {
                format!("Failed to open config file path at {:?}", &config_file_path)
            })?;
            let mut contents = String::new();
            config_file.read_to_string(&mut contents).with_context(|| {
                format!("Failed to read from config file at {:?}", &config_file_path)
            })?;
            toml::from_str(&contents).with_context(|| {
                format!("Failed to parse TOML config file at {config_file_path:?}")
            })?
        } else {
            RawConfig::new()
        };

        // Convert to config
        let mut config = Self::from_raw(raw_config).context("Could not process raw config")?;

        // Potentially override styles
        if !enable_styles {
            config.style = StyleConfig {
                command_name: Style::default(),
                description: Style::default(),
                example_text: Style::default(),
                example_code: Style::default(),
                example_variable: Style::default(),
            };
        }

        Ok(config)
    }
}

/// Return the path to the config directory.
///
/// The config dir path can be overridden using the `TEALDEER_CONFIG_DIR` env
/// variable. Otherwise, the user config directory is returned.
///
/// Note that this function does not verify whether the directory at that
/// location exists, or is a directory.
pub fn get_config_dir() -> Result<(PathBuf, PathSource)> {
    // Allow overriding the config directory by setting the
    // $TEALDEER_CONFIG_DIR env variable.
    if let Ok(value) = env::var("TEALDEER_CONFIG_DIR") {
        return Ok((PathBuf::from(value), PathSource::EnvVar));
    };

    // Otherwise, fall back to the user config directory.
    let dirs = get_app_root(AppDataType::UserConfig, &crate::APP_INFO)
        .context("Failed to determine the user config directory")?;
    Ok((dirs, PathSource::OsConvention))
}

/// Return the path to the config file.
///
/// Note that this function does not verify whether the file at that location
/// exists, or is a file.
pub fn get_config_path() -> Result<(PathBuf, PathSource)> {
    let (config_dir, source) = get_config_dir()?;
    let config_file_path = config_dir.join(CONFIG_FILE_NAME);
    Ok((config_file_path, source))
}

/// Create default config file.
pub fn make_default_config() -> Result<PathBuf> {
    let (config_dir, _) = get_config_dir()?;

    // Ensure that config directory exists
    if config_dir.exists() {
        ensure!(
            config_dir.is_dir(),
            "Config directory could not be created: {} already exists but is not a directory",
            config_dir.to_string_lossy(),
        );
    } else {
        fs::create_dir_all(&config_dir).context("Could not create config directory")?;
    }

    // Ensure that a config file doesn't get overwritten
    let config_file_path = config_dir.join(CONFIG_FILE_NAME);
    ensure!(
        !config_file_path.is_file(),
        "A configuration file already exists at {}, no action was taken.",
        config_file_path.to_str().unwrap()
    );

    // Create default config
    let serialized_config =
        toml::to_string(&RawConfig::new()).context("Failed to serialize default config")?;

    // Write default config
    let mut config_file =
        fs::File::create(&config_file_path).context("Could not create config file")?;
    let _wc = config_file
        .write(serialized_config.as_bytes())
        .context("Could not write to config file")?;

    Ok(config_file_path)
}

#[test]
fn test_serialize_deserialize() {
    let raw_config = RawConfig::new();
    let serialized = toml::to_string(&raw_config).unwrap();
    let deserialized: RawConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(raw_config, deserialized);
}
