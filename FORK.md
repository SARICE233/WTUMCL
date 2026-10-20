# WTU 方块学园 · 二次开发准备说明

本项目是基于 [SJMCL](https://github.com/UNIkeEN/SJMCL) 的二次开发分支，由 **WTU 方块学园** 组织维护并发行。

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
| 2 | 在 README、分发页面、支持文档、软件「关于」页声明基于 SJMCL 并给出源仓库 URL | 附加条款 2 | 待写（README 三语 + 关于页） |
| 3 | 统计上报的版本号带不少于两个字母的前缀 | 附加条款（统计要求） | 见第 4 节，前缀 `WTU-` |
| 4 | 分发二进制时能以 GPLv3 提供含本分支修改的完整对应源码 | GPLv3 | 发布仓库需公开 |

补充两点：

- **上游版权声明不可删除**。`tauri.conf.json` 的 copyright 与语言文件中的版权文案应为**双署名**，不是覆盖。
- 附加条款 3 的豁免（仅新增预设认证服务器时可沿用原名）**不适用于本项目**——本项目会大改品牌与功能。
- README 中 `NOT AN OFFICIAL MINECRAFT SERVICE.` 免责声明必须保留；产品名中避免出现 `Minecraft`。

## 2. 命名清单（待定稿）

以下为建议值，**未定稿**。技术约束已核实。

| 配置项 | 位置 | 建议值 | 约束说明 |
| --- | --- | --- | --- |
| productName | `src-tauri/tauri.conf.json:3` | `WTU Launcher` | 必须 ASCII：参与安装包文件名，Linux deb/rpm 的 Package 字段需降格为 ASCII |
| 界面显示名 | `src/locales/*.json` | `WTU 方块学园启动器` | 窗口标题、关于页等文案 |
| identifier | `src-tauri/tauri.conf.json:5` | `cn.wtu.blockacademy` | 反向域名；**决定数据目录**（%APPDATA%\cn.wtu.blockacademy），定稿后不可再改 |
| deep-link scheme | `src-tauri/tauri.conf.json:71` | `wtublock` | 需与第 3.1 节的代码常量同步修改 |
| 认证服务器自报名称 | `src-tauri/src/account/helpers/offline/yggdrasil_server.rs:147-148` | `WTU 方块学园` | `serverName` / `implementationName` |
| 版权 | `tauri.conf.json:34` + 语言文件 | 双署名 | 保留上游版权，追加本分支修改声明 |

## 3. 改动面清单

### 3.1 功能性（不改会出错）

| 位置 | 问题 | 处理 |
| --- | --- | --- |
| `src-tauri/src/launcher_config/helpers/updater.rs:17-33` | `SOURCES` 指向上游发布源，用户点更新会被**覆盖成官方版** | 换成自有发布源，或先禁用更新 |
| `src-tauri/src/utils/sys_info.rs:17-27` | 启动即上报版本/OS/exe SHA-256 到 `mc.sjtu.cn` | 保留并加 `WTU-` 前缀（推荐），或移除并在 README 说明 |
| `src-tauri/tauri.conf.json:71` + `src/services/deep-link.ts:4` + `src-tauri/src/lib.rs:46,68,384` + `src-tauri/src/launch/commands.rs:1019` | deep-link scheme 硬编码 `sjmcl://` | 三处一致替换，漏一处会导致导入整合包/启动链接失效 |
| `src-tauri/src/account/helpers/offline/yggdrasil_server.rs:147-148` | 对外自报 `SJMCL` | 改为自有名称 |
| `install.sh`、`PKGBUILD` | 安装脚本指向上游 | 改或删 |

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

### 5.2 CI

`.github/workflows` 共 13 个。Fork 后建议：

| 保留 | 调整 | 删除 |
| --- | --- | --- |
| `test.yml`（lint + 三平台构建） | `build.yml`、`release.yml`（改发布目标与 secrets） | `publish-winget.yml`、`publish-aur.yml`、`publish-snap.yml`、`sign-macos.yml`、`update_mod_data.yml`、`update_version_list.yml`、`post-release-publish.yml` |

`test.yml` 会跑 `pnpm eslint`、`cargo fmt --check` 与三平台 `pnpm tauri build`；`scripts/release/upload_to_sjmc_server.sh` 是上游专用上传脚本，需删除或替换。提交钩子 `.husky/pre-commit` 走 `pnpm lint-staged`。

## 6. 动工顺序

1. 定稿软件名、identifier、仓库地址（第 2 节）
2. 改 `tauri.conf.json`：productName / identifier / copyright / deep-link scheme
3. 同步改 deep-link 代码常量（第 3.1 节四处）
4. 换更新源或禁用更新器
5. 加 `WTU-` 版本前缀，移除或保留统计上报
6. 品牌串受控替换（跳过 URL）
7. 补 README 归属声明与双署名
8. 替换图标
9. 清理 CI 与上游专用脚本

## 7. 本机环境基线（已验证）

| 项 | 结论 |
| --- | --- |
| git 基线 | 已建立，`main` 对齐 `upstream/main@ba24b6dc`，工作区干净 |
| 子模块 | 已改 https 并初始化：`51efce8`（create-extension-CLI）、`83d1176`（vscode-extension，v0.2.0） |
| 前端依赖 | `pnpm install` 成功（39s），`pnpm-lock.yaml` 无变化 |
| Rust 工具链 | **1.99.1**（原 1.96.1 低于要求的 1.98.1，已升级） |
| 前端构建 | `pnpm build` 通过，44 个页面全部静态导出，`out/` 16.9 MB |
| 前端 lint | `pnpm eslint 'src/**/*.{js,jsx,ts,tsx}' --no-fix --max-warnings=0` 通过（CI 同款命令） |
| 后端类型检查 | **阻塞**：缺 MSVC 链接器，见 7.3 |
| 后端格式检查 | Windows 上 `cargo fmt --check` 必然失败，见 7.1 |

### 7.1 Windows 上 cargo fmt 的假阳性

`src-tauri/rustfmt.toml` 设置了 `newline_style = "Native"`：Linux 上 Native 即 LF，与仓库文件一致，CI 通过；Windows 上 Native 即 CRLF，而仓库里存的是 LF，于是 `cargo fmt --check` 会对**每一个** Rust 文件报 `Incorrect newline style`。干净的上游检出同样如此，不是本项目引入的问题。

可选处理（三选一，建议 a）：

a. 本地只跑 `cargo fmt`（不带 `--check`）后提交，格式校验交给 Linux CI；
b. 把 `newline_style` 改成 `"Unix"`（对 CI 无影响，本地与仓库一致）；
c. 新增 `.gitattributes` 写入 `* text=auto eol=lf`，统一各平台检出为 LF。

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

### 7.3 阻塞项：缺少 MSVC 链接器

依赖下载完成后，`cargo check` 在编译构建脚本阶段失败：

```
error: linker `link.exe` not found
note: the msvc targets depend on the msvc linker but `link.exe` was not found
```

排查结论（**不是代码问题**，是机器环境问题）：

- `C:\Program Files\Microsoft Visual Studio\2022`、`...\Microsoft Visual Studio\18` 等目录均为**空目录**，是卸载残留；`vswhere.exe` 不存在
- `C:\Program Files (x86)\Windows Kits\10` 下只有 `UnionMetadata`，缺 `Lib` 与 `Include`
- 系统内没有 `gcc` / `clang` / `ld`，也没有 MinGW、MSYS2、LLVM
- 当前会话**没有管理员权限**，无法安装 VS Build Tools

因此在装好 MSVC 之前，`cargo check`、`cargo build`、`pnpm tauri build` 全都无法执行。两条路：

**A（推荐，与 CI 一致）** 安装 Visual Studio Build Tools 2022，勾选「使用 C++ 的桌面开发」工作负载（含 MSVC v143 与 Windows SDK）。装完重跑 `cargo check` 即可。

**B（无管理员权限时）** 装免安装版 MinGW-w64，执行 `rustup target add x86_64-pc-windows-gnu`，用 GNU 目标编译检查。能验证源码可编译，但与 CI 上 Windows runner 的 MSVC 结果不完全等价。

### 7.4 提交身份

当前仓库本地身份是占位值，正式开发前请替换：

```bash
git config user.name  "你的名字"
git config user.email "你的邮箱"
```

## 8. 尚未完成（动工前需你决定）

- 软件名 / identifier / 仓库地址未定稿（见第 2 节）
- 后端编译检查待 MSVC 环境就绪后进行（见 7.3）
- 未做任何品牌替换与功能改动，本次只准备环境与文档
