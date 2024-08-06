# Changelog

This project follows semantic versioning.

Possible log types:

- `[added]` for new features.
- `[changed]` for changes in existing functionality.
- `[deprecated]` for once-stable features removed in upcoming releases.
- `[removed]` for deprecated features removed in this release.
- `[fixed]` for any bug fixes.
- `[security]` to invite users to upgrade in case of vulnerabilities.
- `[docs]` for documentation changes.
- `[chore]` for maintenance work.


### [v1.7.0][v1.7.0] (2022-10-24)

It's been 20 months since the last release, time for Tealdeer 1.7.0! Thanks to
16 individual contributors, a few nice changes and features are included in
this release.

One change is that you can **query multiple platforms at once**. For example:

    tldr --platform openbsd --platform linux df

This will show the `df` page for OpenBSD (if available), followed by Linux (if
available), with fallback to the current platform on which Tealdeer runs.

What's that `openbsd` thing up there? Yes, there's now **support for the BSD
platforms `freebsd`, `netbsd` and `openbsd`**.

And since we're already talking about platform support: Our **binary releases
now include builds for ARM64 (aka `aarch64`) on macOS (Apple Silicon, M1/M2/M3)
and Linux**. _(Keep in mind that binary releases are generated in CI and are
unsigned. For a trusted build, please compile from source.)_

