#!/bin/sh
# Checks a macOS bundle produced by `npx tauri build`: signature, Info.plist keys,
# architecture and minimum macOS version of the binary, localized strings and the disk
# image. Read-only: it never changes, signs or launches anything.
#
# Usage: scripts/macos/check-bundle.sh [path/to/PC Diagnostics.app]
# Default: target/release/bundle/macos/PC Diagnostics.app (run from the repository root).
set -eu

app=${1:-"target/release/bundle/macos/PC Diagnostics.app"}
plist="$app/Contents/Info.plist"
fail=0

ok() { printf 'ok    %s\n' "$1"; }
bad() { printf 'FAIL  %s\n' "$1"; fail=1; }

[ -d "$app" ] || { echo "no bundle at $app (run npx tauri build first)"; exit 2; }

key() { /usr/libexec/PlistBuddy -c "Print :$1" "$plist" 2>/dev/null || true; }

exe=$(key CFBundleExecutable)
bin="$app/Contents/MacOS/$exe"
[ -n "$exe" ] && [ -x "$bin" ] && ok "executable $exe" || bad "CFBundleExecutable missing or not executable"

[ "$(key CFBundleIdentifier)" = "com.pcdiagnostics.desktop" ] && ok "identifier com.pcdiagnostics.desktop" \
  || bad "CFBundleIdentifier is '$(key CFBundleIdentifier)'"

min=$(key LSMinimumSystemVersion)
[ -n "$min" ] && ok "LSMinimumSystemVersion $min" || bad "LSMinimumSystemVersion missing"

[ -n "$(key NSDocumentsFolderUsageDescription)" ] && ok "NSDocumentsFolderUsageDescription set" \
  || bad "NSDocumentsFolderUsageDescription missing (report export asks for Documents access)"

for lang in en it; do
  f="$app/Contents/Resources/$lang.lproj/InfoPlist.strings"
  if [ -f "$f" ] && plutil -lint -s "$f"; then ok "$lang.lproj/InfoPlist.strings"; else bad "$lang.lproj/InfoPlist.strings missing or invalid"; fi
done

[ -f "$app/Contents/Resources/$(key CFBundleIconFile)" ] || [ -f "$app/Contents/Resources/$(key CFBundleIconFile).icns" ] \
  && ok "icon $(key CFBundleIconFile)" || bad "icon file missing"

archs=$(lipo -archs "$bin" 2>/dev/null || true)
[ -n "$archs" ] && ok "architectures: $archs" || bad "lipo could not read $bin"

# LC_BUILD_VERSION minos of the binary must not be newer than the declared minimum.
minos=$(otool -l "$bin" | awk '/LC_BUILD_VERSION/{f=1} f&&/minos/{print $2; exit}')
if [ -n "$minos" ] && [ -n "$min" ]; then
  newest=$(printf '%s\n%s\n' "$minos" "$min" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)
  [ "$newest" = "$min" ] && ok "binary minos $minos <= $min" || bad "binary minos $minos is newer than LSMinimumSystemVersion $min"
else
  bad "could not read minos"
fi

if codesign --verify --deep --strict "$app" 2>/dev/null; then ok "codesign --verify --deep --strict"; else bad "signature does not verify"; fi
info=$(codesign -dv --verbose=2 "$app" 2>&1 || true)
echo "$info" | grep -q 'Signature=adhoc' && ok "ad-hoc signature" || echo "note  not ad-hoc: $(echo "$info" | grep -E '^Authority=' | head -1)"
echo "$info" | grep -q 'flags=.*runtime' && ok "hardened runtime" || bad "hardened runtime flag missing"

dmg_dir="$(dirname "$(dirname "$app")")/dmg"
for dmg in "$dmg_dir"/*.dmg; do
  [ -f "$dmg" ] || continue
  if hdiutil verify -quiet "$dmg"; then ok "disk image verifies: $(basename "$dmg")"; else bad "hdiutil verify failed: $dmg"; fi
done

printf 'sizes: app %s, binary %s bytes\n' "$(du -sh "$app" | cut -f1)" "$(stat -f %z "$bin")"
exit $fail
