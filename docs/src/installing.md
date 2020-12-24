# Installing

There are a few different ways to install Tealdeer:

- Through [packet managers](#packet-managers)
- Through [static binaries](#static-binaries-linux)
- Through [cargo install](#through-cargo-install)
- By [building from source](#build-from-source)

Additionally, when not using system packages, you can [manually install
autocompletions](#autocompletion).

## Packet Managers

Tealdeer has been added to a few package managers:

- Arch Linux AUR: [`tealdeer`](https://aur.archlinux.org/packages/tealdeer/),
  [`tealdeer-bin`](https://aur.archlinux.org/packages/tealdeer-bin/) or
  [`tealdeer-git`](https://aur.archlinux.org/packages/tealdeer-git/)
- Fedora: [`tealdeer`](https://src.fedoraproject.org/rpms/rust-tealdeer)
- FreeBSD: [`sysutils/tealdeer`](https://www.freshports.org/sysutils/tealdeer/)
- macOS Homebrew: [`tealdeer`](https://formulae.brew.sh/formula/tealdeer)
- NetBSD: [`sysutils/tealdeer`](https://pkgsrc.se/sysutils/tealdeer)
- Nix: [`tealdeer`](https://nixos.org/nixos/packages.html#tealdeer)
- openSUSE: [`tealdeer`](https://software.opensuse.org/package/tealdeer?search_term=tealdeer)
- Solus: [`tealdeer`](https://packages.getsol.us/shannon/t/tealdeer/)
- Void Linux: [`tealdeer`](https://github.com/void-linux/void-packages/tree/master/srcpkgs/tealdeer)

## Static Binaries (Linux)

Static binary builds (currently for Linux only) are available on the
[GitHub releases page](https://github.com/dbrgn/tealdeer/releases).
Simply download the binary for your platform and run it!

## Through `cargo install`

Build and install the tool via cargo...

    $ cargo install tealdeer

*(Note: You might need to install OpenSSL development headers, otherwise you get
a "failed to run custom build command for openssl-sys" error message. The
package is called `libssl-dev` on Ubuntu.)*

## Build From Source

Debug build with logging enabled:

    $ cargo build --features logging

Release build without logging:

    $ cargo build --release

To enable the log output, set the `RUST_LOG` env variable:

    $ export RUST_LOG=tldr=debug

## Autocompletion

- *Bash*: copy `bash_tealdeer` to `/usr/share/bash-completion/completions/tldr`
- *Fish*: copy `fish_tealdeer` to `~/.config/fish/completions/tldr.fish`
- *Zsh*: copy `zsh_tealdeer` to `/usr/share/zsh/site-functions/_tldr`
