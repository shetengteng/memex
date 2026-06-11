#!/usr/bin/env bash
# Memex macOS 安装/修复脚本
# 用法：
#   curl -fsSL https://raw.githubusercontent.com/shetengteng/memex/main/scripts/install-macos.sh | bash
#   或下载后 chmod +x install-macos.sh && ./install-macos.sh

set -e

readonly APP_PATH="/Applications/Memex.app"
readonly BUNDLE_ID="ttshe.memex"

color() {
    case "$1" in
        green)  printf '\033[32m%s\033[0m' "$2" ;;
        yellow) printf '\033[33m%s\033[0m' "$2" ;;
        red)    printf '\033[31m%s\033[0m' "$2" ;;
        blue)   printf '\033[34m%s\033[0m' "$2" ;;
        *) printf '%s' "$2" ;;
    esac
}

log_info()    { echo "$(color blue '[INFO]') $1"; }
log_ok()      { echo "$(color green '[OK]') $1"; }
log_warn()    { echo "$(color yellow '[WARN]') $1"; }
log_err()     { echo "$(color red '[ERR]') $1"; }

# 检查是否 macOS
if [[ "$(uname)" != "Darwin" ]]; then
    log_err "本脚本仅支持 macOS"
    exit 1
fi

# 检查 Memex.app 是否存在
if [[ ! -d "$APP_PATH" ]]; then
    log_err "未找到 $APP_PATH"
    log_info "请先把下载的 Memex.app 拖入 /Applications/，然后再运行本脚本"
    log_info "下载地址：https://github.com/shetengteng/memex/releases"
    exit 1
fi

# Step 1：终止可能运行中的旧版进程
# 新版 CFBundleExecutable = Memex（旧版 = memex-menubar），都要 cover
log_info "终止旧进程（如果有）"
pkill -9 -x Memex 2>/dev/null || true
pkill -9 -x memex-menubar 2>/dev/null || true
pkill -9 -x memex-daemon 2>/dev/null || true
rm -f "$HOME/.memex/daemon.lock" 2>/dev/null || true
sleep 1

# Step 2：清除 quarantine 扩展属性（关键步骤）
# macOS 默认会给从浏览器/AirDrop/U 盘等"外部来源"下载的文件打上 com.apple.quarantine 标签
# 导致 ad-hoc 签名的 App 启动时被 Gatekeeper 拦截或扔到 AppTranslocation 临时目录
# xattr -cr 递归清空所有扩展属性，让 App 像本地编译产物一样被信任
log_info "清除 quarantine 扩展属性"
if xattr -l "$APP_PATH" 2>/dev/null | grep -q "com.apple.quarantine"; then
    xattr -cr "$APP_PATH"
    log_ok "quarantine 已清除"
else
    log_ok "未发现 quarantine 标记"
fi

# Step 3：刷新 LaunchServices 数据库（避免老幽灵注册项）
log_info "刷新 LaunchServices 数据库"
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
if [[ -x "$LSREG" ]]; then
    "$LSREG" -f "$APP_PATH" >/dev/null 2>&1 && log_ok "LaunchServices 已刷新"
fi

# Step 4：验证签名
log_info "验证 App 签名"
if codesign -v "$APP_PATH" 2>/dev/null; then
    log_ok "签名验证通过（ad-hoc）"
else
    log_warn "签名验证未通过，App 可能仍能运行但 macOS 会提示开发者未识别"
fi

# Step 5：启动 App
log_info "启动 Memex"
open "$APP_PATH"
sleep 2

if pgrep -x Memex >/dev/null; then
    log_ok "Memex 已启动 ✓"
    echo ""
    echo "$(color green '安装完成！') 菜单栏右上角应该看到 Memex 图标，点击或按 ⌘⇧M 唤出 popup"
    echo "命令行工具位于 /Applications/Memex.app/Contents/MacOS/memex-cli"
    echo "建议在 ~/.zshrc 加 alias："
    echo "  alias memex='/Applications/Memex.app/Contents/MacOS/memex-cli'"
else
    log_err "Memex 未能启动，请手动 open $APP_PATH 查看错误"
    exit 1
fi
