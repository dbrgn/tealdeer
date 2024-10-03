# Section: \[updates\]

## Automatic updates

Tealdeer can refresh the cache automatically when it is outdated. This
behavior can be configured in the `updates` section and is disabled by
default.

### `auto_update`

Specifies whether the auto-update feature should be enabled (defaults to
`false`).

    [updates]
    auto_update = true

### `auto_update_interval_hours`

Duration, since the last cache update, after which the cache will be
refreshed (defaults to 720 hours). This parameter is ignored if `auto_update`
is set to `false`.

    [updates]
    auto_update = true
    auto_update_interval_hours = 24

### archive_url

URL for the location of tldr pages archive. Default is the main `thdr.sh`
archive location.

    [updates]
    auto_update = true
    auto_update_interval_hours = 24
    archive_url = https://tldr.infra.local/assets/tldr.zip

