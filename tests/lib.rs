//! Integration tests.

use std::{
    fs::{self, File, create_dir_all},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use assert_cmd::prelude::*;
use predicates::{
    boolean::PredicateBooleanExt,
    ord::eq,
    prelude::predicate::str::{contains, diff, is_empty, is_match},
};
use tempfile::{Builder as TempfileBuilder, TempDir};

pub static TLDR_PAGES_DIR: &str = "tldr-pages";

struct TestEnv {
    _test_dir: TempDir,
    pub default_features: bool,
    pub features: Vec<String>,
}

impl TestEnv {
    fn new() -> Self {
        let test_dir: TempDir = TempfileBuilder::new()
            .prefix(".tldr.test")
            .tempdir()
            .unwrap();

        let this = TestEnv {
            _test_dir: test_dir,
            default_features: true,
            features: vec![],
        };

        create_dir_all(this.cache_dir()).unwrap();
        create_dir_all(this.config_dir()).unwrap();
        create_dir_all(this.custom_pages_dir()).unwrap();

        this.init_config();

        this
    }

    fn cache_dir(&self) -> PathBuf {
        self._test_dir.path().join(".cache")
    }

    fn config_dir(&self) -> PathBuf {
        self._test_dir.path().join(".config")
    }

    fn custom_pages_dir(&self) -> PathBuf {
        self._test_dir.path().join(".custom_pages")
    }

    fn append_to_config(&self, content: impl AsRef<str>) {
        File::options()
            .create(true)
            .append(true)
            .open(self.config_dir().join("config.toml"))
            .expect("Failed to open config file")
            .write_all(content.as_ref().as_bytes())
            .expect("Failed to append to config file.");
    }
    fn delete_config(&self) {
        fs::remove_file(self.config_dir().join("config.toml")).unwrap();
    }
    fn init_config(&self) {
        self.append_to_config(format!(
            "directories.cache_dir = '{}'\n",
            self.cache_dir().to_str().unwrap(),
        ));
    }

    fn create_secondary_config(self) -> Self {
        self.append_to_secondary_config(format!(
            "directories.cache_dir = '{}'\n",
            self.cache_dir().to_str().unwrap(),
        ));
        self
    }

    fn append_to_secondary_config(&self, content: impl AsRef<str>) {
        File::options()
            .create(true)
            .append(true)
            .open(self.config_dir().join("config-secondary.toml"))
            .expect("Failed to open config file")
            .write_all(content.as_ref().as_bytes())
            .expect("Failed to append to config file.");
    }

    fn remove_initial_config(self) -> Self {
        let _ = fs::remove_file(self.config_dir().join("config.toml"));
        self
    }

    /// Add entry for that environment to the "common" pages.
    fn add_entry(&self, name: &str, contents: &str) {
        self.add_os_entry("common", name, contents);
    }

    /// Add entry for that environment to an OS-specific subfolder.
    fn add_os_entry(&self, os: &str, name: &str, contents: &str) {
        self.add_os_lang_entry(os, "en", name, contents);
    }

    /// Add entry for that environment to a language-specific subfolder.
    fn add_lang_entry(&self, lang: &str, name: &str, contents: &str) {
        self.add_os_lang_entry("common", lang, name, contents);
    }

    /// Add entry for that environment to an OS- and language specific subfolder.
    fn add_os_lang_entry(&self, os: &str, lang: &str, name: &str, contents: &str) {
        let dir = self
            .cache_dir()
            .join(TLDR_PAGES_DIR)
            .join(format!("pages.{lang}"))
            .join(os);
        create_dir_all(&dir).unwrap();

        fs::write(dir.join(format!("{name}.md")), contents.as_bytes()).unwrap();
    }

    /// Add custom patch entry to the custom_pages_dir
    fn add_page_entry(&self, name: &str, contents: &str) {
        let dir = &self.custom_pages_dir();
        create_dir_all(dir).unwrap();
        fs::write(dir.join(format!("{name}.page.md")), contents.as_bytes()).unwrap();
    }

    /// Add custom patch entry to the custom_pages_dir
    fn add_patch_entry(&self, name: &str, contents: &str) {
        let dir = &self.custom_pages_dir();
        create_dir_all(dir).unwrap();
        fs::write(dir.join(format!("{name}.patch.md")), contents.as_bytes()).unwrap();
    }

    /// Disable default features.
    fn no_default_features(mut self) -> Self {
        self.default_features = false;
        self
    }

    /// Add the specified feature.
    fn with_feature<S: Into<String>>(mut self, feature: S) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Return a new `Command` with env vars set.
    fn command(&self) -> Command {
        let mut build = escargot::CargoBuild::new()
            .bin("tldr")
            .arg("--color=never")
            .current_release()
            .current_target();
        if !self.default_features {
            build = build.no_default_features();
        }
        if !self.features.is_empty() {
            build = build.features(self.features.join(" "))
        }
        let run = build.run().expect("Failed to build tealdeer for testing");
        let mut cmd = run.command();

        // Avoid inheriting those from the test process. We can't just use .env_clear() because
        // this breaks tests on Windows in GitHub Actions.
        let relevant_env_variables = [
            "LANG",
            "LANGUAGE",
            "TEALDEER_CACHE_DIR",
            "EDITOR",
            "NO_COLOR",
        ];
        for variable_name in relevant_env_variables {
            cmd.env_remove(variable_name);
        }
        cmd.env("TEALDEER_CONFIG_DIR", self.config_dir().to_str().unwrap());
        cmd
    }

    fn install_default_cache(self) -> Self {
        copy_recursively(
            &PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), "tests", "cache"]),
            &self.cache_dir().join(TLDR_PAGES_DIR),
        )
        .expect("Failed to copy the cache to the test environment");

        self
    }

    fn install_default_custom_pages(self) -> Self {
        copy_recursively(
            &PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), "tests", "custom-pages"]),
            self.custom_pages_dir().as_path(),
        )
        .expect("Failed to copy the custom pages to the test environment");

        self.write_custom_pages_config()
    }

    fn write_custom_pages_config(self) -> Self {
        self.append_to_config(format!(
            "directories.custom_pages_dir = '{}'\n",
            self.custom_pages_dir().to_str().unwrap()
        ));

        self
    }
}

