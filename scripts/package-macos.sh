#!/bin/sh
# Build an unsigned SciPlot.app and a tar.gz next to it.
# Usage: scripts/package-macos.sh <binary> <output-dir> [arch-label]
# SCIPLOT_VERSION overrides the version written into Info.plist (default: workspace version).
# SCIPLOT_BUILD is the build number used in the archive name (default: 0).
# This does not sign or notarize the app, and it does not create a GitHub Release.
set -eu

bin=${1:?binary}
out=${2:?output directory}
label=${3:-macos-aarch64}

if [ -z "${SCIPLOT_VERSION:-}" ]; then
  root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
  SCIPLOT_VERSION=$(awk '/^\[workspace.package\]/{flag=1; next} /^\[/{flag=0} flag && /^version = /{gsub(/"/, "", $3); print $3; exit}' "$root/Cargo.toml")
fi
version=${SCIPLOT_VERSION:?version}
build=${SCIPLOT_BUILD:-0}

rm -rf "$out/SciPlot.app"
mkdir -p "$out/SciPlot.app/Contents/MacOS" "$out/SciPlot.app/Contents/Resources"
cp "$bin" "$out/SciPlot.app/Contents/MacOS/sciplot"
chmod +x "$out/SciPlot.app/Contents/MacOS/sciplot"

cat > "$out/SciPlot.app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>SciPlot</string>
  <key>CFBundleDisplayName</key>
  <string>SciPlot</string>
  <key>CFBundleIdentifier</key>
  <string>dev.sciplot.app</string>
  <key>CFBundleExecutable</key>
  <string>sciplot</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}.${build}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

archive="$out/sciplot-${version}-b${build}-${label}.tar.gz"
rm -f "$archive"
tar -C "$out" -czf "$archive" SciPlot.app
