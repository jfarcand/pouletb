#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Build Pouletb.app and make it the default handler for .torrent files and
# magnet links: opening one starts `pouletb` in a new Terminal window.
#
#   macos/install.sh               install into ~/Applications
#   macos/install.sh --uninstall   remove it (the previous handler takes over again)
#
# POULETB_APP_DIR overrides the install directory.
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
DEST="${POULETB_APP_DIR:-$HOME/Applications}"
APP="$DEST/Pouletb.app"
LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

if [[ "${1:-}" == "--uninstall" ]]; then
  if [[ -d "$APP" ]]; then
    "$LSREGISTER" -u "$APP"
    rm -rf "$APP"
    echo "Removed $APP"
  else
    echo "Nothing to remove at $APP"
  fi
  exit 0
fi

if ! command -v pouletb >/dev/null && [[ ! -x "$HOME/.cargo/bin/pouletb" ]]; then
  echo "warning: pouletb is not installed yet; run 'cargo install pouletb' before opening a torrent" >&2
fi

mkdir -p "$DEST"
rm -rf "$APP"
osacompile -o "$APP" "$HERE/Pouletb.applescript"

plist="$APP/Contents/Info.plist"
set_key() { plutil -replace "$1" -json "$2" "$plist"; }

set_key CFBundleIdentifier '"io.github.jfarcand.pouletb"'
set_key CFBundleName '"Pouletb"'
set_key NSAppleEventsUsageDescription '"Pouletb opens a Terminal window to run the download."'
# macOS declares no type for .torrent, so declare the conventional one.
set_key UTImportedTypeDeclarations '[{
  "UTTypeIdentifier": "org.bittorrent.torrent",
  "UTTypeDescription": "BitTorrent file",
  "UTTypeConformsTo": ["public.data"],
  "UTTypeTagSpecification": {
    "public.filename-extension": ["torrent"],
    "public.mime-type": ["application/x-bittorrent"]
  }
}]'
set_key CFBundleDocumentTypes '[{
  "CFBundleTypeName": "BitTorrent file",
  "CFBundleTypeRole": "Viewer",
  "LSHandlerRank": "Owner",
  "LSItemContentTypes": ["org.bittorrent.torrent"]
}]'
set_key CFBundleURLTypes '[{
  "CFBundleURLName": "Magnet link",
  "CFBundleURLSchemes": ["magnet"]
}]'

# Editing Info.plist invalidates osacompile's signature; re-sign ad hoc.
codesign --force --deep --sign - "$APP" 2>&1 | { grep -v "replacing existing signature" || true; }
"$LSREGISTER" -f "$APP"
echo "Installed $APP"

swift "$HERE/set-default-handler.swift" "$APP"

echo
echo "Double-click a .torrent file or click a magnet link to start a download."
echo "The first time, macOS asks whether Pouletb may control Terminal: allow it."
