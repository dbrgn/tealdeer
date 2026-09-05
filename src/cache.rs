use std::{
    fs::{self, File},
    io::{Cursor, ErrorKind, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use log::{debug, info};
use ureq::{
    Agent,
    http::StatusCode,
    tls::{RootCerts, TlsConfig, TlsProvider},
};
use zip::ZipArchive;

use crate::{
    config::{Language, TlsBackend},
    types::PlatformType,
};

pub static TLDR_PAGES_DIR: &str = "tldr-pages";

#[derive(Clone)]
pub struct CacheConfig<'a> {
    pub pages_directory: &'a Path,
    pub custom_pages_directory: Option<&'a Path>,
    pub platforms: &'a [PlatformType],
    pub search_languages: &'a [Language<'a>],
    pub download_languages: &'a [Language<'a>],
}

/// The directory backing this cache is checked to be populated at construction.
pub struct Cache<'a> {
    config: CacheConfig<'a>,
}

#[derive(Debug)]
pub struct PageLookupResult {
    pub page_path: PathBuf,
    pub patch_path: Option<PathBuf>,
}

impl<'a> Cache<'a> {
    /// Try opening a cache at the location given by `config.pages_directory`. If no directory
    /// exists at this location, `Ok(None)` is returned.
    pub fn open(config: CacheConfig<'a>) -> Result<Option<Self>> {
        match config.pages_directory.metadata() {
            Ok(md) => {
                ensure!(
                    md.is_dir(),
                    "Cache directory `{}` exists, but is not a directory.",
                    config.pages_directory.display(),
                );
                Ok(Some(Cache { config }))
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(anyhow!(err).context(format!(
                "Error getting metdata of cache directory {}",
                config.pages_directory.display()
            ))),
        }
    }

    /// Open an existing cache at `config.pages_directory` or create one if no cache resides at
    /// this location. In case of success, the return value is a tuple with the `Cache` and a
    /// boolean indicating whether the cache was newly created.
    pub fn open_or_create(config: CacheConfig<'a>, quietly: bool) -> Result<(Self, bool)> {
        if let Some(cache) = Self::open(config.clone())? {
            return Ok((cache, false));
        }

        fs::create_dir_all(config.pages_directory).with_context(|| {
            format!(
                "Cache directory `{}` cannot be created",
                config.pages_directory.display(),
            )
        })?;
        if !quietly {
            eprintln!(
                "Successfully created cache directory `{}`.",
                config.pages_directory.display(),
            );
        }

        Ok((Cache { config }, true))
    }

    pub fn age(&self) -> Result<Duration> {
        let mtime = self.config.pages_directory.metadata()?.modified()?;
        SystemTime::now()
            .duration_since(mtime)
            .context("Error comparing cache mtime with current time")
    }

    pub fn find_page(&self, command: &str) -> Option<PageLookupResult> {
        let page_filename = format!("{command}.md");
        let patch_filename = format!("{command}.patch.md");
        let custom_filename = format!("{command}.page.md");

        if let Some(custom_pages_dir) = self.config.custom_pages_directory {
            let custom_page = custom_pages_dir.join(custom_filename);
            if custom_page.is_file() {
                return Some(PageLookupResult::with_page(custom_page));
            }
        }

        let patch_path = self
            .config
            .custom_pages_directory
            .map(|dir| dir.join(&patch_filename))
            .filter(|path| path.is_file());

        for &platform in self.config.platforms {
            for language in self.config.search_languages {
                let mut search_path = self.config.pages_directory.to_path_buf();
                search_path.push(language.directory_name());
                search_path.push(platform.directory_name());
                search_path.push(&page_filename);

                if search_path.is_file() {
                    return Some(
                        PageLookupResult::with_page(search_path).with_optional_patch(patch_path),
                    );
                }
            }
        }

        None
    }

    pub fn list_pages(&self) -> Result<impl IntoIterator<Item = String> + use<>> {
        let mut pages = Vec::new();

        let mut append_all = |directory: &Path, suffix: &str| -> Result<()> {
            let Ok(file_iter) = fs::read_dir(directory) else {
                return Ok(());
            };

            for entry in file_iter {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    let mut page_path = entry
                        .file_name()
                        .into_string()
                        .map_err(|_| anyhow!("Found invalid filename: {:?}", entry.path()))?;

                    if page_path.ends_with(suffix) {
                        page_path.truncate(page_path.len() - suffix.len());
                        pages.push(page_path);
                    } else {
                        debug!(
                            "Skipping page entry not ending in \".md\": {:?}",
                            entry.path(),
                        );
                    }
                }
            }

            Ok(())
        };

        let mut search_path = self.config.pages_directory.to_path_buf();
        for language in self.config.search_languages {
            search_path.push(language.directory_name());
            for platform in self.config.platforms {
                search_path.push(platform.directory_name());
                append_all(&search_path, ".md")?;
                search_path.pop();
            }
            search_path.pop();
        }

        if let Some(custom_pages_dir) = self.config.custom_pages_directory {
            append_all(custom_pages_dir, ".page.md")?;
        }

        pages.sort_unstable();
        pages.dedup();
        Ok(pages)
    }

