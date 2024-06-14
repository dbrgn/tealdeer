//! Integration tests.

use std::{
    fs::{create_dir_all, File},
    io::Write,
    process::Command,
    time::{Duration, SystemTime},
};

use assert_cmd::prelude::*;
use predicates::{
    boolean::PredicateBooleanExt,
    prelude::predicate::str::{contains, diff, is_empty, is_match},
};
use tempfile::{Builder as TempfileBuilder, TempDir};

// TODO: Should be 'cache::CACHE_DIR_ENV_VAR'. This requires to have a library crate for the logic.
static CACHE_DIR_ENV_VAR: &str = "TEALDEER_CACHE_DIR";

pub static TLDR_PAGES_DIR: &str = "tldr-pages";

struct TestEnv {
    pub cache_dir: TempDir,
    pub custom_pages_dir: TempDir,
    pub config_dir: TempDir,
    pub input_dir: TempDir,
    pub default_features: bool,
    pub features: Vec<String>,
}

impl TestEnv {
    fn new() -> Self {
        TestEnv {
            cache_dir: TempfileBuilder::new()
                .prefix(".tldr.test.cache")
                .tempdir()
                .unwrap(),
            config_dir: TempfileBuilder::new()
                .prefix(".tldr.test.conf")
                .tempdir()
                .unwrap(),
            custom_pages_dir: TempfileBuilder::new()
                .prefix(".tldr.test.custom-pages")
                .tempdir()
                .unwrap(),
            input_dir: TempfileBuilder::new()
                .prefix(".tldr.test.input")
                .tempdir()
                .unwrap(),
            default_features: true,
            features: vec![],
        }
    }

    /// Write `content` to "config.toml" in the `config_dir` directory
    fn write_config(&self, content: impl AsRef<str>) {
        let config_file_name = self.config_dir.path().join("config.toml");
        println!("Config path: {config_file_name:?}");

        let mut config_file = File::create(&config_file_name).unwrap();
        config_file.write_all(content.as_ref().as_bytes()).unwrap();
    }

    /// Add entry for that environment to the "common" pages.
    fn add_entry(&self, name: &str, contents: &str) {
        self.add_os_entry("common", name, contents);
    }

