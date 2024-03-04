# tealdeer bash completion

_tealdeer()
{
	local cur prev words cword
	_init_completion || return

	case $prev in
		-h|--help|-v|--version|-l|--list|-u|--update|--no-auto-update|-c|--clear-cache|--pager|-r|--raw|--show-paths|--seed-config|-q|--quiet)
			return
			;;
		-f|--render)
			_filedir
			return
			;;
		-p|--platform)
			COMPREPLY=( $(compgen -W 'linux macos sunos windows android freebsd netbsd openbsd' -- "${cur}") )
			return
			;;
		--color)
			COMPREPLY=( $(compgen -W 'always auto never' -- "${cur}") )
			return
			;;
	esac

	if [[ $cur == -* ]]; then
		COMPREPLY=( $( compgen -W '$( _parse_help "$1" )' -- "$cur" ) )
		return
	fi
	if tldrlist=$(tldr -l 2>/dev/null); then
		COMPREPLY=( $(compgen -W '$( echo "$tldrlist" | tr -d , )' -- "${cur}") )
	fi
}

complete -F _tealdeer tldr
