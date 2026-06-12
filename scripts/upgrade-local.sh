#!/usr/bin/env bash
# upgrade-local.sh — 在本地一键升级 /Applications/Memex.app 到最新 build
#
# 步骤：
#   1. 校验版本（package.json / tauri.conf.json / Cargo.toml 三处一致）
#   2. 停掉所有 Memex 进程
#   3. 备份 ~/.memex（保留数据；用户数据不会丢）
#   4. 完整重 build .app bundle（tauri build --bundles app）
#   5. 删除 /Applications/Memex.app 与 target 旧 bundle，再部署新 bundle
#   6. xattr 清除 quarantine + lsregister 刷新
#   7. open 启动新 app
#   8. 输出版本号与运行 PID
#
# 数据安全：
#   ~/.memex 不会被脚本修改/删除；旧版本与新版本 schema 兼容（schema migration 框架已就位）。
#   备份目录 ~/.memex.bak.<unix_ts>/ 仅作保险；如确认升级 OK 可手动删除。
#
# Usage: bash scripts/upgrade-local.sh [--skip-backup] [--skip-build]

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_PATH="/Applications/Memex.app"
# Cargo workspace lives in app/ now, so build output is rooted at app/target/.
BUNDLE_PATH="$ROOT/app/target/release/bundle/macos/Memex.app"
MEMEX_DIR="$HOME/.memex"

SKIP_BACKUP=false
SKIP_BUILD=false
for arg in "$@"; do
  case "$arg" in
    --skip-backup) SKIP_BACKUP=true ;;
    --skip-build)  SKIP_BUILD=true ;;
    -h|--help)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown arg: $arg" >&2
      exit 2
      ;;
  esac
done

cd "$ROOT"

echo "==> 1. 校验版本一致性"
PKG_VER=$(node -p "require('./app/desktop/package.json').version")
CONF_VER=$(node -p "require('./app/desktop/src-tauri/tauri.conf.json').version")
CARGO_VER=$(grep -m1 '^version = ' app/Cargo.toml | sed -E 's/version = "([^"]+)"/\1/')

echo "   package.json     = $PKG_VER"
echo "   tauri.conf.json  = $CONF_VER"
echo "   Cargo.toml       = $CARGO_VER"

if [ "$PKG_VER" != "$CONF_VER" ] || [ "$PKG_VER" != "$CARGO_VER" ]; then
  echo "   ✗ 版本不一致，请先运行: node scripts/sync-version.js" >&2
  exit 1
fi
echo "   ✓ 版本一致: v$PKG_VER"
echo

echo "==> 2. 停止运行中的 memex 进程"
# 用 `-x` 精确匹配 progname，避免误杀 Cursor / VSCode 等 cmdline 里出现
# "Memex" 字串的进程。
pkill -9 -x Memex 2>/dev/null || true
sleep 1
REMAINING=$(pgrep -x Memex 2>/dev/null | wc -l | tr -d ' \n' || echo 0)
echo "   Memex 进程: $REMAINING"
echo

echo "==> 3. 备份用户数据（~/.memex/）"
if [ "$SKIP_BACKUP" = "true" ]; then
  echo "   --skip-backup 已指定，跳过"
elif [ -d "$MEMEX_DIR" ]; then
  TS=$(date +%s)
  BAK="$HOME/.memex.bak.$TS"
  cp -R "$MEMEX_DIR" "$BAK"
  DB_SIZE=$(du -sh "$MEMEX_DIR" 2>/dev/null | cut -f1)
  echo "   ✓ 已备份到 $BAK (size=$DB_SIZE)"
else
  echo "   (~/.memex 不存在，无需备份)"
fi
echo

echo "==> 4. 构建新 .app bundle"
if [ "$SKIP_BUILD" = "true" ]; then
  echo "   --skip-build 已指定，使用 $BUNDLE_PATH"
  if [ ! -d "$BUNDLE_PATH" ]; then
    echo "   ✗ $BUNDLE_PATH 不存在；不能跳过 build" >&2
    exit 1
  fi
else
  rm -rf "$BUNDLE_PATH"
  # tauri build 可能在生成 updater signature 时报错（缺 TAURI_SIGNING_PRIVATE_KEY），
  # 但 .app 已经先 bundle 完成。我们只关心 .app 是否生成 → 忽略 exit code，看产物。
  set +e
  (cd app/desktop && npm run tauri:bundle:app) 2>&1 | tail -20
  set -e
  if [ ! -d "$BUNDLE_PATH" ]; then
    echo "   ✗ build 后仍找不到 $BUNDLE_PATH" >&2
    exit 1
  fi
fi
BUNDLE_VER=$(plutil -extract CFBundleShortVersionString raw "$BUNDLE_PATH/Contents/Info.plist")
echo "   ✓ bundle version = $BUNDLE_VER"
echo

echo "==> 5. 替换 /Applications/Memex.app"
if [ -d "$APP_PATH" ]; then
  OLD_VER=$(plutil -extract CFBundleShortVersionString raw "$APP_PATH/Contents/Info.plist" 2>/dev/null || echo "?")
  echo "   旧版本: v$OLD_VER → 新版本: v$BUNDLE_VER"
  /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -u "$APP_PATH" 2>/dev/null || true
  rm -rf "$APP_PATH"
fi
cp -R "$BUNDLE_PATH" "$APP_PATH"
echo "   ✓ 已部署"
echo

echo "==> 6. 清除 quarantine + 重新注册 LaunchServices"
xattr -cr "$APP_PATH"
/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f "$APP_PATH"
echo "   ✓ 已注册"
echo

echo "==> 7. 启动新版"
open "$APP_PATH"
sleep 2

NEW_PID=$(pgrep -x Memex 2>/dev/null | head -1 || true)

echo "==> 8. 完成"
echo "   Memex PID:  ${NEW_PID:-N/A}"
echo "   版本:       v$BUNDLE_VER"
echo "   数据目录:   $MEMEX_DIR (保留)"
echo ""
echo "在屏幕右上角菜单栏点击 Memex (M) 图标验证 popup 是否正常弹出。"
