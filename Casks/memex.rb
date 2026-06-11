cask "memex" do
  version "0.2.4"

  # Apple Silicon
  on_arm do
    sha256 "REPLACE_WITH_SHA256_OF_aarch64_DMG"

    url "https://github.com/shetengteng/memex/releases/download/v#{version}/Memex_#{version}_aarch64.dmg",
        verified: "github.com/shetengteng/memex/"
  end

  # Intel
  on_intel do
    sha256 "REPLACE_WITH_SHA256_OF_x64_DMG"

    url "https://github.com/shetengteng/memex/releases/download/v#{version}/Memex_#{version}_x64.dmg",
        verified: "github.com/shetengteng/memex/"
  end

  name "Memex"
  desc "Local AI session memory across 7 IDEs (Claude Code · Cursor · Codex · OpenCode · ...)"
  homepage "https://github.com/shetengteng/memex"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :catalina"

  app "Memex.app"

  # bundle 内 CLI 二进制叫 `memex-cli`（避免与主 GUI binary `Memex` 在 APFS
  # 大小写不敏感文件系统上撞名）；通过 `target: "memex"` 把命令仍然暴露成 `memex`。
  # Phase 4 起 `memex-daemon` 不再有独立 binary —— daemon 折叠成 Tauri 主进程
  # 内的 in-process task，所以 cask 里不再 `binary` 那个文件。
  binary "#{appdir}/Memex.app/Contents/MacOS/memex-cli", target: "memex"

  # 移除 macOS quarantine 属性，避免用户首次启动被 Gatekeeper 拦截
  # 由于使用 ad-hoc 签名（无 Apple Developer Program），必须显式 xattr -cr
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/Memex.app"],
                   sudo: false
  end

  zap trash: [
    "~/.memex",
    "~/Library/Application Support/Memex",
    "~/Library/Caches/Memex",
    "~/Library/Logs/Memex",
    "~/Library/Preferences/ttshe.memex.plist",
  ]
end
