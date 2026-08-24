use std::{
    borrow::Cow,
    env, fmt,
    fs::{self, File},
    io::{ErrorKind, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use clap::ValueEnum;
use log::info;
use serde::Serialize as _;
use serde_derive::{Deserialize, Serialize};
use toml::map::Map;
use yansi::{Color, Style};

use crate::{
    extensions::Dedup as _,
    types::{PathSource, PlatformType},
};

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const MAX_CACHE_AGE: Duration = Duration::from_secs(2_592_000); // 30 days
const DEFAULT_UPDATE_INTERVAL_HOURS: u64 = MAX_CACHE_AGE.as_secs() / 3600; // 30 days
const SUPPORTED_TLS_BACKENDS: &[RawTlsBackend] = &[
    #[cfg(feature = "native-tls")]
    RawTlsBackend::NativeTls,
    #[cfg(feature = "rustls-with-webpki-roots")]
    RawTlsBackend::RustlsWithWebpkiRoots,
    #[cfg(feature = "rustls-with-native-roots")]
    RawTlsBackend::RustlsWithNativeRoots,
];

struct SystemDirectories {
    config: PathBuf,
    cache: PathBuf,
    data: PathBuf,
}

impl SystemDirectories {
    fn discover() -> Result<Self> {
        use etcetera::{
            AppStrategy, AppStrategyArgs, app_strategy::choose_native_strategy, choose_app_strategy,
        };

        let args = AppStrategyArgs {
            top_level_domain: String::new(),
            author: String::new(),
            app_name: crate::NAME.to_string(),
        };

        // The app strategy prefers XDG on MacOs, whereas the native strategy returns paths which
        // are used by installed applications. On Linux and Windows, the strategies are the same.
        let app_dirs = choose_app_strategy(args.clone())?;
        let native_dirs = choose_native_strategy(args)?;

        // We prefer the XDG paths, but before tealdeer 1.9, we used only the native paths on MacOs.
        // So if we find files in these locations, we keep using them.
        let fallback = |app_dir: PathBuf, native_dir: PathBuf| {
            if !app_dir.exists() && native_dir.exists() {
                native_dir
            } else {
                app_dir
            }
        };

        Ok(Self {
            config: fallback(app_dirs.config_dir(), native_dirs.config_dir()),
            cache: fallback(app_dirs.cache_dir(), native_dirs.cache_dir()),
            data: fallback(app_dirs.data_dir(), native_dirs.data_dir()),
        })
    }
}
static SYSTEM_DIRECTORIES: LazyLock<SystemDirectories> = LazyLock::new(|| {
    SystemDirectories::discover().expect("Failed to initialize system directories.")
});

pub(crate) fn supported_tls_backends_string() -> String {
    SUPPORTED_TLS_BACKENDS
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}

fn default_underline() -> bool {
    false
}

const fn default_base_indent() -> usize {
    2
}

const fn default_command_indent() -> usize {
    6
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
            RawColor::Rgb { r, g, b } => Self::Rgb(r, g, b),
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

impl From<&RawStyleConfig> for StyleConfig {
    fn from(raw_style_config: &RawStyleConfig) -> Self {
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
#[serde(deny_unknown_fields)]
struct RawDisplayConfig {
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub use_pager: bool,
    #[serde(default)]
    pub show_title: bool,
    #[serde(default)]
    pub indent: RawIndent,
    #[serde(default)]
    pub placeholder_format: PlaceholderFormat,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawIndent {
    #[serde(default = "default_base_indent")]
    base: usize,
    #[serde(default = "default_command_indent")]
    command: usize,
}

impl Default for RawIndent {
    fn default() -> Self {
        Self {
            base: 2,
            command: 6,
        }
    }
}

impl From<RawIndent> for Indent {
    fn from(raw_indent: RawIndent) -> Self {
        Self {
            base: raw_indent.base,
            command: raw_indent.command,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlaceholderFormat {
    Short,
    #[default]
    Long,
    Both,
}

impl From<&RawDisplayConfig> for DisplayConfig {
    fn from(raw_display_config: &RawDisplayConfig) -> Self {
        Self {
            compact: raw_display_config.compact,
            use_pager: raw_display_config.use_pager,
            show_title: raw_display_config.show_title,
            indent: raw_display_config.indent.into(),
            placeholder_format: raw_display_config.placeholder_format,
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

fn default_archive_source() -> String {
    "https://github.com/tldr-pages/tldr/releases/latest/download".to_owned()
}

/// Controls when a warning about an outdated cache is printed.
///
/// Currently, the only nameable option is `"never"`. In the future, this may
/// be extended to also accept a duration (e.g. `"60d"`), after which the
/// warning should be shown.
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RawWarnCacheAge {
    Never,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawUpdatesConfig {
    #[serde(default)]
    pub auto_update: bool,
    #[serde(default = "default_auto_update_interval_hours")]
    pub auto_update_interval_hours: u64,
    #[serde(default = "default_archive_source")]
    pub archive_source: String,
    #[serde(default)]
    pub tls_backend: RawTlsBackend,
    #[serde(default)]
    pub download_languages: Option<Vec<String>>,
    #[serde(default)]
    pub warn_cache_age: Option<RawWarnCacheAge>,
}

impl Default for RawUpdatesConfig {
    fn default() -> Self {
        Self {
            auto_update: false,
            auto_update_interval_hours: DEFAULT_UPDATE_INTERVAL_HOURS,
            archive_source: default_archive_source(),
            tls_backend: RawTlsBackend::default(),
            download_languages: None,
            warn_cache_age: None,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawDirectoriesConfig {
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,
    #[serde(default)]
    pub custom_pages_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawPlatformType {
    Current,
    All,
    MacOs, // alias for Platform(PlatformType::OsX)
    #[serde(untagged)]
    Platform(PlatformType),
}

impl RawPlatformType {
    pub fn flatten(raw_platforms: impl IntoIterator<Item = Self>) -> Vec<PlatformType> {
        let mut flattened = Vec::new();
        for raw_platform in raw_platforms {
            match raw_platform {
                RawPlatformType::Current => flattened.push(PlatformType::current()),
                RawPlatformType::Platform(platform) => flattened.push(platform),
                RawPlatformType::MacOs => flattened.push(PlatformType::OsX),
                RawPlatformType::All => flattened.extend(PlatformType::value_variants()),
            }
        }
        flattened.clear_duplicates();
        flattened
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RawSearchConfig {
    pub languages: Option<Vec<String>>,
    pub platforms: Option<Vec<RawPlatformType>>,
}

impl<'a> From<&'a RawSearchConfig> for SearchConfig<'a> {
    fn from(raw_search_config: &'a RawSearchConfig) -> Self {
        let languages = raw_search_config
            .languages
            .as_ref()
            .map_or_else(get_languages_from_env, |langs| {
                langs.iter().map(|lang| Language(lang)).collect()
            });
        let platforms = if let Some(raw_platforms) = raw_search_config.platforms.as_ref() {
            RawPlatformType::flatten(raw_platforms.iter().copied())
        } else {
            RawPlatformType::flatten([
                RawPlatformType::Current,
                RawPlatformType::Platform(PlatformType::Common),
                RawPlatformType::All,
            ])
        };

        Self {
            languages,
            platforms,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    style: RawStyleConfig,
    display: RawDisplayConfig,
    updates: RawUpdatesConfig,
    directories: RawDirectoriesConfig,
    search: RawSearchConfig,
}

impl Default for RawConfig {
    fn default() -> Self {
        let mut raw_config = RawConfig {
            style: RawStyleConfig::default(),
            display: RawDisplayConfig::default(),
            updates: RawUpdatesConfig::default(),
            directories: RawDirectoriesConfig::default(),
            search: RawSearchConfig::default(),
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

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
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
    pub show_title: bool,
    pub indent: Indent,
    pub placeholder_format: PlaceholderFormat,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Indent {
    pub base: usize,
    pub command: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatesConfig<'a> {
    pub auto_update: bool,
    pub auto_update_interval: Duration,
    pub archive_source: &'a str,
    pub tls_backend: TlsBackend,
    pub download_languages: Vec<Language<'a>>,
    pub warn_cache_age: Option<Duration>,
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
pub struct SearchConfig<'a> {
    pub languages: Vec<Language<'a>>,
    pub platforms: Vec<PlatformType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language<'a>(pub &'a str);

fn get_languages<'a>(
    env_lang: Option<&'a str>,
    env_language: Option<&'a str>,
) -> Vec<Language<'a>> {
    // Language list according to
    // https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md#language

    let Some(env_lang) = env_lang else {
        return vec![Language("en")];
    };

    // Create an iterator that contains $LANGUAGE (':' separated list) followed by $LANG (single language)
    let locales = env_language.unwrap_or("").split(':').chain([env_lang]);

    let mut lang_list = Vec::new();
    for locale in locales {
        if !locale.is_ascii() {
            info!("Skipping non-ASCII locale string: {locale}");
            continue;
        }

        // Language plus country code (e.g. `en_US`)
        if locale.len() >= 5 && locale.chars().nth(2) == Some('_') {
            lang_list.push(Language(&locale[..5]));
        }
        // Language code only (e.g. `en`)
        if locale.len() >= 2 && locale != "POSIX" {
            lang_list.push(Language(&locale[..2]));
        }
    }

    lang_list.push(Language("en"));
    lang_list.clear_duplicates();
    lang_list
}

pub fn get_languages_from_env<'a>() -> Vec<Language<'a>> {
    static LANG: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("LANG").ok());
    static LANGUAGE: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("LANGUAGE").ok());
    get_languages(
        LANG.as_ref().map(String::as_str),
        LANGUAGE.as_ref().map(String::as_str),
    )
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawTlsBackend {
    /// Native TLS (`SChannel` on Windows, Secure Transport on macOS and OpenSSL otherwise)
    NativeTls,
    /// Rustls with `WebPKI` roots.
    RustlsWithWebpkiRoots,
    /// Rustls with native roots.
    RustlsWithNativeRoots,
}

impl Default for RawTlsBackend {
    fn default() -> Self {
        *SUPPORTED_TLS_BACKENDS.first().unwrap()
    }
}

impl std::fmt::Display for RawTlsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.serialize(f)
    }
}

/// Allows choosing a `reqwest`'s TLS backend. Available TLS backends:
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TlsBackend {
    /// Native TLS (`SChannel` on Windows, Secure Transport on macOS and OpenSSL otherwise)
    #[cfg(feature = "native-tls")]
    NativeTls,
    /// Rustls with `WebPKI` roots.
    #[cfg(feature = "rustls-with-webpki-roots")]
    RustlsWithWebpkiRoots,
    /// Rustls with native roots.
    #[cfg(feature = "rustls-with-native-roots")]
    RustlsWithNativeRoots,
}

impl TryFrom<RawTlsBackend> for TlsBackend {
    type Error = anyhow::Error;

    fn try_from(raw: RawTlsBackend) -> Result<Self, Self::Error> {
        match raw {
            #[cfg(feature = "native-tls")]
            RawTlsBackend::NativeTls => Ok(TlsBackend::NativeTls),
            #[cfg(feature = "rustls-with-webpki-roots")]
            RawTlsBackend::RustlsWithWebpkiRoots => Ok(TlsBackend::RustlsWithWebpkiRoots),
            #[cfg(feature = "rustls-with-native-roots")]
            RawTlsBackend::RustlsWithNativeRoots => Ok(TlsBackend::RustlsWithNativeRoots),
            // when compiling without all TLS backend features, we want to handle config error.
            #[allow(unreachable_patterns)]
            _ => Err(anyhow!(
                "Unsupported TLS backend: {}. This tealdeer build has support for the following options: {}",
                raw,
                supported_tls_backends_string(),
            )),
        }
    }
}

impl TlsBackend {
    const fn as_raw(self) -> RawTlsBackend {
        match self {
            #[cfg(feature = "native-tls")]
            Self::NativeTls => RawTlsBackend::NativeTls,
            #[cfg(feature = "rustls-with-webpki-roots")]
            Self::RustlsWithWebpkiRoots => RawTlsBackend::RustlsWithWebpkiRoots,
            #[cfg(feature = "rustls-with-native-roots")]
            Self::RustlsWithNativeRoots => RawTlsBackend::RustlsWithNativeRoots,
        }
    }
}

impl fmt::Display for TlsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_raw().fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config<'a> {
    pub style: StyleConfig,
    pub display: DisplayConfig,
    pub updates: UpdatesConfig<'a>,
    pub directories: DirectoriesConfig,
    pub search: SearchConfig<'a>,
    pub file_path: PathWithSource,
}

impl<'a> Config<'a> {
    /// Convert a `RawConfig` to a high-level `Config`.
    ///
    /// For this, some values need to be converted to other types and some
    /// defaults need to be set (sometimes based on env variables).
    fn from_raw(raw_config: &'a RawConfig, config_file_path: PathWithSource) -> Result<Self> {
        let style = (&raw_config.style).into();
        let display = (&raw_config.display).into();
        let search: SearchConfig<'a> = (&raw_config.search).into();

        let updates = UpdatesConfig {
            auto_update: raw_config.updates.auto_update,
            auto_update_interval: Duration::from_secs(
                raw_config.updates.auto_update_interval_hours * 3600,
            ),
            archive_source: &raw_config.updates.archive_source,
            tls_backend: raw_config.updates.tls_backend.try_into()?,
            download_languages: raw_config.updates.download_languages.as_ref().map_or_else(
                || search.languages.clone(),
                |languages| languages.iter().map(|lang| Language(lang)).collect(),
            ),
            warn_cache_age: match raw_config.updates.warn_cache_age {
                None => Some(MAX_CACHE_AGE),
                Some(RawWarnCacheAge::Never) => None,
            },
        };

        let relative_path_root = config_file_path
            .path()
            .parent()
            .context("Failed to get config directory")?;
        let home_path = env::home_dir();

        // Determine directories config. For this, we need to take some
        // additional factory into account, like env variables, or the
        // user config.
        let cache_dir_env_var = "TEALDEER_CACHE_DIR";
        let cache_dir = if let Ok(env_var) = env::var(cache_dir_env_var) {
            // For backwards compatibility reasons, the cache directory can be
            // overridden using an env variable. This is deprecated and will be
            // phased out in the future.
            eprintln!(
                "Warning: The ${cache_dir_env_var} env variable is deprecated, use the `cache_dir` option in the config file instead."
            );
            PathWithSource {
                path: PathBuf::from(env_var),
                source: PathSource::EnvVar,
            }
        } else if let Some(config_value) = &raw_config.directories.cache_dir {
            // Resolve possible ~ prefixed path
            let expanded_path = expand_home(config_value, home_path.as_deref())?;
            // Resolve possible relative path.
            let resolved_path = relative_path_root.join(expanded_path);

            PathWithSource {
                path: resolved_path,
                source: PathSource::ConfigFile,
            }
        } else {
            PathWithSource {
                path: SYSTEM_DIRECTORIES.cache.clone(),
                source: PathSource::OsConvention,
            }
        };
        let custom_pages_dir = raw_config
            .directories
            .custom_pages_dir
            .as_ref()
            .map(|path| -> Result<PathWithSource> {
                // Resolve possible ~ prefixed path
                let expanded_path = expand_home(path, home_path.as_deref())?;
                // Resolve possible relative path.
                let resolved_path = relative_path_root.join(expanded_path);

                Ok(PathWithSource {
                    path: resolved_path,
                    source: PathSource::ConfigFile,
                })
            })
            .transpose()?
            .or_else(|| {
                // Note: The `join("")` call ensures that there's a trailing slash
                Some(PathWithSource {
                    path: SYSTEM_DIRECTORIES.data.join("pages").join(""),
                    source: PathSource::OsConvention,
                })
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
            search,
            file_path: config_file_path,
        })
    }
}

/// Expands tilde (~) prefixed directories into its absolute version
fn expand_home<'a>(input_path: &'a Path, home_path: Option<&Path>) -> Result<Cow<'a, Path>> {
    let mut components = input_path.components();

    if let Some(Component::Normal(first_component_raw)) = components.next() {
        let first_component = first_component_raw
            .to_str()
            .ok_or(anyhow!("Path contains invalid UTF-8"))?;

        if first_component == "~" {
            let home_path = home_path.ok_or(anyhow!("Unable to find user home directory"))?;
            let rest: PathBuf = components.collect();
            let expanded = home_path.join(rest);

            return Ok(Cow::Owned(expanded));
        } else if first_component.starts_with('~') {
            return Err(anyhow!("Tilde expansion with a login name not supported"));
        }
    }

    Ok(Cow::Borrowed(input_path))
}

/// The [`ConfigLoader`] is used to load a [`Config`] from a file.
///
/// Since the rich [`Config`] keeps references to [`RawConfig`], the raw config needs to be kept alive outside of the
/// [`Config`]. The [`ConfigLoader`] thus offers the following flow:
/// 1. Read a raw config using [`ConfigLoader::read`] or [`ConfigLoader::read_default_path`].
/// 2. Validate the contents to a [`Config`] that borrows the [`ConfigLoader`].
pub struct ConfigLoader {
    raw: RawConfig,
    path: PathWithSource,
}

impl ConfigLoader {
    fn read_internal(
        path: PathWithSource,
        allow_not_found: bool,
        overrides: &[String],
    ) -> Result<Self> {
        let read_raw_config = match fs::read_to_string(&path.path) {
            Ok(content) => toml::from_str(&content).with_context(|| {
                format!(
                    "Could not parse config file contents as toml from {}.",
                    path.path.display()
                )
            })?,
            Err(e) if allow_not_found && e.kind() == ErrorKind::NotFound => RawConfig::default(),
            Err(e) => {
                return Err(e).context(format!(
                    "Could not read config file contents from {}.",
                    path.path().display()
                ));
            }
        };

        let read_config_table = toml::Table::try_from(read_raw_config)?;
        let used_config_table = Self::override_config_with(read_config_table, overrides)
            .context("Failed to apply config overrides")?;
        let raw = used_config_table.try_into()?;

        Ok(Self { raw, path })
    }

    fn override_config_with(
        config_table: toml::Table,
        overrides: &[String],
    ) -> Result<toml::Table> {
        let mut config_table = toml::Value::Table(config_table);
        for override_str in overrides {
            let (name, value) = override_str
                .split_once('=')
                .ok_or(anyhow!("Invalid override-string: {override_str} (correct example: \"display.compact = true\")"))?;

            let name = name.trim();
            let value = toml::Value::from_str(value.trim())?;

            let mut entry = &mut config_table;
            for subkey in name.split('.') {
                let toml::Value::Table(entry_table) = entry else {
                    bail!(
                        "\"{name}\" is not a valid identifier since \"{subkey}\" already refers to a value which is not a toml-Table."
                    );
                };

                entry = entry_table
                    .entry(subkey)
                    .or_insert(toml::Value::Table(Map::default()));
            }

            *entry = value;
        }

        match config_table {
            toml::Value::Table(config_table) => Ok(config_table),
            _ => unreachable!("root table is never modified"),
        }
    }

    /// Create a loader that uses the config at `path`.
    /// `overrides`: If set, overrides the default values of the config
    pub fn read(path: PathBuf, overrides: &[String]) -> Result<Self> {
        Self::read_internal(
            PathWithSource {
                path,
                source: PathSource::Cli,
            },
            false,
            overrides,
        )
    }

    /// Create a loader that uses the default config file location. If no file is present at the default location, the
    /// default configuration is used.
    /// `overrides`: If set, overrides the default values of the config
    pub fn read_default_path(overrides: &[String]) -> Result<Self> {
        let path = get_default_config_path();
        Self::read_internal(path, true, overrides)
    }

    /// Parse the read [`RawConfig`] into a [`Config`].
    pub fn load(&self) -> Result<Config<'_>> {
        Config::from_raw(&self.raw, self.path.clone())
            .context("Could not process raw config into rich config")
    }
}

/// Return the path to the config directory.
///
/// The config dir path can be overridden using the `TEALDEER_CONFIG_DIR` env
/// variable. Otherwise, the user config directory is returned.
///
/// Note that this function does not verify whether the directory at that
/// location exists, or is a directory.
pub fn get_config_dir() -> (PathBuf, PathSource) {
    // Allow overriding the config directory by setting the
    // $TEALDEER_CONFIG_DIR env variable.
    if let Ok(value) = env::var("TEALDEER_CONFIG_DIR") {
        return (PathBuf::from(value), PathSource::EnvVar);
    }

    (SYSTEM_DIRECTORIES.config.clone(), PathSource::OsConvention)
}

/// Return the path to the config file.
///
/// Note that this function does not verify whether the file at that location
/// exists, or is a file.
pub fn get_default_config_path() -> PathWithSource {
    let (mut path, source) = get_config_dir();
    path.push(CONFIG_FILE_NAME);
    PathWithSource { path, source }
}

/// Create default config file.
/// path: Can be specified to create the config in that path instead of
/// the default path.
pub fn make_default_config(path: Option<&Path>) -> Result<PathBuf> {
    let config_file_path = if let Some(p) = path {
        p.into()
    } else {
        let (config_dir, _) = get_config_dir();

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

        config_dir.join(CONFIG_FILE_NAME)
    };

    // Ensure that a config file doesn't get overwritten
    ensure!(
        !config_file_path.is_file(),
        "A configuration file already exists at {}, no action was taken.",
        config_file_path.to_str().unwrap()
    );

    // Create default config
    let serialized_config =
        toml::to_string(&RawConfig::default()).context("Failed to serialize default config")?;

    // Write default config
    let mut config_file =
        File::create(&config_file_path).context("Could not create config file")?;
    let _wc = config_file
        .write(serialized_config.as_bytes())
        .context("Could not write to config file")?;

    Ok(config_file_path)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serialize_deserialize() {
        let raw_config = RawConfig::default();
        let serialized = toml::to_string(&raw_config).unwrap();
        let deserialized: RawConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(raw_config, deserialized);
    }

    #[test]
    fn expand_path_with_valid_home() {
        let home = Some(PathBuf::from("/foo/bar"));
        let path_to_expand = PathBuf::from("~/baz");

        assert_eq!(
            *expand_home(&path_to_expand, home.as_deref()).unwrap(),
            PathBuf::from("/foo/bar/baz")
        );
    }

    #[test]
    fn expand_path_with_absolute_path() {
        let home = Some(PathBuf::from("/foo/bar"));
        let dir_to_expand = PathBuf::from("/one/two");

        assert_eq!(
            *expand_home(&dir_to_expand, home.as_deref()).unwrap(),
            dir_to_expand
        );
    }

    #[test]
    fn error_with_tilde_username() {
        let home = Some(PathBuf::from("/foo/bar"));
        let dir_to_expand = PathBuf::from("~baz/foo");

        assert!(expand_home(&dir_to_expand, home.as_deref()).is_err());
    }

    #[test]
    fn expand_tilde_in_config_file() {
        let mut raw_config = RawConfig::default();
        raw_config.directories.cache_dir = Some("~/my/custom_cache".into());
        raw_config.directories.custom_pages_dir = Some("~/custom_pages".into());

        let config = Config::from_raw(
            &raw_config,
            PathWithSource {
                path: PathBuf::from("/path/to/config/config.toml"),
                source: PathSource::OsConvention,
            },
        )
        .unwrap();

        let home_dir = env::home_dir().unwrap();

        assert_eq!(
            config.directories.cache_dir.path(),
            home_dir.join("my/custom_cache")
        );
        assert_eq!(
            config.directories.custom_pages_dir.unwrap().path(),
            home_dir.join("custom_pages")
        );
    }

    #[test]
    fn relative_path_resolution() {
        let mut raw_config = RawConfig::default();
        raw_config.directories.cache_dir = Some("../cache".into());
        raw_config.directories.custom_pages_dir = Some("../custom_pages".into());

        let config = Config::from_raw(
            &raw_config,
            PathWithSource {
                path: PathBuf::from("/path/to/config/config.toml"),
                source: PathSource::OsConvention,
            },
        )
        .unwrap();

        assert_eq!(
            config.directories.cache_dir.path(),
            Path::new("/path/to/config/../cache")
        );
        assert_eq!(
            config.directories.custom_pages_dir.unwrap().path(),
            Path::new("/path/to/config/../custom_pages")
        );
    }

    mod override_config {
        use super::*;
        use toml::Value;

        fn base_config() -> toml::Table {
            toml::Table::from_str(
                "
            global_value = false

            [some]
            value = 0

            [some.inner]
            value1 = 1
            value2 = \"a string\"

            [some.other]
            value1 = 3
            value2 = [ 1, \"text\", true ]
            ",
            )
            .unwrap()
        }

        #[test]
        fn basic() {
            let original_config = base_config();
            let overrides = &["some.inner.value1 = 'some text'".to_string()];

            let new_config = ConfigLoader::override_config_with(original_config, overrides)
                .expect("config should be successfully overwritten");

            assert_eq!(new_config["some"]["value"], Value::Integer(0));
            assert_eq!(
                new_config["some"]["inner"]["value1"],
                Value::String("some text".to_string()),
            );
            assert_eq!(
                new_config["some"]["inner"]["value2"],
                Value::String("a string".to_string()),
            );
            assert_eq!(new_config["some"]["other"]["value1"], Value::Integer(3));
            assert_eq!(new_config["global_value"], Value::Boolean(false));
        }

        macro_rules! style_config_with {
            ($config:ident, $overrides:expr) => {
                let loader = ConfigLoader::read(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/style-config.toml"),
                    $overrides,
                )
                .unwrap();
                let $config = loader.load().unwrap();
            };
        }

        #[test]
        fn dependent_config() {
            style_config_with!(config, &["search.languages = ['de', 'it']".to_string()]);
            assert_eq!(config.search.languages, [Language("de"), Language("it")]);
            // Value is copied after override is applied
            assert_eq!(
                config.updates.download_languages,
                [Language("de"), Language("it")]
            );
        }

        #[test]
        fn order() {
            style_config_with!(
                config,
                &[
                    "display.compact = false".to_string(),
                    "display.compact = true".to_string(),
                ]
            );
            assert!(config.display.compact);
        }

        #[test]
        fn override_with_table() {
            style_config_with!(config, &["display = {'compact' = true}".to_string()]);
            assert!(config.display.compact);
            assert_eq!(
                config.display.indent,
                RawConfig::default().display.indent.into()
            );
        }
    }

    mod language {
        use super::*;

        #[test]
        fn missing_lang_env() {
            let lang_list = get_languages(None, Some("de:fr"));
            assert_eq!(lang_list, [Language("en")]);
            let lang_list = get_languages(None, None);
            assert_eq!(lang_list, [Language("en")]);
        }

        #[test]
        fn missing_language_env() {
            let lang_list = get_languages(Some("de"), None);
            assert_eq!(lang_list, [Language("de"), Language("en")]);
        }

        #[test]
        fn preference_order() {
            let lang_list = get_languages(Some("de"), Some("fr:cn"));
            assert_eq!(
                lang_list,
                [
                    Language("fr"),
                    Language("cn"),
                    Language("de"),
                    Language("en")
                ]
            );
        }

        #[test]
        fn country_code_expansion() {
            let lang_list = get_languages(Some("pt_BR"), None);
            assert_eq!(
                lang_list,
                [Language("pt_BR"), Language("pt"), Language("en")]
            );
        }

        #[test]
        fn with_encoding() {
            let lang_list = get_languages(Some("de_DE.UTF-8"), None);
            assert_eq!(
                lang_list,
                [Language("de_DE"), Language("de"), Language("en")]
            );
        }

        #[test]
        fn ignore_posix_and_c() {
            let lang_list = get_languages(Some("POSIX"), None);
            assert_eq!(lang_list, [Language("en")]);
            let lang_list = get_languages(Some("C"), None);
            assert_eq!(lang_list, [Language("en")]);
        }

        #[test]
        fn no_duplicates() {
            let lang_list = get_languages(Some("de"), Some("fr:de:cn:de"));
            assert_eq!(
                lang_list,
                [
                    Language("fr"),
                    Language("de"),
                    Language("cn"),
                    Language("en")
                ]
            );
        }
    }
}
