#!/usr/bin/env bash
# update-cask-sha.sh — 给 Casks/memex.rb 自动填入两个架构 DMG 的 SHA256
#
# 触发场景：v0.X.X release 已经上传到 GitHub Releases（含 .dmg 文件），
# 此脚本下载两个 DMG、计算 sha256、用 sed 替换 cask 里的占位符或旧 hash。
#
# 用法:
#   bash scripts/update-cask-sha.sh                # 用当前 Casks/memex.rb 里的 version
#   bash scripts/update-cask-sha.sh v0.2.0         # 显式指定 tag
#
# 完成后:
#   git diff Casks/memex.rb        # review
#   git commit -am "chore: cask SHA256 for v0.2.0"
#   git push
#
# 如果你已经把 cask 移到独立 tap repo (shetengteng/homebrew-memex),
# 把更新后的 memex.rb 复制过去再 push.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CASK="$ROOT/Casks/memex.rb"
REPO="shetengteng/memex"

if [ ! -f "$CASK" ]; then
  echo "✗ $CASK not found" >&2
  exit 1
fi

TAG="${1:-}"
if [ -z "$TAG" ]; then
  VER=$(grep -m1 '^\s*version ' "$CASK" | sed -E 's/.*"([^"]+)".*/\1/')
  TAG="v$VER"
fi
VERSION="${TAG#v}"

echo "==> Target: tag=$TAG version=$VERSION"

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

ARM_URL="https://github.com/$REPO/releases/download/$TAG/Memex_${VERSION}_aarch64.dmg"
X64_URL="https://github.com/$REPO/releases/download/$TAG/Memex_${VERSION}_x64.dmg"

echo "==> Downloading ARM DMG"
curl -fSL -o "$TMPDIR/arm.dmg" "$ARM_URL"

echo "==> Downloading x86_64 DMG"
curl -fSL -o "$TMPDIR/x64.dmg" "$X64_URL"

ARM_SHA=$(shasum -a 256 "$TMPDIR/arm.dmg" | cut -d' ' -f1)
X64_SHA=$(shasum -a 256 "$TMPDIR/x64.dmg" | cut -d' ' -f1)

echo "   ARM SHA256: $ARM_SHA"
echo "   x64 SHA256: $X64_SHA"

# Cask 里 sha256 在 on_arm / on_intel 各一处。用 awk 分块替换以避免误伤。
python3 - <<PY
import re, pathlib
p = pathlib.Path("$CASK")
src = p.read_text()

def replace_in_block(src, opener, sha):
    pat = re.compile(rf'({opener}\s*do\b.*?)sha256\s+"[^"]*"', re.DOTALL)
    return pat.sub(rf'\1sha256 "{sha}"', src, count=1)

src = replace_in_block(src, "on_arm", "$ARM_SHA")
src = replace_in_block(src, "on_intel", "$X64_SHA")
p.write_text(src)
print("[ok] patched", p)
PY

echo ""
echo "==> Done. Review:"
echo "    git diff Casks/memex.rb"