There's also a breaking change for the folks using [custom pages and
patches](https://dbrgn.github.io/tealdeer/usage_custom_pages.html): These files
now use a `.md` extension. Old files will continue to work, but will result a
deprecation warning being printed when used.

Changes:

- [added] Allow querying multiple platforms ([#300])
- [added] Add BSD platform support ([#354])
- [added] Allow building with native-tls in addition to rustls ([#303])
- [changed] Change custom page files to use a `.md` file extension ([#322])
- [changed] Update to clap v4 for doing command line parsing ([#298])
- [changed] Performance optimization in LineIterator ([#314])
- [changed] Performance optimizations by tweaking Cargo flags ([#355])
- [changed] Include completions in published crate ([#333])
- [changed] Minimal supported Rust version is now 1.75 ([#298])
- [fixed] Fix bash/zsh/fish completions when cache is empty ([#327], [#331])
- [docs] Publish docs only when tagging a release ([#362])
- [docs] List Scoop and Debian packages ([#305], [#315])
- [docs] Add "Tips and Tricks" chapter to user manual ([#342])
- [docs] Various docs improvements ([#293])
- [chore] Improvements to CI workflows ([#324])
- [chore] Update Cargo.toml license field following SPDX 2.1 ([#336])
- [chore] Dependency updates

Contributors to this version:

- [Adam Henley][@adamazing]
- [Andrea Frigido][@frisoft]
- [Blair Noctis][@nc7s]
- [Danilo Bargen][@dbrgn]
- [Felix Yan][@felixonmars]
- [Iliia Maleki][@iliya-malecki]
- [JJ Style][@jj-style]
- [K.B.Dharun Krishna][@kbdharun]
- [Linus Walker][@Walker-00]
- [Mohit Raj][@agrmohit]
- [Nicolai Fröhlich][@nifr]
- [Niklas Mohrin][@niklasmohrin]
- [@qknogxxb][@qknogxxb]
- [@tveness][@tveness]
- [Y.D.X.][@YDX-2147483647]
- [Zacchary Dempsey-Plante][@zedseven]

Thanks!


### [v1.6.1][v1.6.1] (2022-10-24)

Changes:

- [fixed] Fix path source for custom pages dir ([#297])
- [chore] Update dependendencies ([#299])

Contributors to this version:

- [Cyrus Yip][@CyrusYip]
- [Danilo Bargen][@dbrgn]

Thanks!


### [v1.6.0][v1.6.0] (2022-10-02)

It's been 9 months since the last release already! This is not a huge update
feature-wise, but it still contains a few nice new improvements and a few
bugfixes, contributed by 11 different people. The most important new feature is
probably the option to override the cache directory through the config file.
The `TEALDEER_CACHE_DIR` env variable is now deprecated.

A note to packagers: Shell completions have been moved to the `completion/`
subdirectory! Packaging scripts might need to be updated.

Changes:

- [added] Allow overriding cache directory through config ([#276])
- [added] Add `--no-auto-update` CLI flag ([#257])
- [added] Show note about auto-updates when cache is missing ([#254])
- [added] Add support for android platform ([#274])
- [added] Add custom pages to list output ([#285])
- [fixed] Cache: Return error if HTTP client cannot be created ([#247])
- [fixed] Handle cache download errors ([#253])
- [fixed] Do not page output of `tldr --update` ([#231])
- [fixed] Create macOS release builds with bundled root certificates ([#272])
- [fixed] Clean up and fix shell completions ([#262])
- [deprecated] The `TEALDEER_CACHE_DIR` env variable is now deprecated ([#276])
- [removed] The `--config-path` command was removed, use `--show-paths` instead ([#290])
- [removed] The `-o/--os` command was removed, use `-p/--platform` instead ([#290])
- [removed] The `-m/--markdown` command was removed, use `-r/--raw` instead ([#290])
- [chore] Move shell completion scripts to their own directory ([#259])
- [chore] Update dependencies ([#271], [#287], [#291])
- [chore] Use anyhow for error handling ([#249])
- [chore] Switch to Rust 2021 edition ([#284])

Contributors to this version:

- [@bagohart][@bagohart]
- [@cyqsimon][@cyqsimon]
- [Danilo Bargen][@dbrgn]
- [Danny Mösch][@SimplyDanny]
- [Evan Lloyd New-Schmidt][@newsch]
- [Hans Gaiser][@hgaiser]
- [Kian-Meng Ang][@kianmeng]
- [Marcin Puc][@tranzystorek-io]
- [Niklas Mohrin][@niklasmohrin]
- [Olav de Haas][@Olavhaasie]
- [Simon Perdrisat][@gagarine]

Thanks!


### [v1.5.0][v1.5.0] (2021-12-31)

This is quite a big release with many new features. In the 15 months since the
last release, 59 pull requests from 16 different contributors were merged!

The highlights:

- **Custom pages and patches**: You can now create your own local-only tldr
  pages. But not just that, you can also extend existing upstream pages with
  your own examples. For more details, see
  [the docs](https://dbrgn.github.io/tealdeer/usage_custom_pages.html).
- **Change argument parsing from docopt to clap**: We replaced docopt.rs as
  argument parsing library with clap v3, resulting in almost 1 MiB smaller
  binaries and a 22% speed increase when rendering a tldr page.
- **Multi-language support**: You can now override the language with `-L/--language`.
- **A new `--show-paths` command**: By running `tldr --show-paths`, you can list
  the currently used config dir, cache dir, upstream pages dir and custom pages dir.
- **Compliance with the tldr client spec v1.5**: We renamed `-o/--os` to
  `-p/--platform` and implemented transparent lowercasing of the page names.
- **Docs**: The README based documentation has reached its limits. There are
  now new mdbook based docs over at
  [dbrgn.github.io/tealdeer/](https://dbrgn.github.io/tealdeer/), we hope these
  make using tealdeer easier. Of course, documentation improvements are
  welcome! Also, if you're confused about how to use a certain feature, feel
  free to open an issue, this way we can improve the docs.

Note that the MSRV (Minimal Supported Rust Version) of the project
[changed][i190]:

> When publishing a tealdeer release, the Rust version required to build it
> should be stable for at least a month.

Changes:

- [added] Support custom pages and patches ([#142][i142])
- [added] Multi-language support ([#125][i125], [#161][i161])
- [added] Add support for ANSI code and RGB colors ([#148][i148])
- [added] Implement new `--show-paths` command ([#162][i162])
- [added] Support for italic text styling ([#197][i197])
- [added] Allow SunOS platform override ([#176][i176])
- [added] Automatically lowercase page names before lookup ([#227][i227])
- [added] Add "macos" alias for "osx" ([#215][i215])
- [fixed] Consider only standalone command names for styling ([#157][i157])
- [fixed] Fixed and improved zsh completions ([#168][i168])
- [fixed] Create cache directory path if it does not exist ([#174][i174])
- [fixed] Use default style if user-defined style is missing ([#210][i210])
- [changed] Switch from docopt to clap for argument parsing ([#108][i108])
- [changed] Switch from OpenSSL to Rustls ([#187][i187])
- [changed] Performance improvements ([#187][i187])
- [changed] Send all progress logging messages to stderr ([#171][i171])
- [changed] Rename `-o/--os` to `-p/--platform` ([#217][i217])
- [changed] Rename `-m/--markdown` to `-r/--raw` ([#108][i108])
- [deprecated] The `--config-path` command is deprecated, use `--show-paths` instead ([#162][i162])
- [deprecated] The `-o/--os` command is deprecated, use `-p/--platform` instead ([#217][i217])
- [deprecated] The `-m/--markdown` command is deprecated, use `-r/--raw` instead ([#108][i108])
- [docs] New docs at [dbrgn.github.io/tealdeer/](https://dbrgn.github.io/tealdeer/)
- [docs] Add comparative benchmarks with hyperfine ([#163][i163], [README](https://github.com/dbrgn/tealdeer#goals))
- [chore] Download tldr pages archive from their website, not from GitHub ([#213][i213])
- [chore] Bump MSRV to 1.54 and change MSRV policy ([#190][i190])
- [chore] The `master` branch was renamed to `main`
- [chore] All release binaries are now generated in CI. Binaries for macOS and Windows are also provided. ([#240][i240])
- [chore] Update all dependencies

Contributors to this version:

- [@bl-ue][@bl-ue]
- [Cameron Tod][@cam8001]
- [Dalton][@dmaahs2017]
- [Danilo Bargen][@dbrgn]
- [Danny Mösch][@SimplyDanny]
- [Marcin Puc][@tranzystorek-io]
- [Michael Cho][@cho-m]
- [MS_Y][@black7375]
- [Niklas Mohrin][@niklasmohrin]
- [Rithvik Vibhu][@rithvikvibhu]
- [rnd][@0ndorio]
- [Sondre Nilsen][@sondr3]
- [Tomás Farías Santana][@tomasfarias]
- [Tsvetomir Bonev][@invakid404]
- [@tveness][@tveness]
- [ギャラ][@laxect]

Thanks!

Last but not least, [Niklas Mohrin][@niklasmohrin] has joined the project as
co-maintainer. Thank you for your help!


### [v1.4.1][v1.4.1] (2020-09-04)

- [fixed] Syntax error in zsh completion file ([#138][i138])

Contributors to this version:

- [Danilo Bargen][@dbrgn]
- [Bruno A. Muciño][@mucinoab]
- [Francesco][@BachoSeven]

Thanks!


### [v1.4.0][v1.4.0] (2020-09-03)

- [added] Configurable automatic cache updates ([#115][i115])
- [added] Improved color detection and support for `--color` argument and
  `NO_COLOR` env variable ([#111][i111])
- [changed] Make `--list` option comply with official spec ([#112][i112])
- [changed] Move cache age warning to stderr ([#113][i113])

Contributors to this version:

- [Atul Bhosale][@Atul9]
- [Danilo Bargen][@dbrgn]
- [Danny Mösch][@SimplyDanny]
- [Ilaï Deutel][@ilai-deutel]
- [Kornel][@kornelski]
- [@LovecraftianHorror][@LovecraftianHorror]
- [@michaeldel][@michaeldel]
- [Niklas Mohrin][@niklasmohrin]

Thanks!


### [v1.3.0][v1.3.0] (2020-02-28)

- [added] New config option for compact output mode ([#89][i89])
- [added] New -m/--markdown parameter for raw rendering ([#95][i95])
- [added] Provide zsh autocompletion ([#86][i86])
- [changed] Require at least Rust 1.39 to build (previous: 1.32)
- [changed] Switch to GitHub actions, CI testing now covers Windows as well ([#99][i99])
- [changed] Tweak the "outdated cache" warning message ([#97][i97])
- [changed] General maintenance: Upgrade dependencies, fix linter warnings
- [fixed] Fix Fish autocompletion on macOS ([#87][i87])
- [fixed] Fix compilation on Windows by disabling pager ([#99][i99])

Contributors to this version:

- [Bruno Heridet][@Delapouite]
- [Danilo Bargen][@dbrgn]
- [Hugo Locurcio][@Calinou]
- [Isak Johansson][@Plommonsorbet]
- [James Doyle][@james2doyle]
- [Jesús Trinidad Díaz Ramírez][@jesdazrez]
- [@korrat][@korrat]
- [Marc-André Renaud][@ma-renaud]

Thanks!


### [v1.2.0][v1.2.0] (2019-08-10)

- [added] Add Windows support ([#77][i77])
- [added] Add support for spaces in commands ([#75][i75])
- [added] Add support for Fish-based autocompletion ([#71][i71])
- [added] Add pager support ([#44][i44])
- [added] Print detected OS with `-v` / `--version` ([#57][i57])
- [changed] OS detection: Treat BSDs as "osx" ([#58][i58])
- [changed] Move from curl to reqwest ([#61][i61])
- [changed] Move to Rust 2018, require Rust 1.32 ([#69][i69] / [#84][i84])
- [fixed] Add (back) support for proxies ([#68][i68])

Contributors to this version:

- [Bar Hatsor][@Bassets]
- [Danilo Bargen][@dbrgn]
- [Gabriel Martinez][@mystal]
- [Ivan Smirnov][@aldanor]
- [Jan Christian Grünhage][@jcgruenhage]
- [Jonathan Dahan][@jedahan]
- [Juan D. Vega][@jdvr]
- [Natalie Pendragon][@natpen]
- [Raphael Das Gupta][@das-g]

Thanks!


### [v1.1.0][v1.1.0] (2018-10-22)

- [added] Configuration file support ([#43][i43])
- [added] Allow configuration of colors/style ([#43][i43])
- [added] New `--quiet` / `-q` option to suppress most non-error messages ([#48][i48])
- [changed] Require at least Rust 1.28 to build (previous: 1.19)
- [fixed] Fix building on systems with openssl 1.1.1 ([#47][i47])

Contributors to this version:

- [Danilo Bargen][@dbrgn]
- [@equal-l2][@equal-l2]
- [Jonathan Dahan][@jedahan]
- [Lukas Bergdoll][@Voultapher]

Thanks!


### [v1.0.0][v1.0.0] (2018-02-11)

- [added] Include bash completions ([#34][i34])
- [changed] Update all dependencies
- [changed] Require at least Rust 1.19 to build (previous: 1.9)
- [changed] Improved unit/integration testing


### v0.4.0 (2016-11-25)

- [added] Support for new page format
- [changed] Update all dependencies


### v0.3.0 (2016-08-01)

- [changed] Update curl dependency


### v0.2.0 (2016-04-16)

- First crates.io release



[@0ndorio]: https://github.com/0ndorio
[@adamazing]: https://github.com/adamazing
[@agrmohit]: https://github.com/agrmohit
[@aldanor]: https://github.com/aldanor
[@Atul9]: https://github.com/Atul9
[@BachoSeven]: https://github.com/BachoSeven
[@bagohart]: https://github.com/bagohart
[@Bassets]: https://github.com/Bassets
[@black7375]: https://github.com/black7375
[@bl-ue]: https://github.com/bl-ue
[@Calinou]: https://github.com/Calinou
[@cam8001]: https://github.com/cam8001
[@cho-m]: https://github.com/cho-m
[@cyqsimon]: https://github.com/cyqsimon
[@CyrusYip]: https://github.com/CyrusYip
[@das-g]: https://github.com/das-g
[@dbrgn]: https://github.com/dbrgn
[@Delapouite]: https://github.com/Delapouite
[@dmaahs2017]: https://github.com/dmaahs2017
[@equal-l2]: https://github.com/equal-l2
[@felixonmars]: https://github.com/felixonmars
[@frisoft]: https://github.com/frisoft
[@gagarine]: https://github.com/gagarine
[@hgaiser]: https://github.com/hgaiser
[@ilai-deutel]: https://github.com/ilai-deutel
[@iliya-malecki]: https://github.com/iliya-malecki
[@invakid404]: https://github.com/invakid404
[@james2doyle]: https://github.com/james2doyle
[@jcgruenhage]: https://github.com/jcgruenhage
[@jdvr]: https://github.com/jdvr
[@jedahan]: https://github.com/jedahan
[@jesdazrez]: https://github.com/jesdazrez
[@jj-style]: https://github.com/jj-style
[@kbdharun]: https://github.com/kbdharun
[@kianmeng]: https://github.com/kianmeng
[@kornelski]: https://github.com/kornelski
[@korrat]: https://github.com/korrat
[@laxect]: https://github.com/laxect
[@LovecraftianHorror]: https://github.com/LovecraftianHorror
[@ma-renaud]: https://github.com/ma-renaud
[@michaeldel]: https://github.com/michaeldel
[@mucinoab]: https://github.com/mucinoab
[@mystal]: https://github.com/mystal
[@natpen]: https://github.com/natpen
[@nc7s]: https://github.com/nc7s
[@newsch]: https://github.com/newsch
[@nifr]: https://github.com/nifr
[@niklasmohrin]: https://github.com/niklasmohrin
[@Olavhaasie]: https://github.com/Olavhaasie
[@Plommonsorbet]: https://github.com/Plommonsorbet
[@qknogxxb]: https://github.com/qknogxxb
[@rithvikvibhu]: https://github.com/rithvikvibhu
[@SimplyDanny]: https://github.com/SimplyDanny
[@sondr3]: https://github.com/sondr3
[@tomasfarias]: https://github.com/tomasfarias
[@tranzystorek-io]: https://github.com/tranzystorek-io
[@tveness]: https://github.com/tveness
[@Voultapher]: https://github.com/Voultapher
[@Walker-00]: https://github.com/Walker-00
[@YDX-2147483647]: https://github.com/YDX-2147483647
[@zedseven]: https://github.com/zedseven

[v1.0.0]: https://github.com/dbrgn/tealdeer/compare/v0.4.0...v1.0.0
[v1.1.0]: https://github.com/dbrgn/tealdeer/compare/v1.0.0...v1.1.0
[v1.2.0]: https://github.com/dbrgn/tealdeer/compare/v1.1.0...v1.2.0
[v1.3.0]: https://github.com/dbrgn/tealdeer/compare/v1.2.0...v1.3.0
[v1.4.0]: https://github.com/dbrgn/tealdeer/compare/v1.3.0...v1.4.0
[v1.4.1]: https://github.com/dbrgn/tealdeer/compare/v1.4.0...v1.4.1
[v1.5.0]: https://github.com/dbrgn/tealdeer/compare/v1.4.1...v1.5.0
[v1.6.0]: https://github.com/dbrgn/tealdeer/compare/v1.5.0...v1.6.0
[v1.6.1]: https://github.com/dbrgn/tealdeer/compare/v1.6.0...v1.6.1
[v1.7.0]: https://github.com/dbrgn/tealdeer/compare/v1.6.1...v1.7.0

[i34]: https://github.com/dbrgn/tealdeer/issues/34
[i43]: https://github.com/dbrgn/tealdeer/issues/43
[i44]: https://github.com/dbrgn/tealdeer/issues/44
[i47]: https://github.com/dbrgn/tealdeer/issues/47
[i48]: https://github.com/dbrgn/tealdeer/issues/48
[i57]: https://github.com/dbrgn/tealdeer/issues/57
[i58]: https://github.com/dbrgn/tealdeer/issues/58
[i61]: https://github.com/dbrgn/tealdeer/issues/61
[i68]: https://github.com/dbrgn/tealdeer/issues/68
[i69]: https://github.com/dbrgn/tealdeer/issues/69
[i71]: https://github.com/dbrgn/tealdeer/issues/71
[i75]: https://github.com/dbrgn/tealdeer/issues/75
[i77]: https://github.com/dbrgn/tealdeer/issues/77
[i84]: https://github.com/dbrgn/tealdeer/issues/84
[i86]: https://github.com/dbrgn/tealdeer/issues/86
[i87]: https://github.com/dbrgn/tealdeer/issues/87
[i89]: https://github.com/dbrgn/tealdeer/issues/89
[i95]: https://github.com/dbrgn/tealdeer/issues/95
[i97]: https://github.com/dbrgn/tealdeer/issues/97
[i99]: https://github.com/dbrgn/tealdeer/issues/99
[i108]: https://github.com/dbrgn/tealdeer/pull/108
[i111]: https://github.com/dbrgn/tealdeer/issues/111
[i112]: https://github.com/dbrgn/tealdeer/issues/112
[i113]: https://github.com/dbrgn/tealdeer/issues/113
[i115]: https://github.com/dbrgn/tealdeer/issues/115
[i125]: https://github.com/dbrgn/tealdeer/pull/125
[i138]: https://github.com/dbrgn/tealdeer/issues/138
[i142]: https://github.com/dbrgn/tealdeer/pull/142
[i148]: https://github.com/dbrgn/tealdeer/pull/148
[i157]: https://github.com/dbrgn/tealdeer/pull/157
[i161]: https://github.com/dbrgn/tealdeer/pull/161
[i162]: https://github.com/dbrgn/tealdeer/pull/162
[i163]: https://github.com/dbrgn/tealdeer/pull/163
[i168]: https://github.com/dbrgn/tealdeer/pull/168
[i171]: https://github.com/dbrgn/tealdeer/pull/171
[i174]: https://github.com/dbrgn/tealdeer/pull/174
[i176]: https://github.com/dbrgn/tealdeer/pull/176
[i187]: https://github.com/dbrgn/tealdeer/pull/187
[i190]: https://github.com/dbrgn/tealdeer/issues/190
[i197]: https://github.com/dbrgn/tealdeer/pull/197
[i210]: https://github.com/dbrgn/tealdeer/pull/210
[i213]: https://github.com/dbrgn/tealdeer/pull/213
[i215]: https://github.com/dbrgn/tealdeer/pull/215
[i217]: https://github.com/dbrgn/tealdeer/pull/217
[i227]: https://github.com/dbrgn/tealdeer/pull/227
[#231]: https://github.com/dbrgn/tealdeer/pull/231
[i240]: https://github.com/dbrgn/tealdeer/pull/240
[#247]: https://github.com/dbrgn/tealdeer/pull/247
[#249]: https://github.com/dbrgn/tealdeer/pull/249
[#253]: https://github.com/dbrgn/tealdeer/pull/253
[#254]: https://github.com/dbrgn/tealdeer/pull/254
[#257]: https://github.com/dbrgn/tealdeer/pull/257
[#259]: https://github.com/dbrgn/tealdeer/pull/259
[#262]: https://github.com/dbrgn/tealdeer/pull/262
[#271]: https://github.com/dbrgn/tealdeer/pull/271
[#272]: https://github.com/dbrgn/tealdeer/pull/272
[#274]: https://github.com/dbrgn/tealdeer/pull/274
[#276]: https://github.com/dbrgn/tealdeer/pull/276
[#284]: https://github.com/dbrgn/tealdeer/pull/284
[#285]: https://github.com/dbrgn/tealdeer/pull/285
[#287]: https://github.com/dbrgn/tealdeer/pull/287
[#290]: https://github.com/dbrgn/tealdeer/pull/290
[#291]: https://github.com/dbrgn/tealdeer/pull/291
[#293]: https://github.com/dbrgn/tealdeer/pull/293
[#297]: https://github.com/dbrgn/tealdeer/pull/297
[#298]: https://github.com/dbrgn/tealdeer/pull/298
[#299]: https://github.com/dbrgn/tealdeer/pull/299
[#300]: https://github.com/dbrgn/tealdeer/pull/300
[#303]: https://github.com/dbrgn/tealdeer/pull/303
[#305]: https://github.com/dbrgn/tealdeer/pull/305
[#314]: https://github.com/dbrgn/tealdeer/pull/314
[#315]: https://github.com/dbrgn/tealdeer/pull/315
[#322]: https://github.com/dbrgn/tealdeer/pull/322
[#324]: https://github.com/dbrgn/tealdeer/pull/324
[#327]: https://github.com/dbrgn/tealdeer/pull/327
[#331]: https://github.com/dbrgn/tealdeer/pull/331
[#333]: https://github.com/dbrgn/tealdeer/pull/333
[#336]: https://github.com/dbrgn/tealdeer/pull/336
[#342]: https://github.com/dbrgn/tealdeer/pull/342
[#354]: https://github.com/dbrgn/tealdeer/pull/354
[#355]: https://github.com/dbrgn/tealdeer/pull/355
[#362]: https://github.com/dbrgn/tealdeer/pull/362