    /// Add entry for that environment to an OS-specific subfolder.
    fn add_os_entry(&self, os: &str, name: &str, contents: &str) {
        let dir = self
            .cache_dir
            .path()
            .join(TLDR_PAGES_DIR)
            .join("pages")
            .join(os);
        create_dir_all(&dir).unwrap();

        let mut file = File::create(dir.join(format!("{name}.md"))).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    /// Add custom patch entry to the custom_pages_dir
    fn add_page_entry(&self, name: &str, contents: &str) {
        let dir = self.custom_pages_dir.path();
        create_dir_all(dir).unwrap();
        let mut file = File::create(dir.join(format!("{name}.page.md"))).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    /// Add custom patch entry to the custom_pages_dir
    fn add_patch_entry(&self, name: &str, contents: &str) {
        let dir = self.custom_pages_dir.path();
        create_dir_all(dir).unwrap();
        let mut file = File::create(dir.join(format!("{name}.patch.md"))).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    /// Disable default features.
    #[allow(dead_code)] // Might be useful in the future
    fn no_default_features(mut self) -> Self {
        self.default_features = false;
        self
    }

    /// Add the specified feature.
    #[allow(dead_code)] // Might be useful in the future
    fn with_feature<S: Into<String>>(mut self, feature: S) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Return a new `Command` with env vars set.
    fn command(&self) -> Command {
        let mut build = escargot::CargoBuild::new()
            .bin("tldr")
            .current_release()
            .current_target();
        if !self.default_features {
            build = build.arg("--no-default-features");
        }
        if !self.features.is_empty() {
            build = build.arg(format!("--feature {}", self.features.join(",")));
        }
        let run = build.run().unwrap();
        let mut cmd = run.command();
        cmd.env(CACHE_DIR_ENV_VAR, self.cache_dir.path().to_str().unwrap());
        cmd.env(
            "TEALDEER_CONFIG_DIR",
            self.config_dir.path().to_str().unwrap(),
        );
        cmd
    }
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
fn test_update_cache() {
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
fn test_quiet_failures() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["--update", "-q"])
        .assert()
        .success()
        .stdout(is_empty());

    testenv
        .command()
        .args(["fakeprogram", "-q"])
        .assert()
        .failure()
        .stdout(is_empty());
}

#[test]
fn test_quiet_old_cache() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["--update", "-q"])
        .assert()
        .success()
        .stdout(is_empty());

    filetime::set_file_mtime(
        testenv.cache_dir.path().join(TLDR_PAGES_DIR),
        filetime::FileTime::from_unix_time(1, 0),
    )
    .unwrap();

    testenv
        .command()
        .args(["tldr"])
        .assert()
        .success()
        .stderr(contains("The cache hasn't been updated for "));

    testenv
        .command()
        .args(["tldr", "--quiet"])
        .assert()
        .success()
        .stderr(contains("The cache hasn't been updated for ").not());
}

#[test]
fn test_create_cache_directory_path() {
    let testenv = TestEnv::new();
    let cache_dir = testenv.cache_dir.path();
    let internal_cache_dir = cache_dir.join("internal");

    let mut command = testenv.command();
    command.env(CACHE_DIR_ENV_VAR, internal_cache_dir.to_str().unwrap());

    assert!(!internal_cache_dir.exists());

    command
        .arg("-u")
        .assert()
        .success()
        .stderr(contains(format!(
            "Successfully created cache directory path `{}`.",
            internal_cache_dir.to_str().unwrap()
        )))
        .stderr(contains("Successfully updated cache."));

    assert!(internal_cache_dir.is_dir());
}

#[test]
fn test_cache_location_not_a_directory() {
    let testenv = TestEnv::new();
    let cache_dir = testenv.cache_dir.path();
    let internal_file = cache_dir.join("internal");
    File::create(&internal_file).unwrap();

    let mut command = testenv.command();
    command.env(CACHE_DIR_ENV_VAR, internal_file.to_str().unwrap());

    command
        .arg("-u")
        .assert()
        .failure()
        .stderr(contains(format!(
            "Cache directory path `{}` is not a directory",
            internal_file.display(),
        )))
        .stderr(contains(
            "Warning: The $TEALDEER_CACHE_DIR env variable is deprecated",
        ));
}

#[test]
fn test_cache_location_source() {
    let testenv = TestEnv::new();
    let default_cache_dir = testenv.cache_dir.path();
    let tmp_cache_dir = TempfileBuilder::new()
        .prefix(".tldr.test.cache_dir")
        .tempdir()
        .unwrap();

    // Source: Default (OS convention)
    let mut command = testenv.command();
    command.env_remove(CACHE_DIR_ENV_VAR);
    command
        .arg("--show-paths")
        .assert()
        .success()
        .stdout(is_match("\nCache dir:        [^(]* \\(XDG convention\\)\n").unwrap());

    // Source: Config variable
    let mut command = testenv.command();
    command.env_remove(CACHE_DIR_ENV_VAR);
    testenv.write_config(format!(
        "[directories]\ncache_dir = '{}'",
        tmp_cache_dir.path().to_str().unwrap(),
    ));
    command
        .arg("--show-paths")
        .assert()
        .success()
        .stdout(is_match("\nCache dir:        [^(]* \\(config file\\)\n").unwrap());

    // Source: Env var
    let mut command = testenv.command();
    command.env(CACHE_DIR_ENV_VAR, default_cache_dir.to_str().unwrap());
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
        .success()
        .stderr(contains("Successfully created seed config file here"));
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
            testenv.config_dir.path().to_str().unwrap(),
        )))
        .stdout(contains(format!(
            "Config path:      {}",
            testenv
                .config_dir
                .path()
                .join("config.toml")
                .to_str()
                .unwrap(),
        )))
        .stdout(contains(format!(
            "Cache dir:        {}",
            testenv.cache_dir.path().to_str().unwrap(),
        )))
        .stdout(contains(format!(
            "Pages dir:        {}",
            testenv
                .cache_dir
                .path()
                .join(TLDR_PAGES_DIR)
                .to_str()
                .unwrap(),
        )));

    // Set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

    // Now ensure that this path is contained in the output
    testenv
        .command()
        .args(["--show-paths"])
        .assert()
        .success()
        .stdout(contains(format!(
            "Custom pages dir: {}",
            testenv.custom_pages_dir.path().to_str().unwrap(),
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
fn test_markdown_rendering() {
    let testenv = TestEnv::new();

    testenv.add_entry("which", include_str!("which-markdown.expected"));

    let expected = include_str!("which-markdown.expected");
    testenv
        .command()
        .args(["--raw", "which"])
        .assert()
        .success()
        .stdout(diff(expected));
}

fn _test_correct_rendering(
    input_file: &str,
    filename: &str,
    expected: &'static str,
    color_option: &str,
) {
    let testenv = TestEnv::new();

    // Create input file
    let file_path = testenv.input_dir.path().join(filename);
    println!("Testfile path: {file_path:?}");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(input_file.as_bytes()).unwrap();

    testenv
        .command()
        .args(["--color", color_option, "-f", file_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(diff(expected));
}

/// An end-to-end integration test for direct file rendering (v1 syntax).
#[test]
fn test_correct_rendering_v1() {
    _test_correct_rendering(
        include_str!("inkscape-v1.md"),
        "inkscape-v1.md",
        include_str!("inkscape-default.expected"),
        "always",
    );
}

/// An end-to-end integration test for direct file rendering (v2 syntax).
#[test]
fn test_correct_rendering_v2() {
    _test_correct_rendering(
        include_str!("inkscape-v2.md"),
        "inkscape-v2.md",
        include_str!("inkscape-default.expected"),
        "always",
    );
}

#[test]
/// An end-to-end integration test for direct file rendering with the `--color auto` option. This
/// will not use styling since output is not stdout.
fn test_rendering_color_auto() {
    _test_correct_rendering(
        include_str!("inkscape-v2.md"),
        "inkscape-v2.md",
        include_str!("inkscape-default-no-color.expected"),
        "auto",
    );
}

#[test]
/// An end-to-end integration test for direct file rendering with the `--color never` option.
fn test_rendering_color_never() {
    _test_correct_rendering(
        include_str!("inkscape-v2.md"),
        "inkscape-v2.md",
        include_str!("inkscape-default-no-color.expected"),
        "never",
    );
}

#[test]
fn test_rendering_i18n() {
    _test_correct_rendering(
        include_str!("chmod.ru.md"),
        "chmod.ru.md",
        include_str!("chmod.ru.expected"),
        "always",
    );
}

/// An end-to-end integration test for rendering with custom syntax config.
#[test]
fn test_correct_rendering_with_config() {
    let testenv = TestEnv::new();

    // Setup config file
    // TODO should be config::CONFIG_FILE_NAME
    let config_file_path = testenv.config_dir.path().join("config.toml");
    println!("Config path: {config_file_path:?}");

    let mut config_file = File::create(&config_file_path).unwrap();
    config_file
        .write_all(include_bytes!("config.toml"))
        .unwrap();

    // Create input file
    let file_path = testenv.input_dir.path().join("inkscape-v2.md");
    println!("Testfile path: {file_path:?}");

    let mut file = File::create(&file_path).unwrap();
    file.write_all(include_bytes!("inkscape-v2.md")).unwrap();

    // Load expected output
    let expected = include_str!("inkscape-with-config.expected");

    testenv
        .command()
        .args(["--color", "always", "-f", file_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(diff(expected));
}

#[test]
fn test_spaces_find_command() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

    testenv
        .command()
        .args(["git", "checkout"])
        .assert()
        .success();
}

#[test]
fn test_pager_flag_enable() {
    let testenv = TestEnv::new();

    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

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
fn test_list_flag_rendering() {
    let testenv = TestEnv::new();

    // set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

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
    let testenv = TestEnv::new();

    // set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

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

    let config_file_path = testenv.config_dir.path().join("config.toml");
    let cache_file_path = testenv.cache_dir.path().join(TLDR_PAGES_DIR);

    // Activate automatic updates, set the auto-update interval to 24 hours
    let mut config_file = File::create(config_file_path).unwrap();
    config_file
        .write_all(b"[updates]\nauto_update = true\nauto_update_interval_hours = 24")
        .unwrap();
    config_file.flush().unwrap();

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

/// End-end test to ensure .page.md files overwrite pages in cache_dir
#[test]
fn test_custom_page_overwrites() {
    let testenv = TestEnv::new();

    // set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

    // Add file that should be ignored to the cache dir
    testenv.add_entry("inkscape-v2", "");
    // Add .page.md file to custom_pages_dir
    testenv.add_page_entry("inkscape-v2", include_str!("inkscape-v2.md"));

    // Load expected output
    let expected = include_str!("inkscape-default-no-color.expected");

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
    let testenv = TestEnv::new();

    // set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

    // Add page to the cache dir
    testenv.add_entry("inkscape-v2", include_str!("inkscape-v2.md"));
    // Add .page.md file to custom_pages_dir
    testenv.add_patch_entry("inkscape-v2", include_str!("inkscape-v2.patch.md"));

    // Load expected output
    let expected = include_str!("inkscape-patched-no-color.expected");

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
    let testenv = TestEnv::new();

    // set custom pages directory
    testenv.write_config(format!(
        "[directories]\ncustom_pages_dir = '{}'",
        testenv.custom_pages_dir.path().to_str().unwrap()
    ));

    testenv.add_entry("test", "");

    // Add page to the cache dir
    testenv.add_page_entry("inkscape-v2", include_str!("inkscape-v2.md"));
    // Add .page.md file to custom_pages_dir
    testenv.add_patch_entry("inkscape-v2", include_str!("inkscape-v2.patch.md"));

    // Load expected output
    let expected = include_str!("inkscape-default-no-color.expected");

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
    let testenv = TestEnv::new();
    testenv
        .command()
        .args(["--update"])
        .assert()
        .success()
        .stderr(contains("Successfully updated cache."));

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
    let testenv = TestEnv::new();

    // Create input file
    let file_path = testenv.input_dir.path().join("inkscape.md");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(include_bytes!("inkscape-v1.md")).unwrap();

    // Base args
    let mut args = vec!["--color", "never", "-f", file_path.to_str().unwrap()];

    // Default render
    testenv
        .command()
        .args(&args)
        .assert()
        .success()
        .stdout(diff(include_str!("inkscape-default-no-color.expected")));

    // Raw render
    args.push("--raw");
    testenv
        .command()
        .args(&args)
        .assert()
        .success()
        .stdout(diff(include_str!("inkscape-v1.md")));
}
