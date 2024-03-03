# Custom Pages and Patches

<!--
TODO: Uncomment when releasing 1.7.0.
> ⚠️ **Breaking change in version 1.7.0:** The file name extension for custom
> pages and patches was changed:
>
> - `<name>.page` → `<name>.page.md`
> - `<name>.patch` → `<name>.patch.md`
>
> If you have custom pages or patches, you need to rename them.
-->

Tealdeer allows creating new custom pages, overriding existing pages as well as
extending existing pages.

The directory, where these custom pages and patches can be placed, follows OS
conventions. On Unix for instance, the default location is
`~/.local/share/tealdeer/pages/`. To print the path used on your system, simply
run `tldr --show-paths`.

The custom pages directory can be [overridden by the config
file](config_directories.html).

## Custom Pages

To document internal command line tools, or if you want to replace an existing
tldr page with one that's better suited for you, place a file with the name
`<command>.page.md` in the custom pages directory. When calling `tldr <command>`,
your custom page will be shown instead of the upstream version in the cache.

Path:

    $CUSTOM_PAGES_DIR/<command>.page.md

Example:

    ~/.local/share/tealdeer/pages/ufw.page.md

## Custom Patches

Sometimes you don't want to fully replace an existing upstream page, but just
want to extend it with your own examples that you frequently need. In this
case, use a file called `<command>.patch.md`, it will be appended to existing
pages.

Path:

    $CUSTOM_PAGES_DIR/<command>.patch.md

Example:

    ~/.local/share/tealdeer/pages/ufw.patch.md
