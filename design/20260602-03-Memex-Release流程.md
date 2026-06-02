# Memex Release 流程

> 从代码改完到用户能 `brew install --cask memex` 之间，每一步要做什么、谁来做、哪些是阻塞项。

---

## 当前现状

| 环节 | 状态 | 说明 |
|---|---|---|
| 版本同步脚本 | ✓ | `scripts/sync-version.js`：bump + sync 4 处版本号 |
| 本地一键升级 | ✓ | `scripts/upgrade-local.sh`：build + 部署 + 启动 |
| GitHub Actions release | ✓（脚本就绪） | `.github/workflows/release.yml` —— 但**还没真跑过** |
| GitHub Releases 上的 DMG | ✗ | 还没有任何 v0.x.x release |
| Tauri Updater signing key | ⚠ | 本地有，但**还没作为 GitHub Secret 配置** |
| `Casks/memex.rb` | ⚠ 模板 | 在主 repo 内，SHA256 是占位符 `REPLACE_WITH_*` |
| `homebrew-memex` tap repo | ✗ | **尚未创建**（用户需要在 GitHub 手动建 repo） |
| Brew Cask 安装 | ✗ | 依赖以上 3 项，全部 OK 后才能用 |

---

## 完整 Release 一次跑通的步骤

### Phase 0：一次性准备（每个 GitHub 账号只做一次）

#### 0.1 GitHub repo Settings → Secrets and variables → Actions

把本地生成过的 Tauri updater signing key 复制进去：

```bash
# 本地查看 key
cat ~/.tauri/memex-updater.key
cat ~/.tauri/memex-updater.key.pub
```

在 GitHub 加两个 secret：

| Secret 名 | 内容 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/memex-updater.key` 的完整内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成 key 时没设密码就留空 |

#### 0.2 创建 Homebrew tap repo

到 GitHub 手动创建 `https://github.com/shetengteng/homebrew-memex`：

- repo name **必须**是 `homebrew-<tap-name>`（这里 tap-name=`memex`，所以 repo 名是 `homebrew-memex`）
- public visibility
- 初始化 README，license = MIT

#### 0.3 启用 GitHub Pages

到 `https://github.com/shetengteng/memex` Settings → Pages：

- Source: **GitHub Actions**（不是 branch）
- 等首次 push 触发 docs.yml 后会自动 deploy 到 `https://shetengteng.github.io/memex/`

---

### Phase 1：日常发版（每次小版本都走一遍）

```bash
# 1. 在 main 分支上完成所有功能改动 + 测试 + commit

# 2. bump 版本号
node scripts/sync-version.js bump patch       # 0.2.0 → 0.2.1
# 或者
node scripts/sync-version.js bump minor       # 0.2.0 → 0.3.0
# 或者直接指定
node scripts/sync-version.js 0.3.0

# 3. （可选）本地跑 upgrade 脚本，验证新版本能正常安装与运行
bash scripts/upgrade-local.sh

# 4. commit + push + tag
git add .
git commit -m "chore: bump version to 0.3.0"
git push
git tag v0.3.0
git push --tags        # ← 推 tag 会自动触发 GHA release.yml
```

### Phase 2：GitHub Actions 自动构建（约 10 分钟）

`release.yml` 会：

1. **matrix build**：在 `macos-14` (Apple Silicon) 与 `macos-13` (Intel) 两个 runner 上并行：
   - `cargo build --release --target <triple>`（带 `MEMEX_BUILD_TARGET` 保证 sidecar 同 arch）
   - `npx tauri build --target <triple> --bundles app,dmg`
   - 把 DMG 改名为 `Memex_<version>_aarch64.dmg` / `Memex_<version>_x64.dmg`
   - 上传 .dmg + .app.tar.gz + .app.tar.gz.sig 三个 artifact
2. **release job**：在 ubuntu runner 上：
   - 下载两组 artifact
   - 生成 `latest.json`（给 Tauri Updater 用）
   - 用 `softprops/action-gh-release@v2` 创建 GitHub Release，上传所有 assets

完成后 `https://github.com/shetengteng/memex/releases/tag/v0.3.0` 上会有：

