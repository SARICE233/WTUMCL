# 上游 1.4.0 非核心提交落地清单（t2）

> 状态：**7 项已逐条处理**（landed 5 项 / deferred 2 项）｜整理日期：**2026-10-25**
> 上游基线：`refs/remotes/upstream/main` = `d3a732fe`（仓库 `UNIkeEN/SJMCL`）
> 本仓基线：`02cf1868`（WTUMCL `main`；origin = https://github.com/SARICE233/WTUMCL）
> 自检脚本：`.owv/checks/verify_noncore_adoption.py`（逐条断言本文件的 status 与产物）
> 来源说明：本文件按上游提交原始 diff（`git show <hash>`）逐条整理；相关说明文字为本仓改写，非上游原文。

## 0. 判定口径

- **landed**：上游改动已在本仓落地（或本仓已有等效实现），并给出产物位置与验证方式。
- **deferred**：本仓不能（或不应）原样落地，必须写明原因与恢复条件；不得留空。
- **品牌硬约束**：本仓为 WTUMCL 分支，文档与页面品牌、链接一律使用 WTUMCL（origin = https://github.com/SARICE233/WTUMCL）；不引入上游的 sjtu 域名、上游 issue/PR 链接，正文不出现裸上游品牌文案（仅允许出现在代码/路径/链接等引用位置）。
- **明确跳过**：`downloader_v2` 下载引擎重构及其配套（下载组命令、`download://` 事件、`src/services/download.ts`）属于核心重构，按任务要求跳过；上游 CHANGELOG 1.4.0 条目不引入（本仓 `CHANGELOG.md` 已有自己的记录）。

## 1. 逐条清单（7 项）

