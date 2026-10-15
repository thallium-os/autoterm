# autoterm

> Local, offline typo correction for fish and bash: `sduo apt update` → `sudo apt update`, safely.

## What it does
- Corrects unknown commands using edit distance with transpositions (`gti` → `git`, `sl` → `ls`).
- Read-only commands (`ls`, `cd`, `grep`, `cat`, …) are corrected silently. Everything else, including `sudo`, `rm`, `dd` and any command it doesn't know, waits for Enter.
- Learns from you. A suggestion you reject twice is never offered again for that typo. If you reject it and then run something else that works, that becomes the preferred fix after it happens twice.
- Every accept, reject and override goes into an append-only audit log protected by a SHA-256 checksum. Suggestions you never answered aren't logged, so a password typed at the prompt by mistake doesn't end up on disk.

## Quick Start
```sh
./install.sh   # cargo install, fish hook, sources bash hook from ~/.bashrc
exec fish      # or: exec bash
sduo ls        # autoterm: run `sudo ls`? [Y/n]
```

## Usage
| | |
|---|---|
| Mistyped command | Handled automatically by `fish_command_not_found` / bash `command_not_found_handle` |
| `Alt+/` | Fix the first word of the command line in place before running it |
| `set -gx AUTOTERM_DRY_RUN 1` | Show corrections, never run them |
| `autoterm log 50` | Last 50 audit entries |
| `autoterm verify` | Check the log against its checksum |
| `autoterm reset` | Delete the log and start learning over (the fix when the checksum fails) |

## Configuration
Data lives in `$XDG_DATA_HOME/autoterm/` (`log.tsv`, `log.tsv.sha256`). To change which commands are corrected silently, edit `SILENT` in `src/safety.rs`. Anything not on that list always asks first.

## Architecture
There is no database. Each lookup scans `$PATH` plus shell builtins and functions (about 3,000 names) and ranks them by distance and personal score. One call takes about 4ms, including process start. Personal scores are recomputed from the log on each call.

If the log fails its checksum, autoterm ignores personal data and refuses to append, so an edited log can't change suggestions. An interrupted write leaves it in the same state; `autoterm reset` clears it. This detects tampering by naive edits. It does not stop a process running as your user, which can rewrite both files.

Known limits: in bash the handler runs in a subshell, so builtins and functions (`cdd` → `cd`) are suggested, not run; use Alt+/. typos after `sudo` (`sudo aptt`) aren't seen, because `sudo` itself exists. Redirections on the mistyped line are applied by fish before the handler runs.

## License
MIT, see [LICENSE](LICENSE).
