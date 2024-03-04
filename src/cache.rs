use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{ensure, Context, Result};
use log::debug;
use reqwest::{blocking::Client, Proxy};
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;

use crate::{types::PlatformType, utils::print_warning};

pub static TLDR_PAGES_DIR: &str = "tldr-pages";
static TLDR_OLD_PAGES_DIR: &str = "tldr-master";

#[derive(Debug)]
pub struct Cache {
    cache_dir: PathBuf,
    enable_styles: bool,
}

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
            .with_context(|| format!("Could not open page file at {}", self.page_path.display()))?;

        // Open patch file
        let patch_file_opt = match &self.patch_path {
            Some(path) => Some(
                File::open(path)
                    .with_context(|| format!("Could not open patch file at {}", path.display()))?,
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

impl Cache {
    pub fn new<P>(cache_dir: P, enable_styles: bool) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            cache_dir: cache_dir.into(),
            enable_styles,
        }
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Make sure that the cache directory exists and is a directory.
    /// If necessary, create the directory.
    fn ensure_cache_dir_exists(&self) -> Result<()> {
        // Check whether `cache_dir` exists and is a directory
        let (cache_dir_exists, cache_dir_is_dir) = self
            .cache_dir
            .metadata()
            .map_or((false, false), |md| (true, md.is_dir()));
        ensure!(
            !cache_dir_exists || cache_dir_is_dir,
            "Cache directory path `{}` is not a directory",
            self.cache_dir.display(),
        );

        if !cache_dir_exists {
            // If missing, try to create the complete directory path
            fs::create_dir_all(&self.cache_dir).with_context(|| {
                format!(
                    "Cache directory path `{}` cannot be created",
                    self.cache_dir.display(),
                )
            })?;
            eprintln!(
                "Successfully created cache directory path `{}`.",
                self.cache_dir.display(),
            );
        }

        Ok(())
    }

    fn pages_dir(&self) -> PathBuf {
        self.cache_dir.join(TLDR_PAGES_DIR)
    }

    /// Download the archive from the specified URL.
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
            .with_context(|| format!("Could not download tldr pages from {archive_url}"))?;
        let mut buf: Vec<u8> = vec![];
        let bytes_downloaded = resp.copy_to(&mut buf)?;
        debug!("{} bytes downloaded", bytes_downloaded);
        Ok(buf)
    }

    /// Update the pages cache from the specified URL.
    pub fn update(&self, archive_url: &str) -> Result<()> {
        self.ensure_cache_dir_exists()?;

        // First, download the compressed data
        let bytes: Vec<u8> = Self::download(archive_url)?;

        // Decompress the response body into an `Archive`
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .context("Could not decompress downloaded ZIP archive")?;

        // Clear cache directory
        // Note: This is not the best solution. Ideally we would download the
        // archive to a temporary directory and then swap the two directories.
        // But renaming a directory doesn't work across filesystems and Rust
        // does not yet offer a recursive directory copying function. So for
        // now, we'll use this approach.
        self.clear()
            .context("Could not clear the cache directory")?;

        // Extract archive into pages dir
        archive
            .extract(&self.pages_dir())
            .context("Could not unpack compressed data")?;

        Ok(())
    }

    /// Return the duration since the cache directory was last modified.
    pub fn last_update(&self) -> Option<Duration> {
        if let Ok(metadata) = fs::metadata(self.pages_dir()) {
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
    fn get_platform_dir(platform: PlatformType) -> &'static str {
        match platform {
            PlatformType::Linux => "linux",
            PlatformType::OsX => "osx",
            PlatformType::SunOs => "sunos",
            PlatformType::Windows => "windows",
            PlatformType::Android => "android",
            PlatformType::FreeBSD => "freebsd",
            PlatformType::NetBSD => "netbsd",
            PlatformType::OpenBSD => "openbsd",
        }
    }

    /// Check for pages for a given platform in one of the given languages.
    fn find_page_for_platform(
        page_name: &str,
        pages_dir: &Path,
        platform: &str,
        language_dirs: &[String],
    ) -> Option<PathBuf> {
        language_dirs
            .iter()
            .map(|lang_dir| pages_dir.join(lang_dir).join(platform).join(page_name))
            .find(|path| path.exists() && path.is_file())
    }

    /// Look up custom patch (<name>.patch.md). If it exists, store it in a variable.
    fn find_patch(patch_name: &str, custom_pages_dir: Option<&Path>) -> Option<PathBuf> {
        custom_pages_dir
            .map(|custom_dir| custom_dir.join(patch_name))
            .filter(|path| path.exists() && path.is_file())
    }

    /// Search for a page and return the path to it.
    pub fn find_page(
        &self,
        name: &str,
        languages: &[String],
        custom_pages_dir: Option<&Path>,
        platforms: &[PlatformType],
    ) -> Option<PageLookupResult> {
        let page_filename = format!("{name}.md");
        let patch_filename = format!("{name}.patch.md");
        let custom_filename = format!("{name}.page.md");

        // Determine directory paths
        let pages_dir = self.pages_dir();
        let lang_dirs: Vec<String> = languages
            .iter()
            .map(|lang| {
                if lang == "en" {
                    String::from("pages")
                } else {
                    format!("pages.{lang}")
                }
            })
            .collect();

        // Look up custom page (<name>.page.md). If it exists, return it directly
        if let Some(config_dir) = custom_pages_dir {
            // TODO: Remove this check 1 year after version 1.7.0 was released
            self.check_for_old_custom_pages(config_dir);

            let custom_page = config_dir.join(custom_filename);
            if custom_page.exists() && custom_page.is_file() {
                return Some(PageLookupResult::with_page(custom_page));
            }
        }

        let patch_path = Self::find_patch(&patch_filename, custom_pages_dir);

        // Try to find a platform specific path next, in the order supplied by the user, and append custom patch to it.
        for &platform in platforms {
            let platform_dir = Cache::get_platform_dir(platform);
            if let Some(page) =
                Self::find_page_for_platform(&page_filename, &pages_dir, platform_dir, &lang_dirs)
            {
                return Some(PageLookupResult::with_page(page).with_optional_patch(patch_path));
            }
        }

        // Did not find platform specific results, fall back to "common"
        Self::find_page_for_platform(&page_filename, &pages_dir, "common", &lang_dirs)
            .map(|page| PageLookupResult::with_page(page).with_optional_patch(patch_path))
    }

    /// Return the available pages.
    pub fn list_pages(
        &self,
        custom_pages_dir: Option<&Path>,
        platforms: &[PlatformType],
    ) -> Vec<String> {
        // Determine platforms directory and platform
        let platforms_dir = self.pages_dir().join("pages");
        let platform_dirs: Vec<&'static str> = platforms
            .iter()
            .map(|&p| Self::get_platform_dir(p))
            .collect();

        // Closure that allows the WalkDir instance to traverse platform
        // specific and common page directories, but not others.
        let should_walk = |entry: &DirEntry| -> bool {
            let file_type = entry.file_type();
            let file_name = match entry.file_name().to_str() {
                Some(name) => name,
                None => return false,
            };
            if file_type.is_dir() {
                return file_name == "common" || platform_dirs.contains(&file_name);
            } else if file_type.is_file() {
                return true;
            }
            false
        };

        let to_stem = |entry: DirEntry| -> Option<String> {
            entry
                .path()
                .file_stem()
                .and_then(OsStr::to_str)
                .map(str::to_string)
        };

        let to_stem_custom = |entry: DirEntry| -> Option<String> {
            entry
                .path()
                .file_name()
                .and_then(OsStr::to_str)
                .and_then(|s| s.strip_suffix(".page.md"))
                .map(str::to_string)
        };

        // Recursively walk through common and (if applicable) platform specific directory
        let mut pages = WalkDir::new(platforms_dir)
            .min_depth(1) // Skip root directory
            .into_iter()
            .filter_entry(should_walk) // Filter out pages for other architectures
            .filter_map(Result::ok) // Convert results to options, filter out errors
            .filter_map(|e| {
                let extension = e.path().extension().unwrap_or_default();
                if e.file_type().is_file() && extension == "md" {
                    to_stem(e)
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();

        if let Some(custom_pages_dir) = custom_pages_dir {
            let is_page = |entry: &DirEntry| -> bool {
                entry.file_type().is_file()
                    && entry
                        .path()
                        .file_name()
                        .and_then(OsStr::to_str)
                        .map_or(false, |file_name| file_name.ends_with(".page.md"))
            };

            let custom_pages = WalkDir::new(custom_pages_dir)
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .filter_entry(is_page)
                .filter_map(Result::ok)
                .filter_map(to_stem_custom);

            pages.extend(custom_pages);
        }

        pages.sort();
        pages.dedup();
        pages
    }

    /// Delete the cache directory
    ///
    /// Returns true if the cache was deleted and false if the cache dir did
    /// not exist.
    pub fn clear(&self) -> Result<bool> {
        if !self.cache_dir.exists() {
            return Ok(false);
        }
        ensure!(
            self.cache_dir.is_dir(),
            "Cache path ({}) is not a directory.",
            self.cache_dir.display(),
        );

        // Delete old tldr-pages cache location as well if present
        // TODO: To be removed in the future
        for pages_dir_name in [TLDR_PAGES_DIR, TLDR_OLD_PAGES_DIR] {
            let pages_dir = self.cache_dir.join(pages_dir_name);

            if pages_dir.exists() {
                fs::remove_dir_all(&pages_dir).with_context(|| {
                    format!(
                        "Could not remove the cache directory at {}",
                        pages_dir.display()
                    )
                })?;
            }
        }

        Ok(true)
    }

    /// Check for old custom pages (without .md suffix) and print a warning.
    fn check_for_old_custom_pages(&self, custom_pages_dir: &Path) {
        let old_custom_pages_exist = WalkDir::new(custom_pages_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_entry(|entry| entry.file_type().is_file())
            .any(|entry| {
                if let Ok(entry) = entry {
                    let extension = entry.path().extension();
                    if let Some(extension) = extension {
                        extension == "page" || extension == "patch"
                    } else {
                        false
                    }
                } else {
                    false
                }
            });
        if old_custom_pages_exist {
            print_warning(
                self.enable_styles,
                &format!(
                    "Custom pages using the old naming convention were found in {}.\n\
                     Please rename them to follow the new convention:\n\
                     - `<name>.page` → `<name>.page.md`\n\
                     - `<name>.patch` → `<name>.patch.md`",
                    custom_pages_dir.display()
                ),
            );
        }
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
        let page_path = dir.path().join("test.page.md");
        let patch_path = dir.path().join("test.patch.md");
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
        let page_path = dir.path().join("test.page.md");
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
