#compdef tldr

_applications() {
    local -a commands
    if commands=(${(uonzf)"$(tldr --list 2>/dev/null)"//:/\\:}); then
        _describe -t commands 'command' commands
    fi
}

_tealdeer() {
    local I="-h --help -v --version"
    integer ret=1
    local -a args

    args+=(
        "($I -l --list)"{-l,--list}"[List all commands in the cache]"
        "($I -f --render)"{-f,--render}"[Render a specific markdown file]:file:_files"
        "($I -p --platform)"{-p,--platform}'[Override the operating system]:platform:((
            linux
            macos
            sunos
            windows
            android
            freebsd
            netbsd
            openbsd
        ))'
        "($I -L --language)"{-L,--language}"[Override the language settings]:lang"
        "($I -u --update)"{-u,--update}"[Update the local cache]"
        "($I)--no-auto-update[If auto update is configured, disable it for this run]"
        "($I -c --clear-cache)"{-c,--clear-cache}"[Clear the local cache]"
        "($I)--pager[Use a pager to page output]"
        "($I -r --raw)"{-r,--raw}"[Display the raw markdown instead of rendering it]"
        "($I -q --quiet)"{-q,--quiet}"[Suppress informational messages]"
        "($I)--show-paths[Show file and directory paths used by tealdeer]"
        "($I)--seed-config[Create a basic config]"
        "($I)--color[Controls when to use color]:when:((
            always
            auto
            never
        ))"
        '(- *)'{-h,--help}'[Display help]'
        '(- *)'{-v,--version}'[Show version information]'
        '1: :_applications'
    )

    _arguments $args[@] && ret=0
    return ret
}

_tealdeer
