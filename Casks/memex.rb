cask "memex" do
  version "0.2.1"

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

  binary "#{appdir}/Memex.app/Contents/MacOS/memex"
  binary "#{appdir}/Memex.app/Contents/MacOS/memex-daemon"

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
