# Configuration

Tealdeer can be customized with a config file in [TOML
format](https://toml.io/) called `config.toml`.

## Configfile Path

The configuration file path follows OS conventions (e.g.
`$XDG_CONFIG_HOME/tealdeer/config.toml` on Unix). The paths can be queried
with the following command:

    $ tldr --show-paths

Creating the config file can be done manually or with the help of `tldr`:

    $ tldr --seed-config

On Unix, this will usually be `~/.config/tealdeer/config.toml`.

## Config Example

Here's an example configuration file. Note that this example does not contain
all possible config options. For details on the things that can be configured,
please refer to the subsections of this documentation page
([display](config_display.html), [style](config_style.html),
[updates](config_updates.html) or [directories](config_directories.html)).

```toml
[display]
compact = false
use_pager = true

[style.command_name]
foreground = "red"

[style.example_text]
foreground = "green"

[style.example_code]
foreground = "blue"

[style.example_variable]
foreground = "blue"
underline = true

[updates]
auto_update = true
```

## Override Config Directory

The directory where the configuration file resides may be overwritten by the
environment variable `TEALDEER_CONFIG_DIR`. Remember to use an absolute path.
Variable expansion will not be performed on the path.