fn copy_recursively(source: &Path, destination: &Path) -> io::Result<()> {
    if source.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_recursively(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, destination)?;
    }

    Ok(())
}

#[test]
#[should_panic]
fn test_cannot_build_without_tls_feature() {
    let _ = TestEnv::new().no_default_features().command();
}

#[test]
fn test_load_the_correct_config() {
    let testenv = TestEnv::new()
        .install_default_cache()
        .create_secondary_config();
    testenv.append_to_secondary_config(include_str!("style-config.toml"));

    let expected_default = include_str!("rendered/inkscape-default.expected");
    let expected_with_config = include_str!("rendered/inkscape-with-config.expected");

    testenv
        .command()
        .args(["--color", "always", "inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected_default));

    testenv
        .command()
        .args([
            "--color",
            "always",
            "--config-path",
            testenv
                .config_dir()
                .join("config-secondary.toml")
                .to_str()
                .unwrap(),
            "inkscape-v2",
        ])
        .assert()
        .success()
        .stdout(diff(expected_with_config));
}

#[test]
fn test_fail_on_custom_config_path_is_directory() {
    let testenv = TestEnv::new();
    let error = if cfg!(windows) {
        "Access is denied"
    } else {
        "Is a directory"
    };
    testenv
        .command()
        .args([
            "--config-path",
            testenv.config_dir().to_str().unwrap(),
            "sl",
        ])
        .assert()
        .failure()
        .stderr(contains(error));
}

#[test]
fn test_missing_cache() {
    TestEnv::new()
        .command()
        .args(["sl"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));
}

#[test]
fn test_tealdeer_page_works_without_cache() {
    TestEnv::new()
        .command()
        .args(["tealdeer"])
        .assert()
        .success()
        .stdout(contains("for your installed tealdeer version"));
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_update_cache_default_features() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["sl"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));

    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

    testenv.command().args(["sl"]).assert().success();
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_update_cache_rustls_webpki() {
    let testenv = TestEnv::new()
        .no_default_features()
        .with_feature("rustls-with-webpki-roots");

    testenv
        .command()
        .args(["sl"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));

    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

    testenv.command().args(["sl"]).assert().success();
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_update_cache_native_tls() {
    let testenv = TestEnv::new()
        .no_default_features()
        .with_feature("rustls-with-native-roots");

    testenv
        .command()
        .args(["sl"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));

    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

    testenv.command().args(["sl"]).assert().success();
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_quiet_cache() {
    let testenv = TestEnv::new();
    testenv
        .command()
        .args(["--update", "--quiet"])
        .assert()
        .success()
        .stdout(is_empty());

    testenv
        .command()
        .args(["--clear-cache", "--quiet"])
        .assert()
        .success()
        .stdout(is_empty());
}

#[test]
fn test_clear_only_pages_directory() {
    let testenv = TestEnv::new().install_default_cache();
    testenv
        .command()
        .args(["--clear-cache"])
        .assert()
        .success()
        .stderr(contains(format!(
            "Successfully cleared cache at `{}`.",
            testenv.cache_dir().join(TLDR_PAGES_DIR).to_str().unwrap(),
        )));

    assert!(testenv.cache_dir().is_dir());
    assert!(!testenv.cache_dir().join(TLDR_PAGES_DIR).exists());
}

#[test]
fn test_warn_invalid_tls_backend() {
    let testenv = TestEnv::new()
        .no_default_features()
        .with_feature("rustls-with-webpki-roots")
        .remove_initial_config();

    testenv.append_to_config("updates.tls_backend = 'invalid-tls-backend'\n");

    testenv
        .command()
        .args(["sl"])
        .assert()
        .failure()
        .stderr(contains("unknown variant `invalid-tls-backend`, expected one of `native-tls`, `rustls-with-webpki-roots`, `rustls-with-native-roots`"));
}

#[test]
fn test_quiet_failures() {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(["fakeprogram", "-q"])
        .assert()
        .failure()
        .stdout(is_empty());
}

#[test]
fn test_quiet_missing_cache() {
    let testenv = TestEnv::new();

    for args in [["--list", "--quiet"], ["sl", "--quiet"]] {
        testenv
            .command()
            .args(args)
            .assert()
            .failure()
            .stdout(is_empty())
            .stderr(is_empty());
    }
}

#[test]
fn test_quiet_old_cache() {
    let testenv = TestEnv::new().install_default_cache();

    filetime::set_file_mtime(
        testenv.cache_dir().join(TLDR_PAGES_DIR),
        filetime::FileTime::from_unix_time(1, 0),
    )
    .unwrap();

    testenv
        .command()
        .args(["which"])
        .assert()
        .success()
        .stderr(contains("The cache hasn't been updated for "));

    testenv
        .command()
        .args(["which", "--quiet"])
        .assert()
        .success()
        .stderr(contains("The cache hasn't been updated for ").not());
}

#[test]
fn test_warn_cache_age_never() {
    let testenv = TestEnv::new().install_default_cache();

    filetime::set_file_mtime(
        testenv.cache_dir().join(TLDR_PAGES_DIR),
        filetime::FileTime::from_unix_time(1, 0),
    )
    .unwrap();

    testenv.append_to_config("[updates]\nwarn_cache_age = \"never\"\n");

    testenv
        .command()
        .args(["which"])
        .assert()
        .success()
        .stderr(contains("The cache hasn't been updated for ").not());
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_create_cache_directory_path() {
    let testenv = TestEnv::new().remove_initial_config();
    let cache_dir = &testenv.cache_dir();
    let internal_cache_dir = cache_dir.join("internal");
    testenv.append_to_config(format!(
        "directories.cache_dir = '{}'\n",
        internal_cache_dir.to_str().unwrap()
    ));

    let mut command = testenv.command();

    assert!(!internal_cache_dir.exists());

    command
        .arg("--update")
        .assert()
        .success()
        .stderr(contains(format!(
            "Successfully created cache directory `{}`.",
            internal_cache_dir.join(TLDR_PAGES_DIR).to_str().unwrap()
        )))
        .stderr(contains("Successfully updated cache."));

    assert!(internal_cache_dir.is_dir());
}

#[test]
fn test_cache_location_not_a_directory() {
    let testenv = TestEnv::new();
    let cache_dir = &testenv.cache_dir();
    File::create(cache_dir.join(TLDR_PAGES_DIR)).unwrap();

    testenv
        .command()
        .arg("--list")
        .assert()
        .failure()
        .stderr(contains(format!(
            "Cache directory `{}` exists, but is not a directory.",
            cache_dir.join(TLDR_PAGES_DIR).display(),
        )));
}

#[cfg(unix)]
#[test]
fn test_cache_location_permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .arg("--list")
        .assert()
        .success()
        .stderr(contains("Permission denied").not());

    // Make cache directory unreadable
    let cache_dir = testenv.cache_dir();
    let mut permissions = cache_dir.metadata().unwrap().permissions();
    permissions.set_mode(0o0);
    fs::set_permissions(cache_dir, permissions).unwrap();

    testenv
        .command()
        .arg("--list")
        .assert()
        .failure()
        .stderr(contains("Permission denied"));
}

#[test]
fn test_cache_location_source() {
    let testenv = TestEnv::new().remove_initial_config();
    let default_cache_dir = &testenv.cache_dir();
    let tmp_cache_dir = TempfileBuilder::new()
        .prefix(".tldr.test.cache_dir")
        .tempdir()
        .unwrap();

    // Source: Default (OS convention)
    let mut command = testenv.command();
    command
        .arg("--show-paths")
        .assert()
        .success()
        .stdout(is_match("\nCache dir:        [^(]* \\(OS convention\\)\n").unwrap());

    // Source: Config variable
    let mut command = testenv.command();
    testenv.append_to_config(format!(
        "directories.cache_dir = '{}'\n",
        tmp_cache_dir.path().to_str().unwrap(),
    ));
    command
        .arg("--show-paths")
        .assert()
        .success()
        .stdout(is_match("\nCache dir:        [^(]* \\(config file\\)\n").unwrap());

    // Source: Env var
    let mut command = testenv.command();
    command.env("TEALDEER_CACHE_DIR", default_cache_dir.to_str().unwrap());
    command
        .arg("--show-paths")
        .assert()
        .success()
        .stdout(is_match("\nCache dir:        [^(]* \\(env variable\\)\n").unwrap());
}

#[test]
fn test_setup_seed_config() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["--seed-config"])
        .assert()
        .failure()
        .stderr(contains("A configuration file already exists"));

    assert!(testenv.config_dir().join("config.toml").is_file());

    let testenv = testenv.remove_initial_config();
    testenv
        .command()
        .args(["--seed-config"])
        .assert()
        .success()
        .stderr(contains("Successfully created seed config file here"));

    assert!(testenv.config_dir().join("config.toml").is_file());

    // Create parent directories as needed for the default config path.
    fs::remove_dir_all(testenv.config_dir()).unwrap();
    testenv
        .command()
        .args(["--seed-config"])
        .assert()
        .success()
        .stderr(contains("Successfully created seed config file here"));

    assert!(testenv.config_dir().join("config.toml").is_file());

    // Write the default config to --config-path if specified by the user
    // at the same time.
    let custom_config_path = testenv.config_dir().join("config_custom.toml");
    testenv
        .command()
        .args([
            "--seed-config",
            "--config-path",
            custom_config_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(contains("Successfully created seed config file here"));

    assert!(custom_config_path.is_file());

    // DON'T create parent directories for a custom config path.
    fs::remove_dir_all(testenv.config_dir()).unwrap();
    testenv
        .command()
        .args([
            "--seed-config",
            "--config-path",
            custom_config_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(contains("Could not create config file"));

    assert!(!custom_config_path.is_file());
}

#[test]
fn test_show_paths() {
    let testenv = TestEnv::new();

    // Show general commands
    testenv
        .command()
        .args(["--show-paths"])
        .assert()
        .success()
        .stdout(contains(format!(
            "Config dir:       {}",
            testenv.config_dir().to_str().unwrap(),
        )))
        .stdout(contains(format!(
            "Config path:      {}",
            testenv.config_dir().join("config.toml").to_str().unwrap(),
        )))
        .stdout(contains(format!(
            "Cache dir:        {}",
            testenv.cache_dir().to_str().unwrap(),
        )))
        .stdout(contains(format!(
            "Pages dir:        {}",
            testenv.cache_dir().join(TLDR_PAGES_DIR).to_str().unwrap(),
        )));

    let testenv = testenv.write_custom_pages_config();

    // Now ensure that this path is contained in the output
    testenv
        .command()
        .args(["--show-paths"])
        .assert()
        .success()
        .stdout(contains(format!(
            "Custom pages dir: {}",
            testenv.custom_pages_dir().to_str().unwrap(),
        )));
}

#[test]
fn test_os_specific_page() {
    let testenv = TestEnv::new();

    testenv.add_os_entry("sunos", "truss", "contents");

    testenv
        .command()
        .args(["--platform", "sunos", "truss"])
        .assert()
        .success();
}

#[test]
fn test_config_platforms() {
    let testenv = TestEnv::new();
    testenv.add_os_entry("sunos", "sunos-command", "");

    let set_config_platforms = |platforms| {
        testenv.delete_config();
        testenv.init_config();
        testenv.append_to_config(format!("search.platforms = {platforms}"));
    };

    // By default all platforms are searched
    testenv.command().arg("sunos-command").assert().success();

    set_config_platforms("[]");
    testenv.command().arg("sunos-command").assert().failure();

    set_config_platforms("['linux']");
    testenv.command().arg("sunos-command").assert().failure();

    set_config_platforms("['sunos']");
    testenv.command().arg("sunos-command").assert().success();

    set_config_platforms("['linux', 'all']");
    testenv.command().arg("sunos-command").assert().success();

    set_config_platforms("['current', 'all']");
    testenv.command().arg("sunos-command").assert().success();
}

#[test]
fn test_markdown_rendering() {
    let testenv = TestEnv::new().install_default_cache();

    let expected = include_str!("cache/pages.en/common/which.md");
    testenv
        .command()
        .args(["--raw", "which"])
        .assert()
        .success()
        .stdout(diff(expected));
}

fn _test_correct_rendering(page: &str, expected: &'static str, additional_args: &[&str]) {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(additional_args)
        .arg(page)
        .assert()
        .success()
        .stdout(diff(expected));
}

/// An end-to-end integration test for direct file rendering (v1 syntax).
#[test]
fn test_correct_rendering_v1() {
    _test_correct_rendering(
        "inkscape-v1",
        include_str!("rendered/inkscape-default.expected"),
        &["--color", "always"],
    );
}

/// An end-to-end integration test for direct file rendering (v2 syntax).
#[test]
fn test_correct_rendering_v2() {
    _test_correct_rendering(
        "inkscape-v2",
        include_str!("rendered/inkscape-default.expected"),
        &["--color", "always"],
    );
}

#[test]
/// An end-to-end integration test for direct file rendering with the `--color auto` option. This
/// will not use styling since output is not stdout.
fn test_rendering_color_auto() {
    _test_correct_rendering(
        "inkscape-v2",
        include_str!("rendered/inkscape-default-no-color.expected"),
        &["--color", "auto"],
    );
}

#[test]
/// An end-to-end integration test for direct file rendering with the `--color never` option.
fn test_rendering_color_never() {
    _test_correct_rendering(
        "inkscape-v2",
        include_str!("rendered/inkscape-default-no-color.expected"),
        &["--color", "never"],
    );
}

/// An end-to-end integration test for the indent config option
#[test]
fn test_rendering_with_indentation() {
    let testenv = TestEnv::new().install_default_cache();
    let expected_custom_indentation = include_str!("rendered/inkscape-compact-no-color.expected");

    // Configure to set base and command indents
    testenv.append_to_config("display.indent.base = 3\n");
    testenv.append_to_config("display.indent.command = 1\n");

    testenv
        .command()
        .args(["--color", "never", "inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected_custom_indentation));
}

#[test]
fn test_rendering_i18n() {
    _test_correct_rendering(
        "apt",
        include_str!("rendered/apt.ja.expected"),
        &["--color", "always", "--language", "ja"],
    );
}

/// An end-to-end integration test for rendering with custom syntax config.
#[test]
fn test_correct_rendering_with_config() {
    let testenv = TestEnv::new().install_default_cache();

    testenv.append_to_config(include_str!("style-config.toml"));

    let expected = include_str!("rendered/inkscape-with-config.expected");

    testenv
        .command()
        .args(["--color", "always", "inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected));
}

/// An end-to-end integration test for rendering with show_title config option enabled.
#[test]
fn test_show_title_config() {
    // Test that default behavior without show_title shows no title
    let testenv = TestEnv::new().install_default_cache();
    let expected_no_title = include_str!("rendered/inkscape-default.expected");

    testenv
        .command()
        .args(["--color", "always", "inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected_no_title));

    // Configure to enable show_title
    testenv.append_to_config("display.show_title = true\n");

    let expected_no_color = include_str!("rendered/inkscape-with-title-no-color.expected");

    testenv
        .command()
        .args(["inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected_no_color));

    let expected = include_str!("rendered/inkscape-with-title.expected");

    testenv
        .command()
        .args(["--color", "always", "inkscape-v2"])
        .assert()
        .success()
        .stdout(diff(expected));
}

#[test]
fn test_spaces_find_command() {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(["git", "checkout"])
        .assert()
        .success();
}

#[test]
fn test_pager_flag_enable() {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(["--pager", "which"])
        .assert()
        .success();
}

#[test]
fn test_multiple_platform_command_search() {
    let testenv = TestEnv::new();
    testenv.add_os_entry("linux", "linux-only", "this command only exists for linux");
    testenv.add_os_entry(
        "linux",
        "windows-and-linux",
        "# windows-and-linux \n\n > linux version",
    );
    testenv.add_os_entry(
        "windows",
        "windows-and-linux",
        "# windows-and-linux \n\n > windows version",
    );

    testenv
        .command()
        .args(["--platform", "windows", "--platform", "linux", "linux-only"])
        .assert()
        .success();

    // test order of platforms supplied if preserved
    testenv
        .command()
        .args([
            "--platform",
            "windows",
            "--platform",
            "linux",
            "windows-and-linux",
        ])
        .assert()
        .success()
        .stdout(contains("windows version"));

    testenv
        .command()
        .args([
            "--platform",
            "linux",
            "--platform",
            "windows",
            "windows-and-linux",
        ])
        .assert()
        .success()
        .stdout(contains("linux version"));
}

#[test]
fn test_multiple_platform_command_search_not_found() {
    let testenv = TestEnv::new();
    testenv.add_os_entry(
        "windows",
        "windows-only",
        "this command only exists for Windows",
    );

    testenv
        .command()
        .args(["--platform", "macos", "--platform", "linux", "windows-only"])
        .assert()
        .stderr(contains("Page `windows-only` not found in cache."));
}

#[test]
fn test_macos_is_alias_for_osx() {
    let testenv = TestEnv::new();
    testenv.add_os_entry("osx", "maconly", "this command only exists on mac");

    testenv
        .command()
        .args(["--platform", "macos", "maconly"])
        .assert()
        .success();
    testenv
        .command()
        .args(["--platform", "osx", "maconly"])
        .assert()
        .success();

    testenv
        .command()
        .args(["--platform", "macos", "--list"])
        .assert()
        .stdout("maconly\n");
    testenv
        .command()
        .args(["--platform", "osx", "--list"])
        .assert()
        .stdout("maconly\n");

    testenv.append_to_config("search.platforms = ['osx']\n");
    testenv.command().arg("--list").assert().stdout("maconly\n");

    testenv.delete_config();
    testenv.init_config();
    testenv.append_to_config("search.platforms = ['macos']\n");
    testenv.command().arg("--list").assert().stdout("maconly\n");
}

#[test]
fn test_common_platform_is_used_as_fallback() {
    let testenv = TestEnv::new();
    testenv.add_entry("in-common", "this command comes from common");

    // No platform specified
    testenv.command().args(["in-common"]).assert().success();

    // Platform specified
    testenv
        .command()
        .args(["--platform", "linux", "in-common"])
        .assert()
        .success();
}

#[test]
fn test_search_language_precedence() {
    let testenv = TestEnv::new();
    for lang in ["en", "de", "it", "fr", "pl", "nl"] {
        testenv.add_lang_entry(lang, lang, "");
    }

    #[expect(clippy::type_complexity)]
    let run = |cases: &[(Vec<(&str, &str)>, Vec<&str>, &str)]| {
        for (extra_env, extra_args, expected) in cases {
            let mut cmd = testenv.command();
            for (key, value) in extra_env {
                cmd.env(key, value);
            }
            cmd.args(extra_args);
            cmd.arg("--list");
            cmd.assert().success().stdout(eq(*expected));
        }
    };

    let env_cases = &[
        (vec![], vec![], "en\n"),
        (vec![("LANGUAGE", "de:it")], vec![], "en\n"),
        (
            vec![("LANG", "fr"), ("LANGUAGE", "de:it")],
            vec![],
            "de\nen\nfr\nit\n",
        ),
        (
            vec![("LANG", "fr"), ("LANGUAGE", "de:it")],
            vec!["--language", "pl"],
            "pl\n",
        ),
    ];
    run(env_cases);

    // Environment is only used when config setting is not set
    testenv.append_to_config("search.languages = ['nl']\n");
    let config_cases = &[
        (vec![], vec![], "nl\n"),
        (vec![("LANGUAGE", "de:it")], vec![], "nl\n"),
        (vec![("LANG", "fr"), ("LANGUAGE", "de:it")], vec![], "nl\n"),
        (
            vec![("LANG", "fr"), ("LANGUAGE", "de:it")],
            vec!["--language", "pl"],
            "pl\n",
        ),
    ];
    run(config_cases);

    // The above update setting does not change anything
    testenv.append_to_config("updates.download_languages = ['cz']");
    run(config_cases);
    testenv.delete_config();
    testenv.init_config();
    testenv.append_to_config("updates.download_languages = ['cz']");
    run(env_cases);
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_update_language_arg() {
    let testenv = TestEnv::new();
    testenv
        .command()
        .env("LANG", "it")
        .arg("--update")
        .assert()
        .success()
        .stderr(contains("it"))
        .stderr(contains("en"));

    testenv
        .command()
        .env("LANG", "en")
        .args(["--language", "it"])
        .arg("--update")
        .assert()
        .success()
        .stderr(contains("it"))
        .stderr(contains("en").not());
}

#[test]
fn test_list_flag_rendering() {
    let testenv = TestEnv::new().write_custom_pages_config();

    testenv
        .command()
        .args(["--list"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));

    testenv.add_entry("foo", "");

    testenv
        .command()
        .args(["--list"])
        .assert()
        .success()
        .stdout("foo\n");

    testenv.add_entry("bar", "");
    testenv.add_entry("baz", "");
    testenv.add_entry("qux", "");
    testenv.add_page_entry("faz", "");
    testenv.add_page_entry("bar", "");
    testenv.add_page_entry("fiz", "");
    testenv.add_patch_entry("buz", "");

    testenv
        .command()
        .args(["--list"])
        .assert()
        .success()
        .stdout("bar\nbaz\nfaz\nfiz\nfoo\nqux\n");
}

#[test]
fn test_multi_platform_list_flag_rendering() {
    let testenv = TestEnv::new().write_custom_pages_config();

    testenv.add_entry("common", "");

    testenv
        .command()
        .args(["--list"])
        .assert()
        .success()
        .stdout("common\n");

    testenv
        .command()
        .args(["--platform", "linux", "--list"])
        .assert()
        .success()
        .stdout("common\n");

    testenv
        .command()
        .args(["--platform", "windows", "--list"])
        .assert()
        .success()
        .stdout("common\n");

    testenv.add_os_entry("linux", "rm", "");
    testenv.add_os_entry("linux", "ls", "");
    testenv.add_os_entry("windows", "del", "");
    testenv.add_os_entry("windows", "dir", "");
    testenv.add_os_entry("linux", "winux", "");
    testenv.add_os_entry("windows", "winux", "");

    // test `--list` for `--platform linux` by itself
    testenv
        .command()
        .args(["--platform", "linux", "--list"])
        .assert()
        .success()
        .stdout("common\nls\nrm\nwinux\n");

    // test `--list` for `--platform windows` by itself
    testenv
        .command()
        .args(["--platform", "windows", "--list"])
        .assert()
        .success()
        .stdout("common\ndel\ndir\nwinux\n");

    // test `--list` for `--platform linux --platform windows`
    testenv
        .command()
        .args(["--platform", "linux", "--platform", "windows", "--list"])
        .assert()
        .success()
        .stdout("common\ndel\ndir\nls\nrm\nwinux\n");

    // test `--list` for `--platform windows --platform linux`
    testenv
        .command()
        .args(["--platform", "linux", "--platform", "windows", "--list"])
        .assert()
        .success()
        .stdout("common\ndel\ndir\nls\nrm\nwinux\n");
}

#[cfg_attr(feature = "ignore-online-tests", ignore = "online test")]
#[test]
fn test_autoupdate_cache() {
    let testenv = TestEnv::new();

    // The first time, if automatic updates are disabled, the cache should not be found
    testenv
        .command()
        .args(["--list"])
        .assert()
        .failure()
        .stderr(contains("Page cache not found. Please run `tldr --update`"));

    let cache_file_path = testenv.cache_dir().join(TLDR_PAGES_DIR);

    testenv
        .append_to_config("updates.auto_update = true\nupdates.auto_update_interval_hours = 24\n");

    // Helper function that runs `tldr --list` and asserts that the cache is automatically updated
    // or not, depending on the value of `expected`.
    let check_cache_updated = |expected| {
        let assert = testenv.command().args(["--list"]).assert().success();
        let pred = contains("Successfully updated cache");
        if expected {
            assert.stderr(pred)
        } else {
            assert.stderr(pred.not())
        };
    };

    // The cache is updated the first time we run `tldr --list`
    check_cache_updated(true);

    // The cache is not updated with a subsequent call
    check_cache_updated(false);

    // We update the modification and access times such that they are about 23 hours from now.
    // auto-update interval is 24 hours, the cache should not be updated
    let new_mtime = SystemTime::now() - Duration::from_secs(82_800);
    filetime::set_file_mtime(&cache_file_path, new_mtime.into()).unwrap();
    check_cache_updated(false);

    // We update the modification and access times such that they are about 25 hours from now.
    // auto-update interval is 24 hours, the cache should be updated
    let new_mtime = SystemTime::now() - Duration::from_secs(90_000);
    filetime::set_file_mtime(&cache_file_path, new_mtime.into()).unwrap();
    check_cache_updated(true);

    // The cache is not updated with a subsequent call
    check_cache_updated(false);
}

/// Regression test: `--no-auto-update` should be usable together with `--list`,
/// since the auto-update gate in `main.rs` also applies to `--list`.
#[test]
fn test_no_auto_update_with_list() {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(["--list", "--no-auto-update"])
        .assert()
        .success();
}

/// End-end test to ensure .page.md files overwrite pages in cache_dir
#[test]
fn test_custom_page_overwrites() {
    let testenv = TestEnv::new().write_custom_pages_config();

    // Add file that should be ignored to the cache dir
    testenv.add_entry("inkscape-v2", "");
    // Add .page.md file to custom_pages_dir
    testenv.add_page_entry(
        "inkscape-v2",
        include_str!("cache/pages.en/common/inkscape-v2.md"),
    );

    // Load expected output
    let expected = include_str!("rendered/inkscape-default-no-color.expected");

    testenv
        .command()
        .args(["inkscape-v2", "--color", "never"])
        .assert()
        .success()
        .stdout(diff(expected));
}

/// End-End test to ensure that .patch.md files are appended to pages in the cache_dir
#[test]
fn test_custom_patch_appends_to_common() {
    let testenv = TestEnv::new()
        .install_default_cache()
        .install_default_custom_pages();

    // Load expected output
    let expected = include_str!("rendered/inkscape-patched-no-color.expected");

    testenv
        .command()
        .args(["inkscape-v2", "--color", "never"])
        .assert()
        .success()
        .stdout(diff(expected));
}

/// End-End test to ensure that .patch.md files are not appended to .page.md files in the custom_pages_dir
/// Maybe this interaction should change but I put this test here for the coverage
#[test]
fn test_custom_patch_does_not_append_to_custom() {
    let testenv = TestEnv::new()
        .install_default_cache()
        .install_default_custom_pages();

    // In addition to the page in the cache, add the same page as a custom page.
    testenv.add_page_entry(
        "inkscape-v2",
        include_str!("cache/pages.en/common/inkscape-v2.md"),
    );

    // Load expected output
    let expected = include_str!("rendered/inkscape-default-no-color.expected");

    testenv
        .command()
        .args(["inkscape-v2", "--color", "never"])
        .assert()
        .success()
        .stdout(diff(expected));
}

#[test]
#[cfg(target_os = "windows")]
fn test_pager_warning() {
    let testenv = TestEnv::new().install_default_cache();

    // Regular call should not show a "pager flag not available on windows" warning
    testenv
        .command()
        .args(["which"])
        .assert()
        .success()
        .stderr(contains("pager flag not available on Windows").not());

    // But it should be shown if the pager flag is true
    testenv
        .command()
        .args(["--pager", "which"])
        .assert()
        .success()
        .stderr(contains("pager flag not available on Windows"));
}

/// Ensure that page lookup is case insensitive, so a page lookup for `eyed3`
/// and `eyeD3` should return the same page.
#[test]
fn test_lowercased_page_lookup() {
    let testenv = TestEnv::new();

    // Lookup `eyed3`, initially fails
    testenv.command().args(["eyed3"]).assert().failure();

    // Add entry
    testenv.add_entry("eyed3", "contents");

    // Lookup `eyed3` again
    testenv.command().args(["eyed3"]).assert().success();

    // Lookup `eyeD3`, should succeed as well
    testenv.command().args(["eyeD3"]).assert().success();
}

/// Regression test for #219: It should be possible to combine `--raw` and `-f`.
#[test]
fn test_raw_render_file() {
    let testenv = TestEnv::new().install_default_cache();

    let path = testenv
        .cache_dir()
        .join(TLDR_PAGES_DIR)
        .join("pages.en/common/inkscape-v1.md");
    let mut args = vec!["--color", "never", "-f", &path.to_str().unwrap()];

    // Default render
    testenv
        .command()
        .args(&args)
        .assert()
        .success()
        .stdout(diff(include_str!(
            "rendered/inkscape-default-no-color.expected"
        )));

    // Raw render
    args.push("--raw");
    testenv
        .command()
        .args(&args)
        .assert()
        .success()
        .stdout(diff(include_str!("cache/pages.en/common/inkscape-v1.md")));
}

fn touch_custom_page(testenv: &TestEnv) {
    let args = vec!["--edit-page", "foo"];

    testenv
        .command()
        .args(&args)
        .env("EDITOR", "touch")
        .assert()
        .success();
    assert!(testenv.custom_pages_dir().join("foo.page.md").exists());
}

fn touch_custom_patch(testenv: &TestEnv) {
    let args = vec!["--edit-patch", "foo"];

    testenv
        .command()
        .args(&args)
        .env("EDITOR", "touch")
        .assert()
        .success();
    assert!(testenv.custom_pages_dir().join("foo.patch.md").exists());
}

#[test]
fn test_edit_page() {
    let testenv = TestEnv::new().write_custom_pages_config();
    touch_custom_page(&testenv);
}

#[test]
fn test_edit_patch() {
    let testenv = TestEnv::new().write_custom_pages_config();
    touch_custom_patch(&testenv);
}

#[test]
fn test_recreate_dir() {
    let testenv = TestEnv::new().write_custom_pages_config();
    touch_custom_patch(&testenv);
    touch_custom_page(&testenv);
}

#[test]
fn test_custom_pages_dir_is_not_dir() {
    let testenv = TestEnv::new().write_custom_pages_config();
    let _ = std::fs::remove_dir_all(testenv.custom_pages_dir());
    let _ = File::create(testenv.custom_pages_dir()).unwrap();
    assert!(testenv.custom_pages_dir().is_file());

    let args = vec!["--edit-patch", "foo"];

    testenv
        .command()
        .args(&args)
        .env("EDITOR", "touch")
        .assert()
        .failure();
}

#[test]
fn test_failure_on_unknown_field_in_config() {
    let testenv = TestEnv::new().install_default_cache();

    testenv.append_to_config(
        "
        [display]
        unknown = true
        ",
    );

    testenv
        .command()
        .args(["which"])
        .assert()
        .failure()
        .stderr(contains("unknown field `unknown`"));
}

#[test]
fn test_failure_on_unknown_field_in_override_config() {
    let testenv = TestEnv::new().install_default_cache();

    testenv
        .command()
        .args(["--override-config", "display.invalid=true", "which"])
        .assert()
        .failure()
        .stderr(contains("unknown field `invalid`"));
}

mod placeholder_format {
    use super::*;

    #[test]
    fn default_long() {
        let testenv = TestEnv::new().install_default_cache();
        testenv
            .command()
            .args(["--color=always", "playerctl"])
            .assert()
            .success()
            .stdout(eq(include_str!("rendered/playerctl-long.expected")));
    }

    #[test]
    fn config() {
        let cases = [
            ("short", include_str!("rendered/playerctl-short.expected")),
            ("long", include_str!("rendered/playerctl-long.expected")),
            ("both", include_str!("rendered/playerctl-both.expected")),
        ];
        for (setting, expected) in cases {
            let testenv = TestEnv::new().install_default_cache();
            testenv.append_to_config(format!("display.placeholder_format = \"{setting}\"\n"));
            testenv
                .command()
                .args(["--color=always", "playerctl"])
                .assert()
                .success()
                .stdout(eq(expected));
        }
    }

    #[test]
    fn cli() {
        let testenv = TestEnv::new().install_default_cache();
        testenv.append_to_config("display.placeholder_format = \"both\"\n");
        testenv
            .command()
            .args(["--color=always", "--short-options", "playerctl"])
            .assert()
            .success()
            .stdout(eq(include_str!("rendered/playerctl-short.expected")));

        testenv
            .command()
            .args(["--color=always", "--long-options", "playerctl"])
            .assert()
            .success()
            .stdout(eq(include_str!("rendered/playerctl-long.expected")));

        testenv
            .command()
            .args([
                "--color=always",
                "--short-options",
                "--long-options",
                "playerctl",
            ])
            .assert()
            .success()
            .stdout(eq(include_str!("rendered/playerctl-both.expected")));
    }
}
