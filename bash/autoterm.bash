# autoterm: typo correction for unknown commands (bash). Source from ~/.bashrc.
# Read-only commands are fixed silently; everything else asks first.
# AUTOTERM_DRY_RUN=1 prints the correction without running it.
[[ $- == *i* ]] || return 0
[[ -z ${__autoterm_pending_file-} ]] || return 0  # already loaded
command -v autoterm >/dev/null || return 0

# bash runs command_not_found_handle in a subshell, so state that must reach
# the next prompt goes through a file. $$ is still the interactive shell's pid.
__autoterm_pending_file="${XDG_RUNTIME_DIR:-/tmp}/autoterm-pending.$$"

# Keep any existing handler (e.g. distro "install package X" hints).
if declare -F command_not_found_handle >/dev/null && ! declare -F __autoterm_prev_cnf >/dev/null; then
    eval "$(declare -f command_not_found_handle | sed '1s/command_not_found_handle/__autoterm_prev_cnf/')"
fi

__autoterm_suggest() {
    compgen -b -a -A function | autoterm suggest --stdin "$1"
}

command_not_found_handle() {
    local typo=$1 out mode fix key shown
    shift
    if ! out=$(__autoterm_suggest "$typo"); then
        if declare -F __autoterm_prev_cnf >/dev/null; then
            __autoterm_prev_cnf "$typo" "$@"
        else
            printf 'bash: %s: command not found\n' "$typo" >&2
        fi
        return 127
    fi
    mode=${out%%$'\t'*}
    fix=${out#*$'\t'}
    shown=$(printf '%q ' "$fix" "$@")
    shown=${shown% }

    if [[ -n ${AUTOTERM_DRY_RUN-} ]]; then
        echo "autoterm: would run: $shown" >&2
        return 127
    fi
    # Running these here would only affect the subshell (cd, export, ...).
    case $(type -t "$fix") in
        builtin | function | alias)
            echo "autoterm: did you mean \`$shown\`? (Up, then Alt+/ fixes the line)" >&2
            return 127
            ;;
    esac

    echo "$typo" >"$__autoterm_pending_file"
    if [[ $mode == confirm ]]; then
        read -r -n 1 -p "autoterm: run \`$shown\`? [Y/n] " key </dev/tty || key=n
        [[ -n $key ]] && echo >&2
        if [[ -n $key && $key != [yY] ]]; then
            autoterm record reject "$typo" "$fix"
            return 127
        fi
    else
        echo "autoterm: $typo -> $fix" >&2
    fi
    autoterm record accept "$typo" "$fix"
    rm -f "$__autoterm_pending_file"
    "$fix" "$@"
}

# After a rejected suggestion, the next command that succeeds and looks like
# a fix for the typo is recorded as the user's preferred correction.
__autoterm_postexec() {
    local st=$? typo word
    [[ -f $__autoterm_pending_file ]] || return "$st"
    typo=$(<"$__autoterm_pending_file")
    read -r _ word _ <<<"$(HISTTIMEFORMAT= history 1)"
    [[ $word == "$typo" ]] && return "$st"
    [[ $st == 0 && -n $word ]] && autoterm record override "$typo" "$word"
    rm -f "$__autoterm_pending_file"
    return "$st"
}
PROMPT_COMMAND="__autoterm_postexec${PROMPT_COMMAND:+;$PROMPT_COMMAND}"

# Alt+/ fixes the first word of the command line in place; you review it,
# Enter runs it.
__autoterm_fix_commandline() {
    [[ $READLINE_LINE =~ ^([[:space:]]*)([^[:space:]]+)(.*)$ ]] || return
    local pre=${BASH_REMATCH[1]} word=${BASH_REMATCH[2]} post=${BASH_REMATCH[3]} out
    out=$(__autoterm_suggest "$word") || return
    READLINE_LINE="$pre${out#*$'\t'}$post"
    READLINE_POINT=${#READLINE_LINE}
}
bind -x '"\e/": __autoterm_fix_commandline'