| # | 上游提交 | 主题 | 状态 |
| --- | --- | --- | --- |
| 1 | `888857c7` | docs(cli): reorganize the CLI introduction and usage guide | landed（改写适配） |
| 2 | `84f643a1` | docs(frontend): document download service commands and event subscriptions | deferred |
| 3 | `2abf0d22` | feat(ack): update the scl official website address in the acknowledgement page (#2063) | landed |
| 4 | `c8a538f1` | chore: auto update Minecraft version list, add 26.4-snapshot-5（只取版本数据） | landed |
| 5 | `e6da6008` | fix(frontend): correct Chinese update page URL on Linux (#2057) | deferred（不可移植）+ 本地衍生清理 |
| 6 | `4aeb0824` | chore(deps): bump next from 16.3.8 to 16.4.1 (#2084) | landed |
| 7 | `7efc8c46` / `7dfb6641` | fix(deps): upgrade zip to 9.0.5 / resolve RustSec advisories in TLS and QUIC | landed |

### 1. `888857c7` docs(cli): reorganize the CLI introduction and usage guide

- 状态：**landed（改写适配）**
- 上游改动：仅 `cli/README.md`（+49 / −11），把原先一段式的简介重排为「Build / Connect to the GUI app / Commands（MCP Tools、Shortcuts）」。
- 落地位置：`cli/README.md`（本次整体重写），包含小节 `## Build`、`## Connect to the GUI app`、`## Commands`、`### MCP Tools`、`### Progress and exit status`、`### Shortcuts`，并在文首注明改写自上游同名文档与该提交。
- 适配差异（重要）：上游同一提交里 `install` / `download` / `downloads` 三个快捷子命令的段落**未**照搬——本仓 `cli/src/main.rs` 只有 `CliCommand::Help` 与 `CliCommand::Call`（无 `mod install`，全文件 404 行），照搬文档会写出本仓并不存在的命令。重写后的文档只记录本仓真实行为：`-p/--port`（默认 18970）、通用工具调用 `<tool> [json-object]`、`wtumcl://run-silently` 唤醒与 5 秒重试、`--help` 联网时列出工具表、stderr 上的 spinner 与退出码约定，并在 `### Shortcuts` 中说明上游快捷命令本仓暂未实现。
- 证据：`cli/README.md` 存在且含上述小节；文中不出现 sjtu 域名与裸上游品牌文案。

### 2. `84f643a1` docs(frontend): document download service commands and event subscriptions

- 状态：**deferred**
- 上游改动：只给 `src/services/download.ts` 增加 73 行 JSDoc，覆盖 `DownloadService` 的 `submitGroup` / `pauseGroup` / `resumeGroup` / `cancelGroup` / `retryGroup` / `removeGroup` / `acknowledgeCompletion` / `snapshot` / `listTasks` 与 `download://*` 事件订阅。
- 原因：本仓**没有** `src/services/download.ts`，也没有对应的后端下载组命令与 `download://` 事件——本仓未采纳 `downloader_v2` 下载引擎重构（任务明确跳过核心重构），下载任务仍走 `src/services/task.ts` 的 `TaskService`（`schedule_progressive_task_group`、`task:progress-update`、`task:group-update`）。若照搬这些 JSDoc，会为一个不存在的服务文件写出指向不存在接口的文档，属于错误文档，因此不落地。
- 恢复条件：将来若整体采纳 `downloader_v2`（含下载组命令与 `download://` 事件）时，再随该重构一并移植这份 JSDoc。

### 3. `2abf0d22` feat(ack): update the scl official website address in the acknowledgement page (#2063)

- 状态：**landed**
- 上游改动：`src/pages/settings/about.tsx` 中 `ackList.scl` 由 `https://suhang12332.github.io/Swift-Craft-Launcher-Assets/web/` 改为 `https://swift-craft-launcher-web.pages.dev`。
- 落地位置：`src/pages/settings/about.tsx:45`（`scl: "https://swift-craft-launcher-web.pages.dev",`）。该地址是第三方项目（Swift Craft Launcher）的致谢链接，与 WTUMCL 品牌约束不冲突，直接采纳。
- 证据：`grep -n 'swift-craft-launcher-web' src/pages/settings/about.tsx` 命中；旧地址在文件中 0 命中。

### 4. `c8a538f1` chore: auto update Minecraft version list, add 26.4-snapshot-5

- 状态：**landed**
- 上游改动：仅 `src-tauri/assets/game/versions.txt` 追加一行 `26.4-snapshot-5`（提交带 `[skip ci]` 与 PR 号，均未带入本仓）。
- 落地位置：`src-tauri/assets/game/versions.txt`（末行由 `26.4-snapshot-4` 后追加 `26.4-snapshot-5`，保持原文件「一行一版本」的格式与字母/数字序）。
- 证据：`tail -1 src-tauri/assets/game/versions.txt` 为 `26.4-snapshot-5`，且该行在文件中只出现 1 次。

### 5. `e6da6008` fix(frontend): correct Chinese update page URL on Linux

- 状态：**deferred（不可移植）**
- 上游改动：`src/components/modals/notify-new-version-modal.tsx` 的 Linux 分支把 Linux 更新页地址由「上游站点 + `/sjmcl/<语言>`」改为「上游站点 + `/sjmcl`（中文）/ `/sjmcl/en`（英文）」——即修正中文页被拼成 `/sjmcl/zh` 的错误。
- 原因：该修复的目标地址是上游的 sjtu 域名双语文档站，本仓品牌硬约束禁止引入 sjtu 域名；且本仓该分支早已改写为 `https://github.com/SARICE233/WTUMCL/releases/latest`（单一地址，无中英分页），上游所修「Linux 更新页按语言拼错路径」的缺陷在本仓不存在，无可移植对象。
- 本地衍生清理（非上游内容，随本条一并交付）：删除该分支自品牌化改造后遗留、且从未被使用的 `const lang = isZh ? "zh" : "en";`（原 `src/components/modals/notify-new-version-modal.tsx:39`），避免后人误以为仍存在按语言分页的更新落地页。
- 恢复条件：无（除非本仓将来重新提供按语言区分的更新落地页）。

### 6. `4aeb0824` chore(deps): bump next from 16.3.8 to 16.4.1

- 状态：**landed**
- 上游改动：`package.json` 的 `next` 16.3.8 → 16.4.1，`pnpm-lock.yaml` 由 dependabot 重新解析。
- 落地位置：`package.json:53`（`"next": "16.4.1",`）+ `pnpm-lock.yaml`（在本机执行 `pnpm install --prefer-offline` 实际解析，next 16.4.1 与 `@next/swc-*` 16.4.1 等 54 个包被替换，`node_modules/next` 已是 16.4.1）。
- 证据：`node -p "require('./node_modules/next/package.json').version"` 输出 `16.4.1`；`pnpm install` 退出码 0（日志 `.owv/pnpm-install-t2.log`）；`pnpm tsc --noEmit` 通过。未采纳提交自带的 dependabot 说明与上游 PR 链接。

### 7. `7efc8c46` + `7dfb6641` fix(deps): zip 9.0.5 / RustSec TLS+QUIC 依赖

- 状态：**landed**（zip 部分为本仓既有等效实现，QUIC 部分本次补齐）
- `7efc8c46`（zip → 9.0.5）：本仓已由 `24572e13 chore(deps): migrate to zip 9 and stop ignoring its advisories` 完成同等迁移——`src-tauri/Cargo.toml:128` 为 `zip = { version = "9.0.5", default-features = false, features = ["deflate", "deflate64", "time"] }`，`src-tauri/Cargo.lock` 中 zip 为 9.0.5，且 `src-tauri/src/extension/helper.rs`、各加载器与 `src-tauri/src/utils/portable.rs` 的调用点已按 9.x API 迁移。本条不再重复改动，避免把依赖回退到旧版本。
- `7dfb6641`（TLS/QUIC RustSec）：`src-tauri/Cargo.lock` 中 `rustls 0.23.46`、`rustls-webpki 0.103.16`、`webpki-roots 1.0.10` 本仓已在上游目标版本；仅 `quinn-proto` 为 0.11.18，本次执行 `cargo update -p quinn-proto --precise 0.11.19` 对齐到上游的 0.11.19（仅 `src-tauri/Cargo.lock` 变动）。
- 证据：`src-tauri/Cargo.lock` 中 `quinn-proto` = 0.11.19；`cargo check --manifest-path src-tauri/Cargo.toml` 退出码 0。

## 2. 未采纳（明确跳过，不逐条展开）

- `downloader_v2` 下载引擎重构及其配套提交（`src/services/download.ts`、`download://` 事件、下载组命令等）：核心重构，任务要求跳过；对应第 2 条 deferred。
- 上游 `CHANGELOG` 的 1.4.0 条目：本仓 `CHANGELOG.md` 已有独立记录，不合并。
- 上游文档中的 sjtu 域名、上游 issue/PR 编号链接、裸上游品牌文案：按品牌硬约束改写或省略（对应第 1、5 条的处理方式）。

## 3. 本任务验证命令

| 命令 | 期望 |
| --- | --- |
| `python .owv/checks/verify_noncore_adoption.py` | 退出码 0，7 条逐项 PASS |
| `pnpm tsc --noEmit` | 退出码 0 |
| `cargo check --manifest-path src-tauri/Cargo.toml` | 退出码 0 |
| `pnpm locale diff en` / `pnpm locale diff zh-Hans` | 退出码 0 |
