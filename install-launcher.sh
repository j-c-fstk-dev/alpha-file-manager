#!/bin/sh
set -eu

cd "$(dirname "$0")"
app_id=dev.jcdev.AlphaFileManager
data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
bin="$HOME/.local/bin/alpha-file-manager"
desktop="$data_dir/applications/$app_id.desktop"
previous=
if command -v xdg-mime >/dev/null 2>&1; then
    previous=$(xdg-mime query default inode/directory || true)
fi

if [ -x ./alpha-file-manager ]; then
    executable=./alpha-file-manager
else
    cargo build --release --locked
    executable=target/release/alpha-file-manager
fi
install -Dm755 "$executable" "$bin"
install -Dm644 assets/alpha-file-manager.svg "$data_dir/icons/hicolor/scalable/apps/$app_id.svg"
mkdir -p "$(dirname "$desktop")"
# The desktop-entry parser unescapes string values before parsing Exec.
escaped_bin=$(printf '%s' "$bin" | sed 's/[\\`"$]/\\\\&/g; s/%/%%/g')
temporary=$(mktemp "$desktop.XXXXXX.desktop")
trap 'rm -f "$temporary"' EXIT
cat > "$temporary" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Alpha File Manager
GenericName=File Manager
GenericName[pt_BR]=Gerenciador de arquivos
Exec="$escaped_bin" %f
Icon=$app_id
Terminal=false
Categories=System;FileTools;FileManager;
MimeType=inode/directory;
StartupNotify=true
DESKTOP
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$temporary"
fi
chmod 644 "$temporary"
mv -f "$temporary" "$desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$(dirname "$desktop")"
fi
if [ -n "$previous" ] && [ "$previous" != "$app_id.desktop" ] &&
   [ "$(xdg-mime query default inode/directory)" = "$app_id.desktop" ]; then
    mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}"
    xdg-mime default "$previous" inode/directory
fi

# Make Alpha available in the Xfce preferred-applications chooser as well.
helper="$data_dir/xfce4/helpers/$app_id.desktop"
mkdir -p "$(dirname "$helper")"
cat > "$helper" <<HELPER
[Desktop Entry]
Type=X-XFCE-Helper
Name=Alpha File Manager
Icon=$app_id
X-XFCE-Category=FileManager
X-XFCE-Commands="$escaped_bin";
X-XFCE-CommandsWithParameter="$escaped_bin" "%s";
HELPER
printf '%s\n' "$desktop"
