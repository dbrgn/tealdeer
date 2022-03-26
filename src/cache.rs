use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{ensure, Context, Result};
use app_dirs::{get_app_root, AppDataType};
use log::{debug, info};
use reqwest::{blocking::Client, Proxy};
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;

use crate::{
    config::Config,
    types::{PathSource, PlatformType},
};

pub(crate) static CACHE_DIR_ENV_VAR: &str = "TEALDEER_CACHE_DIR";

pub static TLDR_PAGES_DIR: &str = "tldr-pages";
static TLDR_OLD_PAGES_DIR: &str = "tldr-master";

#[derive(Debug)]
pub struct PageLookupResult {
    pub page_path: PathBuf,
    pub patch_path: Option<PathBuf>,
}

impl PageLookupResult {
    pub fn with_page(page_path: PathBuf) -> Self {
        Self {
            page_path,
            patch_path: None,
        }
    }

    pub fn with_optional_patch(mut self, patch_path: Option<PathBuf>) -> Self {
        self.patch_path = patch_path;
        self
    }

    /// Create a buffered reader that sequentially reads from the page and the
    /// patch, as if they were concatenated.
    ///
    /// This will return an error if either the page file or the patch file
    /// cannot be opened.
    pub fn reader(&self) -> Result<BufReader<Box<dyn Read>>> {
        // Open page file
        let page_file = File::open(&self.page_path)
            .with_context(|| format!("Could not open page file at {:?}", self.page_path))?;

        // Open patch file
        let patch_file_opt = match &self.patch_path {
            Some(path) => Some(
                File::open(path)
                    .with_context(|| format!("Could not open patch file at {:?}", path))?,
            ),
            None => None,
        };

        // Create chained reader from file(s)
        //
        // Note: It might be worthwhile to create our own struct that accepts
        // the page and patch files and that will read them sequentially,
        // because it avoids the boxing below. However, the performance impact
        // would first need to be shown to be significant using a benchmark.
        Ok(BufReader::new(if let Some(patch_file) = patch_file_opt {
            Box::new(page_file.chain(&b"\n"[..]).chain(patch_file)) as Box<dyn Read>
        } else {
            Box::new(page_file) as Box<dyn Read>
        }))
    }
}

pub enum CacheFreshness {
    /// The cache is still fresh (less than MAX_CACHE_AGE old)
    Fresh,
    /// The cache is stale and should be updated
    Stale(Duration),
    /// The cache is missing
    Missing,
}

#[derive(Debug)]
pub struct Cache {
    /// The base directory of the cache.
    base_path: PathBuf,
    /// Cached `PathBuf` to the directory containing extracted pages.
    pages_path: PathBuf,
}

