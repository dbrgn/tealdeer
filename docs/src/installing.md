# Installing

There are a few different ways to install tealdeer:

- Through [package managers](#package-managers)
- Through [static binaries](#static-binaries-linux)
- Through [cargo install](#through-cargo-install)
- By [building from source](#build-from-source)

Additionally, when not using system packages, you can [manually install
autocompletions](#autocompletion) and set up a [background cache
update](#updating-the-cache-in-the-background-systemd).

## Package Managers

Tealdeer has been added to a few package managers:

- Arch Linux: [`tealdeer`](https://archlinux.org/packages/extra/x86_64/tealdeer/)
- Debian: [`tealdeer`](https://tracker.debian.org/tealdeer)
- Fedora: [`tealdeer`](https://src.fedoraproject.org/rpms/rust-tealdeer)
- FreeBSD: [`sysutils/tealdeer`](https://www.freshports.org/sysutils/tealdeer/)
- Funtoo: [`app-misc/tealdeer`](https://github.com/funtoo/core-kit/tree/1.4-release/app-misc/tealdeer)
- Homebrew: [`tealdeer`](https://formulae.brew.sh/formula/tealdeer)
- MacPorts: [`tealdeer`](https://ports.macports.org/port/tealdeer/)
- NetBSD: [`sysutils/tealdeer`](https://pkgsrc.se/sysutils/tealdeer)
- Nix: [`tealdeer`](https://search.nixos.org/packages?query=tealdeer)
- openSUSE: [`tealdeer`](https://software.opensuse.org/package/tealdeer?search_term=tealdeer)
- Scoop: [`tealdeer`](https://github.com/ScoopInstaller/Main/blob/master/bucket/tealdeer.json)
- Solus: [`tealdeer`](https://packages.getsol.us/shannon/t/tealdeer/)
- Void Linux: [`tealdeer`](https://github.com/void-linux/void-packages/tree/master/srcpkgs/tealdeer)

## Static Binaries (Linux)

Static binary builds (currently for Linux only) are available on the
[GitHub releases page](https://github.com/tealdeer-rs/tealdeer/releases).
Simply download the binary for your platform and run it!

## Through `cargo install`

Build and install the tool via cargo...

```shell
$ cargo install tealdeer
```

## Build From Source

Release build:

```shell
$ cargo build --release
```

Release build with native TLS support:

```shell
$ cargo build --release --features native-tls
```

Debug build with logging support:

```shell
$ cargo build --features logging
```

(To enable logging at runtime, export the `RUST_LOG=tldr=debug` env variable.)

## Autocompletion

Shell completion scripts are located in the folder `completion`.
Just copy them to their designated location:

- *Bash*: `cp completion/bash_tealdeer /usr/share/bash-completion/completions/tldr`
- *Fish*: `cp completion/fish_tealdeer ~/.config/fish/completions/tldr.fish`
- *Zsh*: `cp completion/zsh_tealdeer /usr/share/zsh/site-functions/_tldr`

## Updating the cache in the background (systemd)

The [`auto_update`](config_updates.md#auto_update) setting refreshes the cache
while you run a `tldr` command, which means that command occasionally blocks for
a few seconds. If you would rather keep the cache fresh out of band, the
`systemd` folder ships a timer and a service that run `tldr --update` on a
schedule.

Because the cache lives in your user's cache directory, these are *user* units,
so install them for your account rather than system wide:

```shell
$ cp systemd/tealdeer-update.service systemd/tealdeer-update.timer ~/.config/systemd/user/
$ systemctl --user daemon-reload
$ systemctl --user enable --now tealdeer-update.timer
```

The timer fires weekly by default and is `Persistent`, so a run that was missed
while the machine was off happens on the next boot. `tldr --update` just prints
an error and exits if the network is down, and the timer tries again on its next
run, so there is no separate `network-online.target` dependency to wire up (that
target is only managed by the system instance, not the per-user one anyway).
Adjust `OnCalendar` in the timer to taste.

The `.service` file calls `/usr/bin/tldr`, which is where distribution packages
put the binary. If you installed tealdeer another way, edit its `ExecStart` to
match (for example `%h/.cargo/bin/tldr` for `cargo install`).
