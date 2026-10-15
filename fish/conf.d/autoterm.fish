# autoterm: typo correction for unknown commands.
# Read-only commands are fixed silently; everything else asks first.
# AUTOTERM_DRY_RUN=1 prints the correction without running it.
status is-interactive; or exit
command -q autoterm; or exit

# Keep whatever handler was there (distro "install package X" hints etc.).
if functions -q fish_command_not_found; and not functions -q __autoterm_prev_cnf
    functions -c fish_command_not_found __autoterm_prev_cnf
end

# Not abbreviations: fish expands those only while typing, so `$fix` could
# never run one and the "accept" would teach a fix that fails.
function __autoterm_suggest -a word
    begin
        builtin -n
        functions -n
    end 2>/dev/null | autoterm suggest --stdin $word
end

function fish_command_not_found
    set -l typo $argv[1]
    set -l rest
    set -q argv[2]; and set rest $argv[2..-1]
    set -l out (__autoterm_suggest $typo | string split \t)
    if test (count $out) -ne 2
        if functions -q __autoterm_prev_cnf
            __autoterm_prev_cnf $argv
        else
            printf 'fish: Unknown command: %s\n' (string escape -- $typo) >&2
        end
        return 127
    end
    set -l mode $out[1]
    set -l fix $out[2]
    set -l shown (string join ' ' -- (string escape -- $fix $rest))

    if set -q AUTOTERM_DRY_RUN
        echo "autoterm: would run: $shown" >&2
        return 127
    end

    set -g __autoterm_pending $typo
    if test "$mode" = confirm
        set -l key
        if not read -n 1 -P "autoterm: run `$shown`? [Y/n] " key </dev/tty; or not contains -- "$key" '' y Y
            autoterm record reject $typo $fix
            return 127
        end
    else
        echo "autoterm: $typo -> $fix" >&2
    end
    autoterm record accept $typo $fix
    set -e __autoterm_pending
    $fix $rest
end

# After a rejected suggestion, the next command that succeeds and looks like
# a fix for the typo is recorded as the user's preferred correction.
function __autoterm_postexec --on-event fish_postexec
    set -l st $status
    set -q __autoterm_pending; or return
    set -l word (string split -f1 ' ' -- (string trim -- $argv[1]))
    test "$word" = "$__autoterm_pending"; and return
    test $st -eq 0; and test -n "$word"; and autoterm record override $__autoterm_pending $word
    set -e __autoterm_pending
end

# Alt+/ fixes the first word of the command line in place; you review it,
# Enter runs it.
function __autoterm_fix_commandline
    set -l m (string match -r '^(\s*)(\S+)(.*)$' -- (commandline | string collect))
    or return
    set -l out (__autoterm_suggest $m[3] | string split \t)
    test (count $out) -eq 2; or return
    commandline -r -- "$m[2]$out[2]$m[4]"
end
bind \e/ __autoterm_fix_commandline
