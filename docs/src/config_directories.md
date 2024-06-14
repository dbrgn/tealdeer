# Section: \[directories\]

This section allows overriding some directory paths.

## `cache_dir`

Override the cache directory. Remember to use an absolute path. Variable
expansion will not be performed on the path. If the directory does not yet
exist, it will be created.

    [directories]
    cache_dir = "/home/myuser/.tealdeer-cache/"

If no `cache_dir` is specified, tealdeer will fall back to a location that
follows OS conventions. On Unix, it will usually be at `~/.cache/tealdeer/`.
Use `tldr --show-paths` to show the path that is being used.

## `custom_pages_dir`

Set the directory to be used to look up [custom
pages](usage_custom_pages.html). Remember to use an absolute path. Variable
expansion will not be performed on the path.

    [directories]
    custom_pages_dir = "/home/myuser/custom-tldr-pages/"
