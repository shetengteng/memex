#!/usr/bin/env bash
# capture-screenshots.sh — refresh docs/screenshots/*.png from the running Memex.app.
#
# What it does:
#   1. Builds a tiny Swift helper that returns the CGWindow ID of the main
#      Memex window. (`screencapture -l <wid>` then captures by window ID
#      regardless of z-order, which is essential because the AI assistant
#      controlling this script is typically the foreground app.)
#   2. Activates Memex and resizes/repositions its window so every page
#      ships at a consistent 1440×900 framing.
#   3. Iterates over the 5 main routes via `memex://goto/<page>` deep links,
#      pauses long enough for the page to render, then captures.
#
# Output: docs/screenshots/{01-today,02-library,03-insights,04-connect,05-settings}.png
#
# Requirements:
#   - macOS (AppleScript + screencapture + swiftc are stock)
#   - Memex.app installed at /Applications/Memex.app (build via upgrade-local.sh)
#   - First run: grant Screen Recording + Accessibility permissions to the
#     terminal app (System Settings → Privacy & Security)
#
# Usage:
#   bash scripts/capture-screenshots.sh
#   bash scripts/capture-screenshots.sh --keep-app  # don't quit Memex at the end

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="/Applications/Memex.app"
OUT="$ROOT/docs/screenshots"
TMP="$(mktemp -d /tmp/memex-capture.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

KEEP_APP=false
for arg in "$@"; do
  case "$arg" in
    --keep-app) KEEP_APP=true ;;
    -h|--help) sed -n '2,28p' "$0"; exit 0 ;;
    *) echo "Unknown arg: $arg" >&2; exit 2 ;;
  esac
done

if [ ! -d "$APP" ]; then
  echo "✗ $APP not found. Run bash scripts/upgrade-local.sh first." >&2
  exit 1
fi

mkdir -p "$OUT"

# ---- 1. Build the window-ID helper ----------------------------------------

GET_WID_SRC="$TMP/get_wid.swift"
GET_WID_BIN="$TMP/get_wid"
cat > "$GET_WID_SRC" <<'SWIFT'
import Cocoa
import CoreGraphics

let options = CGWindowListOption(arrayLiteral: .excludeDesktopElements, .optionAll)
guard let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
  FileHandle.standardError.write("CGWindowListCopyWindowInfo failed\n".data(using: .utf8)!)
  exit(1)
}

var best: (wid: Int, area: Int)? = nil
for w in list {
  let owner = (w[kCGWindowOwnerName as String] as? String) ?? ""
  if owner != "Memex" { continue }
  let layer = (w[kCGWindowLayer as String] as? Int) ?? 0
  if layer != 0 { continue }
  let wid = (w[kCGWindowNumber as String] as? Int) ?? 0
  let bounds = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
  let width = (bounds["Width"] as? Int) ?? 0
  let height = (bounds["Height"] as? Int) ?? 0
  if width < 400 || height < 400 { continue }
  let area = width * height
  if best == nil || area > best!.area {
    best = (wid: wid, area: area)
  }
}

if let b = best {
  print(b.wid)
} else {
  FileHandle.standardError.write("no Memex main window\n".data(using: .utf8)!)
  exit(2)
}
SWIFT

swiftc -O "$GET_WID_SRC" -o "$GET_WID_BIN"

# ---- 2. Make sure Memex is running and positioned ------------------------

echo "==> launching/activating Memex"
osascript <<'APPLESCRIPT'
tell application "Memex" to activate
delay 1
tell application "System Events"
  tell process "Memex"
    set frontmost to true
    if (count of windows) > 0 then
      tell window 1
        set position to {120, 80}
        set size to {1440, 900}
      end tell
    end if
  end tell
end tell
APPLESCRIPT

sleep 1

WID="$("$GET_WID_BIN")"
echo "==> Memex window ID: $WID"

# ---- 3. Capture each page ------------------------------------------------

# (slug, deep_link_page, render_wait_seconds)
PAGES=(
  "01-today:today:2"
  "02-library:library:3"
  "03-insights:insights:3"
  "04-connect:connect:3"
  "05-settings:settings:3"
)

for entry in "${PAGES[@]}"; do
  slug="${entry%%:*}"
  rest="${entry#*:}"
  page="${rest%%:*}"
  wait_s="${rest##*:}"
  out="$OUT/${slug}.png"

  echo "==> [$slug] open memex://goto/$page"
  open "memex://goto/$page" || true
  sleep "$wait_s"

  # screencapture -l targets a specific window ID, so it works even when
  # the terminal/AI tool is foreground.
  screencapture -x -l "$WID" "$out"
  echo "    wrote $out ($(stat -f %z "$out") bytes)"
done

# ---- 4. Optional cleanup -------------------------------------------------

if [ "$KEEP_APP" = "false" ]; then
  echo "==> done; leaving Memex running (pass --keep-app to silence this msg)"
fi

echo
echo "✓ refreshed $(ls "$OUT"/*.png | wc -l | tr -d ' ') screenshot(s) → $OUT"
