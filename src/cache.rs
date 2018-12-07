use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use reqwest::{Client, Proxy};
use flate2::read::GzDecoder;
use log::debug;
use tar::Archive;
use time;
use walkdir::{DirEntry, WalkDir};
use xdg::BaseDirectories;

use crate::error::TealdeerError::{self, CacheError, UpdateError};
use crate::types::OsType;

#[derive(Debug)]
pub struct Cache {
    url: String,
    os: OsType,
}

impl Cache {
    pub fn new<S>(url: S, os: OsType) -> Self
    where
        S: Into<String>,
    {
        Self {
            url: url.into(),
            os,
        }
    }

    /// Return the path to the cache directory.
    fn get_cache_dir(&self) -> Result<PathBuf, TealdeerError> {
        // Allow overriding the cache directory by setting the
        // $TEALDEER_CACHE_DIR env variable.
        if let Ok(value) = env::var("TEALDEER_CACHE_DIR") {
            let path = PathBuf::from(value);

            if path.exists() && path.is_dir() {
                return Ok(path);
            } else {
                return Err(CacheError(
                    "Path specified by $TEALDEER_CACHE_DIR \
                     does not exist or is not a directory."
                        .into(),
                ));
            }
        };

        // Otherwise, fall back to $XDG_CACHE_HOME/tealdeer.
        let xdg_dirs = match BaseDirectories::with_prefix(crate::NAME) {
            Ok(dirs) => dirs,
            Err(_) => return Err(CacheError("Could not determine XDG base directory.".into())),
        };
        Ok(xdg_dirs.get_cache_home())
    }

    /// Download the archive
    fn download(&self) -> Result<Vec<u8>, TealdeerError> {
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
        let client = builder.build().unwrap_or_else(|_| Client::new());
        let mut resp = client.get(&self.url).send()?;
        let mut buf: Vec<u8> = vec![];
        let bytes_downloaded = resp.copy_to(&mut buf)?;
        debug!("{} bytes downloaded", bytes_downloaded);
        Ok(buf)
    }

    /// Decompress and open the archive
    fn decompress<R: Read>(&self, reader: R) -> Archive<GzDecoder<R>> {
        Archive::new(GzDecoder::new(reader))
    }

    /// Update the pages cache.
    pub fn update(&self) -> Result<(), TealdeerError> {
        // First, download the compressed data
        let bytes: Vec<u8> = self.download()?;

        // Decompress the response body into an `Archive`
        let mut archive = self.decompress(&bytes[..]);

        // Determine paths
        let cache_dir = self.get_cache_dir()?;

        // Make sure that cache directory exists
        debug!("Ensure cache directory {:?} exists", &cache_dir);
        fs::create_dir_all(&cache_dir)
            .map_err(|e| UpdateError(format!("Could not create cache directory: {}", e)))?;

        // Clear cache directory
        // Note: This is not the best solution. Ideally we would download the
        // archive to a temporary directory and then swap the two directories.
        // But renaming a directory doesn't work across filesystems and Rust
        // does not yet offer a recursive directory copying function. So for
        // now, we'll use this approach.
        self.clear()?;

        // Extract archive
        archive
            .unpack(&cache_dir)
            .map_err(|e| UpdateError(format!("Could not unpack compressed data: {}", e)))?;

        Ok(())
    }

    #[cfg(unix)]
    /// Return the number of seconds since the cache directory was last modified.
    pub fn last_update(&self) -> Option<i64> {
        if let Ok(cache_dir) = self.get_cache_dir() {
            if let Ok(metadata) = fs::metadata(cache_dir.join("tldr-master")) {
                let mtime = metadata.mtime();
                let now = time::now_utc().to_timespec();
                return Some(now.sec - mtime);
            };
        };
        None
    }

    /// Return the platform directory.
    #[allow(clippy::match_same_arms)]
    fn get_platform_dir(&self) -> Option<&'static str> {
        match self.os {
            OsType::Linux => Some("linux"),
            OsType::OsX => Some("osx"),
            OsType::SunOs => None, // TODO: Does Rust support SunOS?
            OsType::Other => None,
        }
    }

    /// Search for a page and return the path to it.
    pub fn find_page(&self, name: &str) -> Option<PathBuf> {
        // Build page file name
        let page_filename = format!("{}.md", name);

        // Get platform dir
        let platforms_dir = match self.get_cache_dir() {
            Ok(cache_dir) => cache_dir.join("tldr-master").join("pages"),
            _ => return None,
        };

        // Determine platform
        let platform = self.get_platform_dir();

        // Search for the page in the platform specific directory
        if let Some(pf) = platform {
            let path = platforms_dir.join(&pf).join(&page_filename);
            if path.exists() && path.is_file() {
                return Some(path);
            }
        }

        // If platform is not supported or if platform specific page does not exist,
        // look up the page in the "common" directory.
        let path = platforms_dir.join("common").join(&page_filename);

        // Return it if it exists, otherwise give up and return `None`
        if path.exists() && path.is_file() {
            Some(path)
        } else {
            None
        }
    }

    /// Return the available pages.
    pub fn list_pages(&self) -> Result<Vec<String>, TealdeerError> {
        // Determine platforms directory and platform
        let cache_dir = self.get_cache_dir()?;
        let platforms_dir = cache_dir.join("tldr-master").join("pages");
        let platform_dir = self.get_platform_dir();

        // Closure that allows the WalkDir instance to traverse platform
        // specific and common page directories, but not others.
        let should_walk = |entry: &DirEntry| -> bool {
            let file_type = entry.file_type();
            let file_name = match entry.file_name().to_str() {
                Some(name) => name,
                None => return false,
            };
            if file_type.is_dir() {
                if file_name == "common" {
                    return true;
                }
                if let Some(platform) = platform_dir {
                    return file_name == platform;
                }
            } else if file_type.is_file() {
                return true;
            }
            false
        };

        // Recursively walk through common and (if applicable) platform specific directory
        let mut pages = WalkDir::new(platforms_dir)
            .min_depth(1) // Skip root directory
            .into_iter()
            .filter_entry(|e| should_walk(e)) // Filter out pages for other architectures
            .filter_map(|e| e.ok()) // Convert results to options, filter out errors
            .filter_map(|e| {
                let path = e.path();
                let extension = &path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if e.file_type().is_file() && extension == &"md" {
                    path.file_stem()
                        .and_then(|stem| stem.to_str().map(|s| s.into()))
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        pages.sort();
        pages.dedup();
        Ok(pages)
    }

    /// Delete the cache directory.
    pub fn clear(&self) -> Result<(), TealdeerError> {
        let path = self.get_cache_dir()?;
        if path.exists() && path.is_dir() {
            fs::remove_dir_all(&path).map_err(|_| CacheError(format!(
                "Could not remove cache directory ({}).",
                path.display()
            )))?;
        } else if path.exists() {
            return Err(CacheError(format!(
                "Cache path ({}) is not a directory.",
                path.display()
            )));
        } else {
            return Err(CacheError(format!(
                "Cache path ({}) does not exist.",
                path.display()
            )));
        };
        Ok(())
    }
}
