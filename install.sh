#!/bin/sh
# Installs the autoterm binary plus its fish and bash hooks. Never
# overwrites a modified fish hook silently: the old file is kept as .bak.
set -eu

cd "$(dirname "$0")"
cargo install --path . --locked

dest="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d"
mkdir -p "$dest"
if [ -f "$dest/autoterm.fish" ] && ! cmp -s fish/conf.d/autoterm.fish "$dest/autoterm.fish"; then
    cp "$dest/autoterm.fish" "$dest/autoterm.fish.bak"
    echo "backed up existing hook to $dest/autoterm.fish.bak"
fi
cp fish/conf.d/autoterm.fish "$dest/autoterm.fish"
data="${XDG_DATA_HOME:-$HOME/.local/share}/autoterm"
mkdir -p "$data"
cp bash/autoterm.bash "$data/autoterm.bash"
line="[ -f \"$data/autoterm.bash\" ] && . \"$data/autoterm.bash\""
if ! grep -qF "$data/autoterm.bash" "$HOME/.bashrc" 2>/dev/null; then
    printf '\n# autoterm typo correction\n%s\n' "$line" >>"$HOME/.bashrc"
    echo "added autoterm to ~/.bashrc"
fi
echo "installed; open a new fish or bash shell"
