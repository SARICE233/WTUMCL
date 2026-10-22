# WTUMC（武汉纺织大学 MC 社区）· 二次开发准备说明

本项目是基于 [SJMCL](https://github.com/UNIkeEN/SJMCL) 的二次开发分支，由 **WTUMC** 组织维护并发行。

> 本文件记录二次开发前必须完成的前提条件与改动面清单。正式改名动工前请先读完第 1、2 节。

## 0. 基线与来源

| 项 | 值 |
| --- | --- |
| 上游仓库 | https://github.com/UNIkeEN/SJMCL |
| 基线提交 | `ba24b6dc`（upstream/main，2026-10-20） |
| 上游最新发行 | `v1.3.2`（2026-10-08，`944b0e4`） |
| 本仓首个自有提交 | `55015e02` build: use https URLs for git submodules |

初始快照来自 GitHub 的 `SJMCL-main.zip`。经与上游核对：该快照与 `upstream/main@ba24b6dc` 仅差 13 个文件（差异全部是上游更晚的修复），因此基线直接对齐上游 main，保留完整提交历史，后续可正常 merge。

### 0.1 同步上游

`upstream` remote 已配置（https）。同步流程：

```bash
git fetch upstream
git merge upstream/main     # 或 git rebase upstream/main
```

### 0.2 两个快照失真（已修正）

GitHub 的 zip 下载会破坏两类 git 对象，这里已通过直接对齐上游树修正：

- `.claude/skills`、`CLAUDE.md` 上游是符号链接（mode 120000），zip 里变成普通文件
- `tools/create-extension-CLI`、`tools/vscode-extension` 是子模块（gitlink），zip 里只剩空目录

`.gitmodules` 的 SSH 地址已改为 https，便于无 SSH key 的环境初始化子模块。

## 1. 授权义务（分发前必须全部满足）

上游以 **GPLv3** 发布，并依 GPLv3 第 7 条追加了条件，见 `LICENSE.EXTRA`。作为分发方，以下四条全部生效：

| # | 义务 | 依据 | 落实位置 |
| --- | --- | --- | --- |
| 1 | 不得使用 `SJMCL` / `SJMC Launcher` 作为软件名 | 附加条款 1 | 见第 2 节命名清单 |
| 2 | 在 README、分发页面、支持文档、软件「关于」页声明基于 SJMCL 并给出源仓库 URL | 附加条款 2 | 已完成（README 三语 + 关于页） |
| 3 | 统计上报的版本号带不少于两个字母的前缀 | 附加条款（统计要求） | 见第 4 节，前缀 `WTU-` |
| 4 | 分发二进制时能以 GPLv3 提供含本分支修改的完整对应源码 | GPLv3 | 发布仓库需公开 |

补充两点：

- **上游版权声明不可删除**。`tauri.conf.json` 的 copyright 与语言文件中的版权文案应为**双署名**，不是覆盖。
- 附加条款 3 的豁免（仅新增预设认证服务器时可沿用原名）**不适用于本项目**——本项目会大改品牌与功能。
- README 中 `NOT AN OFFICIAL MINECRAFT SERVICE.` 免责声明必须保留；产品名中避免出现 `Minecraft`。

## 2. 命名（已定稿）

| 配置项 | 值 | 位置 |
| --- | --- | --- |
| productName | `WTUMCL` | `src-tauri/tauri.conf.json` |
| identifier | `cn.wtu.wtumcl` | 同上；决定数据目录 `%APPDATA%\cn.wtu.wtumcl` |
| deep-link scheme | `wtumcl` | 配置 1 处 + 代码 6 处 |
| 界面短名 | `WTUMCL`（Logo，末位 L 蓝色高亮） | `src/components/logo-title.tsx` |
| 界面全名 | `WTUMC Launcher` | 同上（可随时改） |
| 认证服务器自报名 | `WTUMCL` | `yggdrasil_server.rs` |
| 版权 | 双署名：上游 + 本分支 | `tauri.conf.json`、7 个语言文件、README 三语 |
| 仓库 | https://github.com/SARICE233/WTUMCL（私有，**尚未推送**） | `origin` |

### 2.1 刻意保留的标识

以下属于**数据格式或生态兼容**标识，改名会破坏用户数据或社区扩展兼容性，故保留原样：

| 保留项 | 原因 |
| --- | --- |
| `sjmcl.ext.json`、`.sjmclx` | 扩展包格式；改动会与社区扩展及脚手架 CLI 不兼容 |
| `sjmclcfg.json`、`sjmcl.conf.json`、`sjmcl.account.json` | 本地数据文件格式 |
| `SJMCLError` / `SJMCLResult` 等 Rust 类型 | 纯内部类型，改名是无效改动量 |
| `api-sjmcl`、`mc.sjtu.cn` 等外部服务 URL | 上游服务端接口，非本仓所有 |
| `sjmcl-window-*` 等 DOM id | 内部标识，用户不可见 |
## 3. 改动面清单

### 3.1 功能性（不改会出错）

| 位置 | 问题 | 处理 |
| --- | --- | --- |
| `src-tauri/src/launcher_config/helpers/updater.rs:17-33` | `SOURCES` 指向上游发布源，用户点更新会被**覆盖成官方版** | 换成自有发布源，或先禁用更新 |
| `src-tauri/src/utils/sys_info.rs:17-27` | 启动即上报版本/OS/exe SHA-256 到 `mc.sjtu.cn` | 保留并加 `WTU-` 前缀（推荐），或移除并在 README 说明 |
| `src-tauri/tauri.conf.json:71` + `src/services/deep-link.ts:4` + `src-tauri/src/lib.rs:46,68,384` + `src-tauri/src/launch/commands.rs:1019` | deep-link scheme 硬编码 `sjmcl://` | 三处一致替换，漏一处会导致导入整合包/启动链接失效 |
| `src-tauri/src/account/helpers/offline/yggdrasil_server.rs:147-148` | 对外自报 `SJMCL` | 改为自有名称 |
| `install.sh`、`PKGBUILD` | 安装脚本指向上游 | 改或删 |
| `cli/src/main.rs` | 配套 CLI 的 deeplink 前缀与 MCP 服务名必须与应用一致 | 已同步为 `wtumcl://` 与 `wtumcl-mcp` |

### 3.2 打包与法律标识

- `src-tauri/tauri.conf.json:3,5,34,45`：productName / identifier / copyright / license
- `src-tauri/Cargo.toml:7`：repository
- `src-tauri/assets/icons/`：图标需替换

### 3.3 界面品牌（约 202 处）

- 7 个语言文件共 192 处 `SJMCL`（en 26 / es 26 / fr 28 / ja 28 / lzh 28 / zh-Hans 28 / zh-Hant 28）
- `.tsx` 中约 10 处：`src/pages/settings/about.tsx`、`src/layouts/main-layout.tsx`、`src/components/modals/star-us-modal.tsx` 等
- 注意：`src-tauri/src/launcher_config/models.rs:374-375`、`commands.rs:70,106` 指向的是上游服务端接口，是否保留取决于是否继续使用其服务

### 3.4 上游归属（必须保留，不可替换）

- README 三语及其徽章、贡献者链接
- `src/pages/settings/about.tsx` 中的 contributors 链接
- 社区扩展列表 URL（`src/locales/*.json` 中 `awesome-SJMCL-extensions`）

批量替换规则：**跳过含 `http` / `github.com` 的行**，逐条复核，禁止全局无脑替换。

## 4. 版本策略

```
包版本（semver，参与打包与更新比较）：  0.1.0
界面 / 关于页显示：                      WTU 0.1.0
上报给上游统计的版本字符串：             WTU-0.1.0
```

**关键**：前缀只能加在上报字符串上。`WTU-1.3.2` 不是合法 semver，写入 Cargo.toml / tauri.conf.json 会直接导致编译与打包失败。改动仅一行：

```rust
// src-tauri/src/utils/sys_info.rs
"version": format!("WTU-{version}"),
```

版本号在代码中的流向见 `launcher_config/helpers/misc.rs:25-37`（Release 取 package version，其余构建取 `dev` / `nightly` / `test-build`）。

## 5. 构建与发布

### 5.1 本地

```bash
pnpm install          # 已完成
pnpm build            # 前端静态导出到 out/
pnpm tauri dev        # 开发
pnpm tauri build      # 打包
```

工具链要求：Node 22+、pnpm 10+、**Rust >= 1.98.1**（`src-tauri/Cargo.toml` 的 workspace `rust-version`）。`.env` 可选（从 `.env.template` 复制，含 CurseForge API key）。

### 5.2 CI（已清理）

Fork 后保留 5 个工作流，其余上游专用流程已删除并通过 YAML 语法校验：

| 保留 | 说明 |
| --- | --- |
| `test.yml` | Lint（eslint + cargo fmt）+ 三平台构建 + 便携版打包 |
| `build.yml` | 可复用的多平台构建；产物前缀已改为 `WTUMCL_` |
| `release.yml` | 打 tag 触发；已移除 macOS 签名作业与 SJTU 服务器上传，改为直接创建 GitHub Draft Release |
| `audit.yml` | cargo 依赖安全审计 |
| `pr-size-label.yml` | PR 体积标签 |

已删除：`nightly.yml`、`post-release-publish.yml`、`publish-aur.yml`、`publish-snap.yml`、`publish-winget.yml`、`sign-macos.yml`、`update_mod_data.yml`、`update_version_list.yml`，以及 `scripts/release/upload_to_sjmc_server.sh`。

编译期环境变量统一改名（`build.rs` 按前缀收集）：`SJMCL_CURSEFORGE_API_KEY` / `SJMCL_BUILD_TYPE` / `SJMCL_COMMIT_SHA` → `WTUMCL_*`。**GitHub 仓库设置里的 secrets 也要同步改名**，否则构建时拿不到 CurseForge key。
## 6. 动工顺序（进度）

- [x] 定稿软件名 / identifier / 仓库地址
- [x] 改 `tauri.conf.json`：productName、identifier、copyright、deep-link scheme
- [x] 同步 deep-link 常量（Rust 5 处 + 前端 1 处）
- [x] 更新源改为本仓，移除上游服务器源
- [x] 统计上报加 `WTU-` 前缀（保留上报）
- [x] 品牌串受控替换（7 个语言文件 + 组件，跳过 URL）
- [x] README 三语归属声明与双署名
- [x] `rustfmt.toml` 改为 `newline_style = "Unix"`，使 Windows 本地格式检查可用
- [x] 替换图标：由 `WTUMC-AI-VEDIO/WTUMCLOGO.png` 适配生成全套（透明底、预乘重采样、ico/icns/appx/前端 Logo）
- [x] CI 清理（见 5.2）
- [x] 用户协议正式成文（`docs/TOS.md`，2026-10-20），README 徽章指向本仓、移除上游服务徽章与 banner 重绘
- [x] 清理 CI（13 个 workflow，多数为上游专用）
- [x] 同步配套 CLI：deeplink 前缀、MCP 服务名、提示文案
- [x] Cargo 包名改为 `WTUMCL`（产物为 `WTUMCL.exe`，与 productName 一致；`[lib]` 仍为 `app_lib`，代码无引用）
## 7. 本机环境基线（已验证）

| 项 | 结论 |
| --- | --- |
| git 基线 | 已建立，`main` 对齐 `upstream/main@ba24b6dc`，工作区干净 |
| 子模块 | 已改 https 并初始化：`51efce8`（create-extension-CLI）、`83d1176`（vscode-extension，v0.2.0） |
| 前端依赖 | `pnpm install` 成功（39s），`pnpm-lock.yaml` 无变化 |
| Rust 工具链 | **1.99.1**（原 1.96.1 低于要求的 1.98.1，已升级） |
| 前端构建 | `pnpm build` 通过，44 个页面全部静态导出，`out/` 16.9 MB |
| 前端 lint | `pnpm eslint 'src/**/*.{js,jsx,ts,tsx}' --no-fix --max-warnings=0` 通过（CI 同款命令） |
| 后端类型检查 | ✅ `cargo check` exit 0，0 错误（见 7.3） |
| 后端格式检查 | Windows 上 `cargo fmt --check` 必然失败，见 7.1 |

### 7.1 Windows 上 cargo fmt 的假阳性

`src-tauri/rustfmt.toml` 设置了 `newline_style = "Native"`：Linux 上 Native 即 LF，与仓库文件一致，CI 通过；Windows 上 Native 即 CRLF，而仓库里存的是 LF，于是 `cargo fmt --check` 会对**每一个** Rust 文件报 `Incorrect newline style`。干净的上游检出同样如此，不是本项目引入的问题。

已采用方案 b：`newline_style` 已改为 `"Unix"`，`cargo fmt --check` 现可在 Windows 本地通过（实测 exit 0），对 Linux CI 无影响。

### 7.2 网络与镜像（已处理）

本网络下官方源会停滞，已做两处替换：

- **rustup**：`static.rust-lang.org` 下到 71.5 MB 后无进展；改用 rsproxy 后成功升级到 1.99.1
- **cargo**：原有 `~/.cargo/config.toml` 使用 tuna 镜像，但其索引中 `dl` 仍指向 `static.crates.io`（退化成直连，下载 30 秒 0 字节后报错）。已改为 rsproxy 稀疏索引，原配置备份在 `~/.cargo/config.toml.bak-tuna`

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
```

更换后依赖可在数秒内完成下载。修好镜像会影响该机器上所有 Rust 项目，若其他项目需要 tuna 可换回备份。

**GitHub 访问**：本网络下 GitHub 的 **SSH 22 端口被拒绝**（`ssh: connect to host github.com port 22: Connection refused`），因此 `origin` 使用 HTTPS 地址，并已执行 `gh auth setup-git` 让 git 复用 `gh` 的凭据。若在其他网络想用 SSH，需另配 SSH key。

### 7.3 后端工具链（已解决）

**MSVC 已安装**：Visual Studio Build Tools 2022 装在 `F:\MicrosoftVisualStudio\2022\BuildTools`（避开 C 盘），缓存目录 `F:\MicrosoftVisualStudio\Cache`。

| 组件 | 版本/路径 |
| --- | --- |
| MSVC 工具集 | 14.44.35207（`F:` 盘，约 3.4 GB） |
| Windows SDK | 10.0.26100.0（`C:\Program Files (x86)\Windows Kits\10`，安装器固定路径） |
| link.exe | `F:\MicrosoftVisualStudio\2022\BuildTools\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\link.exe` |

**无需手动注入 vcvars**：rustc 与 `cc` crate 都会通过 vswhere 自动定位 MSVC/SDK。实测在 `link.exe` 不在 PATH 的普通 shell 里，`cargo check` 依然 **exit 0**（11.9s，增量）。

```bash
cargo check --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```
验证结果：`cargo check` **exit 0，0 错误**（19 条 warning 均为上游既有死代码/未用变量，不在本次改动文件中）。

**完整打包验证**：`pnpm tauri build` exit 0，release 编译 6m29s，产物 `src-tauri/target/release/WTUMCL.exe`（37.8 MB）与安装包 `bundle/nsis/WTUMCL_1.3.2_x64-setup.exe`（21.9 MB）。exe 元数据：ProductName `WTUMCL`、CompanyName `WTUMC`。
### 7.4 提交身份

当前仓库本地身份是占位值，正式开发前请替换：

```bash
git config user.name  "你的名字"
git config user.email "你的邮箱"
```

## 7.5 WTUMC 社区服务对接

站点：`https://wtumc.top:64053`（武汉纺织大学 MC 社区，源码在 `F:\Desktop\GEMINI-WEB\WTUMCweb-1.0-alpha`）。

| 能力 | 端点 | 接入方式 |
| --- | --- | --- |
| 社区新闻 | `https://wtumc.top:64053/api/news?limit=12&offset=N` | 新增 Rust 适配器 `src-tauri/src/discover/helpers/wtumc_news.rs`，把站点的 `{code,msg,data}` 信封映射为启动器的新闻契约；默认数据源已改为该端点 |
| 认证服务器 | `https://wtumc.top:64053/api/yggdrasil` | 已加入 `PRESET_AUTH_SERVERS`（排在第一位），账户页可直接选用 |
| 社区主页 / 用户群 | `https://wtumc.top:64053` | 帮助页、关于页、版本更新提示等链接 |

新闻条目映射规则：标题优先 `title_zh`，摘要取 `content_zh` 前 140 字，关键词用 `category`，`create_at` 补成 `${date}T00:00:00Z`，链接指向 `https://wtumc.top:64053/news/{id}`。

### 给 web 项目的三条反馈

1. **`skinDomains` 带了协议头**：`/api/yggdrasil` 返回 `"skinDomains": ["https://wtumc.top:64053"]`，authlib-injector 规范要求是**裸域名**（如 `wtumc.top`）。部分启动器会因此无法加载皮肤，建议改为裸域名。启动器侧已在注入前规范化为裸域名（2026-10-20），服务端仍建议修正。
2. **缺少 ALI 端点**：`/.well-known/authlib-injector` 返回 404，用户若只填站点根地址，authlib-injector 无法识别为认证服务器，必须手填 `/api/yggdrasil`。建议补一个 ALI 端点提升易用性。
3. **新闻 API 信封与启动器契约不同**：站点是 `{code,msg,data:{data:[...],total}}`，启动器期望 `{posts,next,cursors}`。已在启动器侧适配，若以后有其他 SJMCL 系启动器接入，可考虑服务端直接提供兼容端点。

### 已阉割的入口

本站暂无文档，因此移除了三个文档跳转入口（不再指向上游 SJTU 文档）：帮助页「启动器文档」、扩展页「扩展开发文档」、智能页「MCP 文档」。对应 8 个 key 已从 7 个语言文件移除（2026-10-20），`pnpm locale diff en` 通过。
## 8. 尚未完成

- ~~后端编译验证~~ 已完成：`cargo check` 通过（见 7.3）
- ~~用户协议正文~~ 已完成：`docs/TOS.md` 正式条款（2026-10-20）
- **其余上游文档链接**：语言文件中 `helpUrl`、用户群等仍指向 `mc.sjtu.cn`（描述的是继承自上游的功能文档），后续可迁移
- **GitHub Secrets 改名**：仓库设置里的 `SJMCL_*` 需改成 `WTUMCL_*`（用户决定暂时搁置）
- **banner/图标源文件**：设计源图仍在 `F:\Desktop\WTUMC-AI-VEDIO\`，如需重生成图标按第 9 节方法
- 按要求：**尚未向 origin 推送任何内容**

## 9. 图标生成方法（复现用）

源图为 664×664 不透明 PNG（白六边形 + 黑底）。适配流程：

1. 去掉 2px 边框伪影，从四角 flood fill 抠出六边形外部区域（阈值 160）
2. 对形状外缘 2px 环带按亮度还原 alpha（白描边叠在纯黑上的抗锯齿像素），RGB 置白，避免暗边
3. 按内容 bbox 裁切，居中放到 3% 内边距的正方形画布，缩放到 1024 母版
4. 所有尺寸使用**预乘 alpha**重采样（`convert("RGBa")` → resize → `convert("RGBA")`），否则小尺寸边缘会出现白边/毛刺
5. 输出 `32/64/128/128@2x/512` PNG、多尺寸 `icon.ico`、`icon.icns`、appx 全尺寸、`variants/square.png`，以及前端 `public/images/icons/Logo_128x128.png`、`Logo_32x32.png`

## 10. 网络与 GitHub 凭据

- Rust 与 crates 走 rsproxy 镜像（见 7.2）
- GitHub SSH 22 端口被本网络拒绝，`origin` 使用 HTTPS + `gh auth setup-git` 复用 gh 凭据

## 11. 微软 OAuth 客户端 ID

- 本项目自有 Azure 应用注册尚在申请中，暂用 **PCL2-CE** 的内置公共客户端 ID
  （`c14b0370-8d75-42f8-b329-5b60d39e319f`，取自其 v2.15.1 发布二进制并以
  consumers device-code 端点复核，见 `.owv/n3-client-id-check.md`；注意不是
  PCL2 正式版的 id）。
- 构建期可用 `MS_CLIENT_ID` 环境变量覆盖，自有 ID 批下后直接替换常量并去掉该兜底。