    pub fn old_custom_pages_exist(&self) -> Result<bool> {
        let Some(directory) = self.config.custom_pages_directory else {
            return Ok(false);
        };
        let Ok(file_iter) = fs::read_dir(directory) else {
            return Ok(false);
        };

        for entry in file_iter {
            if let Some(extension) = entry?.path().extension()
                && (extension == "page" || extension == "patch")
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn clear(self) -> Result<()> {
        fs::remove_dir_all(self.config.pages_directory).with_context(|| {
            format!(
                "Could not remove pages directory at {}",
                self.config.pages_directory.display(),
            )
        })
    }

    /// Download archives for the languages in `self.config().download_languages` and replace the
    /// pages directory with the newly downloaded pages. As not all languages might have pages
    /// available (for example, `en_US` instead of `en`), an iterator yielding all languages which
    /// were successfully downloaded is returned.
    pub fn update(
        &mut self,
        archive_url: &str,
        tls_backend: TlsBackend,
    ) -> Result<impl IntoIterator<Item = Language<'_>> + use<'_>> {
        let client = Self::build_client(tls_backend);

        // Download everything before deleting anything
        let mut archives = self
            .config
            .download_languages
            .iter()
            .map(|&lang| {
                Ok((
                    lang,
                    Self::download(
                        &client,
                        &format!("{archive_url}/tldr-{}.zip", lang.directory_name()),
                    )?
                    .map(|bytes| ZipArchive::new(Cursor::new(bytes)))
                    .transpose()?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        // Clear cache directory
        // Note: This is not the best solution. Ideally we would download the
        // archive to a temporary directory and then swap the two directories.
        // But renaming a directory doesn't work across filesystems and Rust
        // does not yet offer a recursive directory copying function. So for
        // now, we'll use this approach.
        fs::remove_dir_all(self.config.pages_directory)?;
        fs::create_dir(self.config.pages_directory)?;

        for (lang, archive) in &mut archives {
            if let Some(archive) = archive {
                info!("Extracting archive for {lang:?}");
                archive.extract(self.config.pages_directory.join(lang.directory_name()))?;
            } else {
                info!("No archive found for {lang:?}");
            }
        }

        Ok(archives
            .into_iter()
            .filter_map(|(lang, archive)| archive.is_some().then_some(lang)))
    }

    pub fn config(&self) -> &CacheConfig<'a> {
        &self.config
    }
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

    /// Create a reader that sequentially reads from the page and the
    /// patch, as if they were concatenated.
    ///
    /// This will return an error if either the page file or the patch file
    /// cannot be opened.
    pub fn reader(&self) -> Result<Box<dyn Read>> {
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
        Ok(if let Some(patch_file) = patch_file_opt {
            Box::new(page_file.chain(&b"\n"[..]).chain(patch_file)) as Box<dyn Read>
        } else {
            Box::new(page_file) as Box<dyn Read>
        })
    }
}

impl Language<'_> {
    fn directory_name(&self) -> String {
        format!("pages.{}", self.0)
    }
}

impl PlatformType {
    fn directory_name(self) -> &'static str {
        match self {
            PlatformType::Linux => "linux",
            PlatformType::OsX => "osx",
            PlatformType::SunOs => "sunos",
            PlatformType::Windows => "windows",
            PlatformType::Android => "android",
            PlatformType::FreeBsd => "freebsd",
            PlatformType::NetBsd => "netbsd",
            PlatformType::OpenBsd => "openbsd",
            PlatformType::Common => "common",
        }
    }
}

impl Cache<'_> {
    fn build_client(tls_backend: TlsBackend) -> Agent {
        let tls_builder = match tls_backend {
            #[cfg(feature = "native-tls")]
            TlsBackend::NativeTls => TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier),
            #[cfg(feature = "rustls-with-webpki-roots")]
            TlsBackend::RustlsWithWebpkiRoots => TlsConfig::builder()
                .provider(TlsProvider::Rustls)
                .root_certs(RootCerts::WebPki),
            #[cfg(feature = "rustls-with-native-roots")]
            TlsBackend::RustlsWithNativeRoots => TlsConfig::builder()
                .provider(TlsProvider::Rustls)
                .root_certs(RootCerts::PlatformVerifier),
        };
        let config = Agent::config_builder()
            .http_status_as_error(false) // because we want to handle them
            .tls_config(tls_builder.build())
            .build();

        config.into()
    }

    /// Download the archive from the specified URL.
    fn download(client: &Agent, archive_url: &str) -> Result<Option<Vec<u8>>> {
        info!("Downloading archive from {archive_url}");
        let response = client.get(archive_url).call();
        match response {
            Ok(response) if response.status().is_success() => {
                let mut buf: Vec<u8> = Vec::new();
                response.into_body().into_reader().read_to_end(&mut buf)?;
                debug!("{} bytes downloaded", buf.len());
                Ok(Some(buf))
            }
            Ok(response) if response.status() == StatusCode::NOT_FOUND => Ok(None),
            _ => {
                bail!("Could not download tldr pages from {archive_url}: {response:?}")
            }
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
