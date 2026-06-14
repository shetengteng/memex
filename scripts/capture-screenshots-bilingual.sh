#!/usr/bin/env bash
# capture-screenshots-bilingual.sh — 给 docs/screenshots/ 刷出 10 张图 (5 页 × zh+en).
#
# 流程:
#   1. 假定 ~/.memex-demo 已经 bootstrap 过, 并通过 seed-demo.py 灌入 mock 数据
#   2. 对 lang ∈ {zh, en}:
#        a) pkill demo memex + 删 daemon.lock
#        b) sqlite UPDATE kv SET value=<lang> WHERE key='ui.locale'
#        c) USER=User + MEMEX_HOME=~/.memex-demo 启动 demo memex (隔离真实数据)
#        d) 调 scripts/capture-screenshots.sh --keep-app 截 5 张 0X-<page>.png
#        e) 把无后缀文件 rename 为 -zh.png / -en.png
#   3. 最后 pkill demo memex
#
# Usage: bash scripts/capture-screenshots-bilingual.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DB="$HOME/.memex-demo/memex.db"
APP_BIN="/Applications/Memex.app/Contents/MacOS/Memex"

if [ ! -f "$DB" ]; then
  echo "✗ $DB 不存在, 先 bootstrap demo memex (USER=User MEMEX_HOME=~/.memex-demo 启动一次) 再 seed-demo.py" >&2
  exit 1
fi

if [ ! -x "$APP_BIN" ]; then
  echo "✗ $APP_BIN 不存在或不可执行" >&2
  exit 1
fi

stop_demo() {
  pkill -f "MacOS/Memex" 2>/dev/null || true
  sleep 1
  rm -f "$HOME/.memex-demo/daemon.lock"
}

for lang in zh en; do
  echo
  echo "================ pass: $lang ================"
  stop_demo

  echo "==> set ui.locale = $lang"
  sqlite3 "$DB" "UPDATE kv SET value='$lang' WHERE key='ui.locale'"

  echo "==> launch demo Memex (USER=User, MEMEX_HOME=$HOME/.memex-demo)"
  USER="User" MEMEX_HOME="$HOME/.memex-demo" \
    nohup "$APP_BIN" >>/tmp/demo-memex.log 2>&1 &
  sleep 5

  bash "$ROOT/scripts/capture-screenshots.sh" --keep-app

  echo "==> rename → -${lang}.png"
  for f in "$ROOT/docs/screenshots"/0[1-5]-*.png; do
    case "$f" in *-zh.png|*-en.png) continue ;; esac
    base="${f%.png}"
    mv "$f" "${base}-${lang}.png"
  done
done

stop_demo

echo
echo "================ done ================"
ls -lh "$ROOT/docs/screenshots"/0[1-5]-*-{zh,en}.png