impl Cache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            pages_path: Self::pages_path_for(&path),
            base_path: path,
        }
    }

    fn ensure_paths_exist(&self) -> Result<()> {
        let (exists, is_dir) = self
            .base_path
            .metadata()
            .map_or((false, false), |md| (true, md.is_dir()));

        ensure!(
            !exists || is_dir,
            "Cache path ({}) exists, but is not a directory!",
            self.base_path.display(),
        );

        if !exists {
            fs::create_dir_all(&self.base_path).with_context(|| {
                format!(
                    "Cache path ({}) cannot be created!",
                    self.base_path.display(),
                )
            })?;
            info!(
                "Successfully created cache directory at `{}`.",
                self.base_path.display(),
            );
        }

        Ok(())
    }

    pub(crate) fn pages_path_for(cache_path: &Path) -> PathBuf {
        cache_path.join(TLDR_PAGES_DIR)
    }

    /// Returns the base directory for the cache.
    pub fn try_determine_cache_location(config: &Config) -> Result<(PathBuf, PathSource)> {
        if let Some(path) = Self::path_from_config(config) {
            return Ok((path, PathSource::ConfigVar));
        }

        if let Some(path) = Self::path_from_env() {
            return Ok((path, PathSource::EnvVar));
        }

        let path = Self::default_path().context("Couldn't resolve cache path in any way (config file, environment variable, default location)")?;
        Ok((path, PathSource::OsConvention))
    }

    pub(crate) fn path_from_config(config: &Config) -> Option<PathBuf> {
        config.directories.cache_dir_override.clone()
    }

    pub(crate) fn path_from_env() -> Option<PathBuf> {
        env::var(CACHE_DIR_ENV_VAR).ok().map(PathBuf::from)
    }

    pub(crate) fn default_path() -> Result<PathBuf> {
        get_app_root(AppDataType::UserCache, &crate::APP_INFO)
            .context("Could not determine user cache directory")
    }

    /// Download the archive
    fn download(archive_url: &str) -> Result<Vec<u8>> {
        let mut builder = Client::builder();
        if let Ok(ref host) = env::var("HTTP_PROXY") {
            if let Ok(proxy) = Proxy::http(host) {
                builder = builder.proxy(proxy);
            }
        }
        if let Ok(ref host) = env::var("HTTPS_PROXY") {
            if let Ok(proxy) = Proxy::https(host) {
                builder = builder.proxy(proxy);
            }
        }
        let client = builder
            .build()
            .context("Could not instantiate HTTP client")?;
        let mut resp = client
            .get(archive_url)
            .send()?
            .error_for_status()
            .with_context(|| format!("Could not download tldr pages from {}", archive_url))?;
        let mut buf: Vec<u8> = vec![];
        let bytes_downloaded = resp.copy_to(&mut buf)?;
        debug!("{} bytes downloaded", bytes_downloaded);
        Ok(buf)
    }

    /// Update the pages cache.
    pub fn update(&self, archive_url: &str) -> Result<()> {
        // First, download the compressed data
        let bytes: Vec<u8> = Self::download(archive_url)?;

        // Decompress the response body into an `Archive`
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .context("Could not decompress downloaded ZIP archive")?;

        self.ensure_paths_exist()?;

        // Clear cache directory
        // Note: This is not the best solution. Ideally we would download the
        // archive to a temporary directory and then swap the two directories.
        // But renaming a directory doesn't work across filesystems and Rust
        // does not yet offer a recursive directory copying function. So for
        // now, we'll use this approach.
        self.clear()
            .context("Could not clear the cache directory")?;

        // Extract archive
        archive
            .extract(&self.pages_path)
            .context("Could not unpack compressed data")?;

        Ok(())
    }

    /// Return the duration since the cache directory was last modified.
    pub fn last_update(&self) -> Option<Duration> {
        if let Ok(metadata) = fs::metadata(&self.pages_path) {
            if let Ok(mtime) = metadata.modified() {
                let now = SystemTime::now();
                return now.duration_since(mtime).ok();
            };
        };
        None
    }

    /// Return the freshness of the cache (fresh, stale or missing).
    pub fn freshness(&self) -> CacheFreshness {
        match self.last_update() {
            Some(ago) if ago > crate::config::MAX_CACHE_AGE => CacheFreshness::Stale(ago),
            Some(_) => CacheFreshness::Fresh,
            None => CacheFreshness::Missing,
        }
    }

    /// Return the platform directory.
    fn platform_directory_name(platform: PlatformType) -> &'static str {
        match platform {
            PlatformType::Linux => "linux",
            PlatformType::OsX => "osx",
            PlatformType::SunOs => "sunos",
            PlatformType::Windows => "windows",
        }
    }

    /// Check for pages for a given platform in one of the given languages.
    fn find_page_for_platform(
        page_name: &str,
        cache_dir: &Path,
        platform: &str,
        language_dirs: &[String],
    ) -> Option<PathBuf> {
        language_dirs
            .iter()
            .map(|lang_dir| cache_dir.join(lang_dir).join(platform).join(page_name))
            .find(|path| path.exists() && path.is_file())
    }

    /// Look up custom patch (<name>.patch). If it exists, store it in a variable.
    fn find_patch(patch_name: &str, custom_pages_dir: Option<&Path>) -> Option<PathBuf> {
        custom_pages_dir
            .map(|custom_dir| custom_dir.join(patch_name))
            .filter(|path| path.exists() && path.is_file())
    }

    /// Search for a page and return the path to it.
    pub fn find_page(
        &self,
        name: &str,
        platform: PlatformType,
        languages: &[String],
        custom_pages_dir: Option<&Path>,
    ) -> Option<PageLookupResult> {
        let page_filename = format!("{}.md", name);
        let patch_filename = format!("{}.patch", name);
        let custom_filename = format!("{}.page", name);

        let lang_dirs: Vec<String> = languages
            .iter()
            .map(|lang| {
                if lang == "en" {
                    String::from("pages")
                } else {
                    format!("pages.{}", lang)
                }
            })
            .collect();

        // Look up custom page (<name>.page). If it exists, return it directly
        if let Some(config_dir) = custom_pages_dir {
            let custom_page = config_dir.join(custom_filename);
            if custom_page.exists() && custom_page.is_file() {
                return Some(PageLookupResult::with_page(custom_page));
            }
        }

        let patch_path = Self::find_patch(&patch_filename, custom_pages_dir);

        // Try to find a platform specific path next, append custom patch to it.
        let platform_dir = Self::platform_directory_name(platform);
        if let Some(page) =
            Self::find_page_for_platform(&page_filename, &self.pages_path, platform_dir, &lang_dirs)
        {
            return Some(PageLookupResult::with_page(page).with_optional_patch(patch_path));
        }

        // Did not find platform specific results, fall back to "common"
        Self::find_page_for_platform(&page_filename, &self.pages_path, "common", &lang_dirs)
            .map(|page| PageLookupResult::with_page(page).with_optional_patch(patch_path))
    }

    /// Return the available pages.
    pub fn list_pages(&self, platform: PlatformType) -> Vec<String> {
        // FIXME: wait, this doesn't respect language settings?
        let english_pages = self.pages_path.join("pages");
        let platform_dir = Self::platform_directory_name(platform);

        // Closure that allows the WalkDir instance to traverse platform
        // specific and common page directories, but not others.
        let should_walk = |entry: &DirEntry| -> bool {
            let file_type = entry.file_type();
            let file_name = match entry.file_name().to_str() {
                Some(name) => name,
                None => return false,
            };
            if file_type.is_dir() {
                return file_name == "common" || file_name == platform_dir;
            } else if file_type.is_file() {
                return true;
            }
            false
        };

        // Recursively walk through common and (if applicable) platform specific directory
        let mut pages = WalkDir::new(english_pages)
            .min_depth(1) // Skip root directory
            .into_iter()
            .filter_entry(|e| should_walk(e)) // Filter out pages for other architectures
            .filter_map(Result::ok) // Convert results to options, filter out errors
            .filter_map(|e| {
                let path = e.path();
                let extension = &path.extension().and_then(OsStr::to_str).unwrap_or("");
                if e.file_type().is_file() && extension == &"md" {
                    path.file_stem()
                        .and_then(|stem| stem.to_str().map(Into::into))
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        pages.sort();
        pages.dedup();
        pages
    }

    /// Delete the cache directory.
    pub fn clear(&self) -> Result<()> {
        let path = &self.base_path;

        // Check preconditions
        ensure!(
            path.exists(),
            "Cache path ({}) does not exist.",
            path.display(),
        );
        ensure!(
            path.is_dir(),
            "Cache path ({}) is not a directory.",
            path.display()
        );

        // Delete old tldr-pages cache location as well if present
        // TODO: To be removed in the future
        for pages_dir_name in [TLDR_PAGES_DIR, TLDR_OLD_PAGES_DIR] {
            let pages_dir = path.join(pages_dir_name);

            if pages_dir.exists() {
                fs::remove_dir_all(&pages_dir).with_context(|| {
                    format!(
                        "Could not remove the cache directory at {}",
                        pages_dir.display()
                    )
                })?;
            }
        }

        Ok(())
    }
}

/// Unit Tests for cache module
#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        fs::File,
        io::{Read, Write},
    };

    #[test]
    fn test_reader_with_patch() {
        // Write test files
        let dir = tempfile::tempdir().unwrap();
        let page_path = dir.path().join("test.page");
        let patch_path = dir.path().join("test.patch");
        {
            let mut f1 = File::create(&page_path).unwrap();
            f1.write_all(b"Hello\n").unwrap();
            let mut f2 = File::create(&patch_path).unwrap();
            f2.write_all(b"World").unwrap();
        }

        // Create chained reader from lookup result
        let lr = PageLookupResult::with_page(page_path).with_optional_patch(Some(patch_path));
        let mut reader = lr.reader().unwrap();

        // Read into a Vec
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(&buf, b"Hello\n\nWorld");
    }

    #[test]
    fn test_reader_without_patch() {
        // Write test file
        let dir = tempfile::tempdir().unwrap();
        let page_path = dir.path().join("test.page");
        {
            let mut f = File::create(&page_path).unwrap();
            f.write_all(b"Hello\n").unwrap();
        }

        // Create chained reader from lookup result
        let lr = PageLookupResult::with_page(page_path);
        let mut reader = lr.reader().unwrap();

        // Read into a Vec
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(&buf, b"Hello\n");
    }
}