- `Memex_0.3.0_aarch64.dmg`
- `Memex_0.3.0_x64.dmg`
- `Memex.app.tar.gz` + `.sig`（updater 用）
- `latest.json`

### Phase 3：更新 Homebrew Cask

```bash
# 1. 从 GitHub Release 下载 DMG，算 SHA256，patch cask
bash scripts/update-cask-sha.sh

# 2. 检查 cask 是被正确改写
git diff Casks/memex.rb
# 预期：on_arm 里 sha256 改成真值，on_intel 同理

# 3. 把改好的 cask 复制到 tap repo（首次）
cd ~/
git clone https://github.com/shetengteng/homebrew-memex
cd homebrew-memex
mkdir -p Casks
cp /Users/TerrellShe/Documents/personal/tt-projects/memex/Casks/memex.rb Casks/
git add Casks/memex.rb
git commit -m "chore: memex 0.3.0"
git push

# 4. 之后的版本 update 同样流程；可以写个 helper 脚本自动 copy + commit + push
```

### Phase 4：用户安装验证

```bash
# 用户首次
brew tap shetengteng/memex
brew install --cask memex

# 之后升级
brew upgrade --cask memex
```

---

## 后续优化项（可做可不做）

### O1：自动化 Phase 3

在 `release.yml` 末尾加一个 job：

```yaml
update-cask:
  needs: release
  runs-on: ubuntu-latest
  steps:
    - name: Checkout tap repo
      uses: actions/checkout@v4
      with:
        repository: shetengteng/homebrew-memex
        token: ${{ secrets.TAP_PAT }}   # 需要 PAT 因为 GITHUB_TOKEN 不能跨 repo write
    - name: Download DMG & compute SHA256
      run: |
        VERSION=${{ github.ref_name }}
        VERSION=${VERSION#v}
        curl -fSL -O https://github.com/shetengteng/memex/releases/download/v$VERSION/Memex_${VERSION}_aarch64.dmg
        curl -fSL -O https://github.com/shetengteng/memex/releases/download/v$VERSION/Memex_${VERSION}_x64.dmg
        # … patch Casks/memex.rb …
    - name: Commit & push
      run: |
        git config user.email "actions@github.com"
        git config user.name "GitHub Actions"
        git commit -am "chore: memex $VERSION"
        git push
```

需要：在主 repo 加一个 secret `TAP_PAT`（PAT 要有目标 tap repo 的 write 权限）。

### O2：Notarization（去掉 ad-hoc 签名）

加入 Apple Developer Program ($99/yr) 后：

- 在 GHA secrets 加 `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`
- 修改 `tauri.conf.json` 的 `signingIdentity` 为真证书
- `release.yml` 里加 notarization 步骤

用户安装时就**无需**跑 `xattr -cr` 了。

### O3：Tauri Updater 端到端

当前 `tauri.conf.json` 已配置 updater endpoint 指向 GitHub Release 的 `latest.json`。
GHA release job 已生成 `latest.json` 并签名，但**还没经过真实升级测试**。
建议下一次 release 后，用旧版本 app 验证一次 in-app update。

---

## 阻塞项汇总（按必要性）

| 阻塞项 | 阻塞什么 | 解决方法 |
|---|---|---|
| GitHub secret `TAURI_SIGNING_PRIVATE_KEY` 未配置 | release.yml 第一次跑会失败（updater signature 生成阶段） | Phase 0.1 |
| 没创建 `shetengteng/homebrew-memex` repo | Phase 3 无法 push cask | Phase 0.2 |
| `Casks/memex.rb` SHA256 是占位符 | brew install 会失败 | Phase 3，跑 `update-cask-sha.sh` |
| 还没有任何 release | brew install 没有可下载的 DMG | Phase 1+2，推第一个 tag |
| Apple Developer 证书没买 | 用户首次启动需要 xattr -cr | 可选；O2 |

---

## 当前可立刻验证的部分

不用买证书、不用 brew tap 也能做的：

```bash
# 本地跑完整 release pipeline 一次（除了 GHA）
node scripts/sync-version.js bump minor
bash scripts/upgrade-local.sh

# 验证 docs site
python3 scripts/build-docs.py
open site/index.html
```

把所有「软件本身能跑」的部分都验证通后，再走 Phase 0 → 1 → 2 → 3 上线 brew。
