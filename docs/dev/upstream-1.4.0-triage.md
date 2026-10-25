# 上游 SJMCL 1.4.0 提交盘点（ba24b6dc..d3a732fe）

> 盘点对象：上游 SJMCL 仓库 1.4.0 发布区间 `ba24b6dc..d3a732fe` 内的全部 82 条提交。
> 本仓基线：WTUMCL `main`，盘点时 HEAD = `90969b3b`。
> 盘点时间：2026-10-25。复核方式见文末「附：复核命令」，或运行 `.owv/checks/verify_upstream_triage.py`。

## 一、本仓与上游的关系及盘点范围

### 1.1 关系

本仓是上游 SJMCL 的二次开发分支 **WTUMCL**（`origin` = https://github.com/SARICE233/WTUMCL），只维护本分支，不向上游提交 PR、不向上游推送；`upstream` 仅用于单向 `git fetch` / `git merge`，即上游改动只进不出。
因此上游提交对本仓只有两种价值：一是提供可参考的 bug 修复（本仓按需移植或已有等效实现），二是提供文档、依赖、版本数据等与核心架构无关的更新（可直接采纳）。
品牌与社区服务集成以本仓为准（例如更新检查地址指向本仓 releases），上游文案不能机械照搬。

### 1.2 范围

- 起点：`ba24b6dc`，即 `git merge-base HEAD upstream/main`，本仓与上游最后一个公共提交。
- 终点：`d3a732fe`（`upstream/main`，`docs(changelog): update changelog for 1.4.0 (#2102)`）。
- 区间内共 82 条提交，其中 5 条是 merge commit（`5d8b9787`、`e32769fa`、`682a1d9a`、`2e240833`、`9ce57e80`）。
- 盘点时本仓自 `ba24b6dc` 起另有 97 条自研提交，不在本表范围内。

判定方法：逐条执行 `git show --stat <hash>` 看实际改动文件，再对本仓对应文件做 `git log -S` / `grep` 核实是否已有等效实现；依赖类条目直接核对 `Cargo.toml`、`Cargo.lock`、`package.json`、`versions.txt` 的当前值。

## 二、分类口径与统计

| 分类 | 含义 | 数量 |
| --- | --- | --- |
| `already-covered` | 本仓在本次盘点之前已有等效实现（给出本仓对应提交） | 1 |
| `adopted` | 本次采纳的非核心更新（文档 / CI / 依赖 / 版本数据 / 小链接修复） | 10 |
| `ported` | 本次移植的上游 bug 修复（给出移植提交） | 11 |
| `deferred` | 暂缓落地，逐条写明原因 | 8 |
| `skipped-core` | 属于 downloader_v2 等核心重构，本仓不引入 | 52 |
| 合计 | — | 82 |

「落地位置」列给出本仓对应的提交短 hash；`—` 表示本轮无落地。

## 三、逐提交盘点（按时间正序）

| 提交 | 标题 | 分类 | 决定 | 理由 | 落地位置 |
| --- | --- | --- | --- | --- | --- |
| `03e51955` | refactor(download): replace task system with downloader engine | skipped-core | 不引入 | downloader_v2 的起点：新增 crate sjmcl-downloader 取代 src-tauri/src/tasks 的任务系统（81 个文件）。本仓 tasks 引擎是自研的（任务组、源健康、断点续传），引入等于替换整套下载子系统，不在本次范围。 | — |
| `48e62544` | fix(download): retry transient failures with exponential backoff | skipped-core | 不引入 | 指数退避重试写在 sjmcl-downloader/src/actor.rs 与 tests/engine_regressions.rs，依附新引擎。 | — |
| `5ad89c97` | fix(download): calculate group progress by completed tasks | skipped-core | 不引入 | 改 src/contexts/task.tsx 的进度聚合（deriveStats / DownloadFinishKind），本仓任务上下文是另一套数据模型。 | — |
| `777be1b0` | feat(download): run installation and verification as postprocessing tasks | skipped-core | 不引入 | 把安装与校验挂成新引擎的 postprocessing task（29 个文件），本仓安装流程不经过该引擎。 | — |
| `a57bd12c` | refactor(download): use DownloadTask for download submissions | skipped-core | 不引入 | 下载提交接口改为 DownloadTask（19 个文件，含 src-tauri/src/download.rs），本仓没有该模块。 | — |
| `695539bc` | feat(mcp): expose download submission and task controls | skipped-core | 不引入 | MCP 暴露新引擎的提交/控制命令（instance/commands.rs、intelligence/mcp_server）。 | — |
| `118aced6` | feat(cli): show progress for MCP downloads and installs | skipped-core | 不引入 | CLI 下载进度依赖上一条 MCP 工具，本仓 cli 没有下载子命令。 | — |
| `6830d1b0` | refactor(download): initialize downloader engine in plugin setup | skipped-core | 不引入 | 把新引擎初始化塞进 tauri 插件 setup（src-tauri/src/download.rs、lib.rs）。 | — |
| `2a565c88` | refactor(download): rename downloader plugin to sjmcl-downloader | skipped-core | 不引入 | 插件改名 sjmcl-downloader，含 capabilities 与 Cargo 配置。 | — |
| `edb84544` | chore(download): inherit downloader version from workspace | skipped-core | 不引入 | 新 crate 的版本改为继承 workspace，只动 crate 内文件。 | — |
| `141b2ef1` | style(backend): fix lint issues and clippy warning | skipped-core | 不引入 | 仅 sjmcl-downloader crate 的 lint / clippy 修正。 | — |
| `fd1d77be` | refactor(backend): separate installation preparation from instance refresh | skipped-core | 不引入 | 安装准备与实例刷新拆分（postprocess.rs、launcher_config/commands.rs），本仓安装流程结构不同。 | — |
| `4be99375` | refactor: remove obsolete first-run flags and retry name parsing | deferred | 暂缓 | 清理 is_first_run 死参数并重试实例名解析。本仓 src-tauri/src/instance/helpers/misc.rs:140 仍保留该形参、:417 的 refresh_and_update_instances 仍带此参，属可移植清理，但会牵动实例刷新与首次运行的兼容行为；本轮写域仅文档，留待后续 chore。 | — |
| `064a8a56` | refactor(backend): persist loader processors in installation tasks | skipped-core | 不引入 | 把 loader processors 持久化进安装任务（download.rs 与各 loader），依赖新任务模型。 | — |
| `04a2f218` | refactor(backend): remove unused Cleanroom installer preprocessing | deferred | 暂缓 | 删除 Cleanroom 安装器里已不用的 client.lzma 预处理。本仓 src-tauri/src/instance/helpers/loader/cleanroom.rs:107 仍在解压 data/client.lzma，:157 仍把 {BINPATCH} 传给启动参数，直接套用会删掉仍在使用的逻辑，需先确认启动参数演进。 | — |
| `7e919d3d` | perf(backend): reuse local libraries and avoid redundant verification | skipped-core | 不引入 | 复用本地库、跳过重复校验依赖新引擎的校验任务（9 个文件）。 | — |
| `bfe0c395` | fix(backend): make installation preparation retryable | skipped-core | 不引入 | 重试点在 src-tauri/src/instance/helpers/loader/postprocess.rs，本仓没有该文件。 | — |
| `e4225b04` | fix(backend): align Cleanroom library download and launch paths | deferred | 暂缓 | Cleanroom 库路径改用 downloads.artifact.path（新增 get_library_artifact_path）。本仓 src-tauri/src/instance/helpers/loader/cleanroom.rs:269、:313 仍只用 convert_library_name_to_path，是可移植的 bug 修复，但要连 launch 侧 file_validator 一起改，本轮写域仅文档。 | — |
| `36aeabf8` | fix(instance): refresh loader state and guard incomplete installations | skipped-core | 不引入 | 实例/启动状态修复建立在 src-tauri/src/download.rs 与新安装任务上（12 个文件，含 7 个 locale）。 | — |
| `9ce57e80` | Merge pull request #2023 from UNIkeEN/codex/separate-installation-from-instance-refresh | skipped-core | 不引入 | PR #2023 的 merge commit，内容即上方安装流程重构，自身无独立改动。 | — |
| `e6da6008` | fix(frontend): correct Chinese update page URL on Linux (#2057) | adopted | 已落地 | 上游修正 Linux 更新页的更新地址；本仓更新页早已品牌化为自有 releases/latest，URL 无法照搬，落地的是同一文件里 const lang 死变量的清理，方向一致（修 Linux 更新入口）。 | `3c5cd8c6` |
| `6b8b8405` | fix(download): finish cancellation of paused groups | skipped-core | 不引入 | 暂停组的取消收尾写在 sjmcl-downloader/src/actor.rs 与生命周期回归测试。 | — |
| `5e4d9307` | fix(install): wait for backend Java discovery | skipped-core | 不引入 | 等待后端 Java 发现依赖 postprocess.rs、download.rs 与 jre_selector，本仓安装流程没有这些钩子。 | — |
| `b987efd4` | fix(download): persist and reconcile pending completion actions | skipped-core | 不引入 | pending completion actions 涉及新引擎的权限、持久化与前端回调（19 个文件）。 | — |
| `52d583a8` | fix(download): preserve manifest sizes in queued tasks | skipped-core | 不引入 | 队列任务保留 manifest 大小，改 crate 的 command / download / model。 | — |
| `8146e617` | fix(download): stabilize remaining time estimates for mixed files | skipped-core | 不引入 | 剩余时间估算面向新任务模型（src/models/download.ts、src/utils/datetime.ts），本仓无 models/download.ts。 | — |
| `6100c2dd` | fix(download): exclude installation steps from file progress | skipped-core | 不引入 | 进度里排除安装步骤，同样基于新任务模型与 locales。 | — |
| `43d4f2ce` | fix(download): hide stopped progress and bound error details | skipped-core | 不引入 | 下载页与下载指示器跟新模型的状态/错误字段。 | — |
| `e9b85b26` | fix(download): hide excessive and invalid remaining times | skipped-core | 不引入 | src/utils/download.ts 本仓不存在，无落点。 | — |
| `682a1d9a` | Merge branch 'main' into refactor/downloader_v2 | skipped-core | 不引入 | downloader_v2 分支合并 main 的 merge commit，自身无独立改动（带入的主线提交已单列）。 | — |
| `c046a7aa` | fix(backend): register download engine state before starting task recovery | skipped-core | 不引入 | 引擎状态注册顺序，crate 内代码加示例。 | — |
| `46af6381` | feat(download): retry across sources before reporting failure | skipped-core | 不引入 | 跨源重试在 crate 与 src-tauri/src/download.rs。 | — |
| `d71b27bd` | refactor(download): filter restored task history, align navbar visibility with main | skipped-core | 不引入 | 恢复任务历史过滤与导航栏可见性对齐，分支内实现。 | — |
| `b62ffe7c` | refactor(backend): initialize shared HTTP client before downloader setup | skipped-core | 不引入 | 把共享 HTTP client 初始化提到 downloader 插件之前（src-tauri/src/download.rs 本仓不存在）。 | — |
| `84f643a1` | docs(frontend): document download service commands and event subscriptions | deferred | 暂缓 | 上游给 src/services/download.ts 补 73 行 JSDoc；本仓没有该文件（任务服务在 src/services/task.ts），没有对应落点。 | — |
| `c8a538f1` | chore: auto update Minecraft version list, add 26.4-snapshot-5 [skip ci] (#2060) | adopted | 已落地 | 上游机器人同步 Minecraft 版本列表并加入 26.4-snapshot-5；本仓已同步同一条数据。 | `a110b674` |
| `2abf0d22` | feat(ack): update the scl official website address in the acknowledgement page (#2063) | adopted | 已落地 | SCL 致谢页链接改为 https://swift-craft-launcher-web.pages.dev；本仓同址。 | `d75fd84c` |
| `5499a3a0` | fix(download): correct remaining time estimates without content-length | skipped-core | 不引入 | 无 content-length 时的剩余时间估算，改 crate 与前端 ETA 工具。 | — |
| `9be692ed` | fix(backend): fall back to full downloads when range requests are unsupported | skipped-core | 不引入 | range 请求不支持时回退整文件下载，crate 内。 | — |
| `b76d08b0` | fix(frontend): smooth download throughput for remaining time estimates | skipped-core | 不引入 | 前端吞吐平滑，改 src/models/download.ts 与 src/utils/download.ts（本仓均不存在）。 | — |
| `da498b9b` | fix(backend): disable transparent windows on linux | ported | 已移植 | Linux 透明窗口导致渲染异常：上游同时改 window.rs 与 tauri.linux.conf.json；本仓拆成两个同名提交分别覆盖后端逻辑与 linux 配置。 | `f83495d5`, `90969b3b` |
| `92468aec` | fix(backend): avoid counting buffered download bytes twice | skipped-core | 不引入 | 缓冲字节被重复计入进度，crate 内加 progress_accounting 测试。 | — |
| `64b9159e` | fix(download): localize task and command errors with structured error codes | skipped-core | 不引入 | 结构化错误码本地化，改 crate 命令/事件与前端 task 上下文（25 个文件）。 | — |
| `7dfb6641` | fix(deps): resolve RustSec advisories in TLS and QUIC dependencies (#2064) | adopted | 已落地 | RustSec 报告的 TLS / QUIC 依赖告警（Cargo.lock）。本仓 02fce6fd 统一升级 quinn-proto 与 rustls 使 cargo audit 通过，a0d67ab8 再把 quinn-proto 对齐到 0.11.19。 | `02fce6fd`, `a0d67ab8` |
| `7efc8c46` | fix(deps): upgrade zip to 9.0.5 to resolve RustSec advisories (#2066) | adopted | 已落地 | zip 升到 9.0.5 解决 RustSec 告警（Cargo.toml/lock、extension/helper.rs、cleanroom.rs 等）；本仓 24572e13 迁移到 zip 9 并移除 audit 忽略。 | `24572e13` |
| `0458d63e` | feat(instances): Implemented row-click selection and double-click-to-open-details on the instance page (#2068) | ported | 已移植 | 实例页整行点击选中 + 双击打开详情（#2068）；本仓 b58e0ee9 实现同一交互（instances-view / players-view / 列表页 + option-item）。 | `b58e0ee9` |
| `526a448d` | fix(instances): double-clicking an instance on a non-instance page now correctly navigates to the instance details. | ported | 已移植 | 非实例页双击实例也能进入详情；本仓 b58e0ee9 的双击处理覆盖同一路径。 | `b58e0ee9` |
| `a51803d9` | feat(instances): Implemented full-card click area for generic grid elements. Implemented full-row click for the player list. | ported | 已移植 | 通用卡片整卡点击 + 玩家列表整行点击；本仓 wrap-card 的 isFullClickZone 由 768763a8 引入，列表行点击由 b58e0ee9 落地。 | `768763a8`, `b58e0ee9` |
| `173688f0` | fix(backend): use a dedicated client for streaming downloads | already-covered | 无需改动 | 给流式下载单独建 client，避免共享 client 的 10 秒总超时截断传输。本仓已在 src-tauri/src/tasks/download.rs 实现专用下载 client 并写明原因（6acc4680），上游这套接线依附 downloader 插件，故不照搬。 | `6acc4680` |
| `6fb5dc1f` | fix(backend): retry failed range transfers as full downloads | skipped-core | 不引入 | range 传输失败后回退整文件下载，crate 内。 | — |
| `457e0955` | fix(download): smooth transfer rates and improve archive progress | skipped-core | 不引入 | 传输速率平滑与压缩包进度，crate 加前端指示器。 | — |
| `4bd8bec0` | feat(backend): prune download history older than thirty days | skipped-core | 不引入 | 下载历史 30 天清理，crate storage 加 launcher_config 配置项。 | — |
| `81a79ed5` | fix(backend): resolve CurseForge redirects before resuming downloads | skipped-core | 不引入 | CurseForge 重定向处理在 crate 与 src-tauri/src/download.rs。 | — |
| `db1b20ac` | fix(download): resolve resume redirects with HEAD and preserve live progress | skipped-core | 不引入 | 断点续传的 HEAD 重定向处理，crate 加 download.rs。 | — |
| `877c8e2c` | feat(frontend): add hover effect to wrap card, update code style | ported | 已移植 | wrap card 增加 hover 边框并统一代码风格；本仓 768763a8 引入 hoverBorderColor 与 _hover 边框，等效。 | `768763a8` |
| `ce5213e2` | test(download): remove tests added by downloader refactor | skipped-core | 不引入 | 删除本次重构新增的测试（cli、crate、前端 ETA 测试）。 | — |
| `ef8bd812` | refactor(client): simplify general and download HTTP client builders | deferred | 暂缓 | 纯重命名式重构（build_sjmcl_client 拆成 general / download 两个 builder），无行为变化；本仓已有独立的下载 client（tasks/download.rs），改名收益低。 | — |
| `e32769fa` | Merge remote-tracking branch 'origin/main' into refactor/downloader_v2 | skipped-core | 不引入 | downloader_v2 分支同步 main 的 merge commit，无独立改动。 | — |
| `888857c7` | docs(cli): reorganize the CLI introduction and usage guide | adopted | 已落地 | 重组 CLI 文档；本仓 cli 命令集不同（没有 install / download 子命令），dad40ad7 按本仓命令重写了使用指南。 | `dad40ad7` |
| `9b4e1f19` | refactor(backend): consolidate downloader persistence schema | skipped-core | 不引入 | 统一下载持久化 schema，crate 的 model / storage。 | — |
| `2e240833` | Merge pull request #2070 from NoirHare/main | skipped-core | 不引入 | PR #2070 的 merge commit，内容为 0458d63e / 526a448d / a51803d9，已由 b58e0ee9 与 768763a8 覆盖；merge 自身无独立改动。 | — |
| `94561910` | feat(extension): provide task command alias for compatibility in the extension host | ported | 已移植 | 扩展宿主为旧下载任务命令名提供别名；本仓 1160482d 同样加别名，但目标插件在本仓不存在，见第四节。 | `1160482d` |
| `5ceedd60` | fix(frontend): hide download details until total size is known | skipped-core | 不引入 | 总大小未知前隐藏下载详情；本仓 downloads 页是自有 taskDescs 结构，没有同名任务模型。 | — |
| `71dc54a9` | refactor(backend): remove redundant download engine initialization polling (#2072) | skipped-core | 不引入 | 去掉新引擎初始化的轮询，改 src-tauri/src/download.rs。 | — |
| `0779ac43` | fix(backend): report invalid Java runtime download entries | deferred | 暂缓 | Java 运行时清单的非法条目改为报错而不是静默丢弃。本仓 build_mojang_java_download_params（src-tauri/src/launcher_config/helpers/java.rs:548）构造的是 PTaskParam::Download，结构不同，需改写适配，本轮写域仅文档。 | — |
| `113e6d24` | feat(frontend): add reusable animated logo loader component | deferred | 暂缓 | 新增纯展示组件 logo-loader。上游自己在 main 上也还没有任何引用（git grep 只命中组件与样式文件本身），没有落地价值。 | — |
| `4150d4c3` | perf(download): batch persistence and streamline small-file scheduling | skipped-core | 不引入 | 批量持久化与小文件调度，crate 内。 | — |
| `94b1ae7e` | fix(backend): wait for worker exit before resuming paused downloads | skipped-core | 不引入 | 暂停组恢复前等待 worker 退出，crate 内。 | — |
| `72ce3c5b` | fix(download): track installation groups by explicit parent IDs | skipped-core | 不引入 | 安装组用显式 parent id 跟踪，crate 加 cli。 | — |
| `e15b0e48` | refactor(frontend): streamline download completion handling and retries | skipped-core | 不引入 | 下载完成处理与重试简化，改 src/contexts/task.tsx 与 downloads 页（新模型）。 | — |
| `5ec47476` | refactor(backend): remove legacy downloader schema migration | skipped-core | 不引入 | 删除旧的下载 schema 迁移，crate storage。 | — |
| `df533127` | fix(discover): limit request time in the news source list page (#2074) | ported | 已移植 | 新闻源列表限时（#2074）：后端的 per-source 超时本仓已在 880116bd 用 fetch_sources_isolated 实现（含并发上限与失败隔离）；本次只补前端 isLoading 在失败分支不落地的问题。 | `36ebeddc` |
| `5d8b9787` | Merge pull request #1952 from UNIkeEN/refactor/downloader_v2 | skipped-core | 不引入 | PR #1952（downloader_v2）的 merge commit，本身无改动。 | — |
| `80057af0` | chore(release): bump version to 1.4.0-beta.1 | adopted | 已落地 | 上游 1.4.0-beta.1 的版本位；本仓直接以 1.4.0 发布，该版本位被 ee54c055 覆盖。 | `ee54c055` |
| `4aeb0824` | chore(deps): bump next from 16.3.8 to 16.4.1 (#2084) | adopted | 已落地 | next 16.3.8 升级到 16.4.1；本仓 929e92d2 同步升级并更新 pnpm-lock。 | `929e92d2` |
| `c2672a61` | fix(frontend): limit dynamic selection menu height (#2085) | ported | 已移植 | 动态选择菜单限制最大高度（#2085）；本仓 076eb8a6 改的是同样三个文件、同样思路。 | `076eb8a6` |
| `ab6e16ca` | fix(backend): tolerate third-party texture download failures (#2092) | deferred | 暂缓 | 第三方账号（authlib）纹理下载容错。本仓 src-tauri/src/account/helpers/authlib_injector/texture.rs:46 仍是 fetch_image(...).await?，单个纹理失败即整表失败；官方 Microsoft 路径已由 02cf1868 做了容错，第三方路径没有，属明确缺口，见第四节。 | — |
| `4ad85ab6` | fix(frontend): preserve nested resource dependency modal stacking (#2081) | ported | 已移植 | 嵌套资源依赖弹窗层级错乱（#2081）；本仓 cdb1a96f 用 dependencyModalZIndex = 1450、先关父弹窗再开新弹窗、blockScrollOnMount = false 解决同一问题，未引入上游 shared-modal 的 {stack} 栈式 API。 | `cdb1a96f` |
| `1bcd85c3` | fix(frontend): allow switching mod loader type while loading (#2094) | ported | 已移植 | 加载版本列表时允许切换 mod loader 类型（#2094）；本仓 8d46d954 改 loader-selector 与 selectable-card 实现同一行为。 | `8d46d954` |
| `3fa5325b` | feat(frontend): open screenshot previews from full cards (#2095) | ported | 已移植 | 卡片整卡打开截图预览（#2095）；本仓 768763a8 落地（wrap-card 的 isFullClickZone/hover、screenshots 页、7 个 locale）。 | `768763a8` |
| `33b7e281` | chore(release): bump version to 1.4.0 | adopted | 已落地 | 上游 1.4.0 版本位（package.json / Cargo / tauri.conf.json）；本仓 ee54c055 同步 bump 到 1.4.0。 | `ee54c055` |
| `d3a732fe` | docs(changelog): update changelog for 1.4.0 (#2102) | adopted | 已落地 | 上游 1.4.0 changelog 更新（CHANGELOG.md 与中文版）；本仓 f9fdc7eb 已写入 1.4.0 条目并同步中文版。 | `f9fdc7eb` |

## 四、缺口与已知问题

### 4.1 本仓相对上游仍然缺少的上游 issue 修复

以下条目在上游已经修复并合入 1.4.0，本仓尚未落地，按影响排序：

1. **`ab6e16ca` 第三方账号纹理下载容错（#2092）** —— 最接近运行项目标的一条。本仓 `src-tauri/src/account/helpers/authlib_injector/texture.rs:46` 仍是 `image: fetch_image(app, skin.url.clone()).await?`，单个纹理请求失败就让整张纹理列表失败；`:61` 也只有 `textures.is_empty()` 时才补默认皮肤。官方 Microsoft 账号路径已由 `02cf1868` 做了逐纹理容错（`microsoft/texture.rs` 的 `assemble_textures` / `assemble_player_capes`），第三方（authlib）路径没有对等处理，与「皮肤/披风列表加载失败」是同一类故障。
2. **`0779ac43` Java 运行时清单校验** —— 上游把 Mojang Java 清单里的非法条目从静默丢弃改为显式报错。本仓 `src-tauri/src/launcher_config/helpers/java.rs:548` 的 `build_mojang_java_download_params` 在 `:600` 仍用 `.filter_map(... .ok()?)` 跳过坏条目，用户只会看到「下载失败」而没有原因。
3. **`e4225b04` Cleanroom 库路径改用 `downloads.artifact.path`** —— 本仓 `src-tauri/src/instance/helpers/loader/cleanroom.rs:269`、`:313` 仍只按 `convert_library_name_to_path` 拼路径，没有 `get_library_artifact_path`，对带 classifier 的库可能取错文件；需要连 `launch` 侧文件校验一起改。
4. **`4ad85ab6` 只落地了弹窗层级，未采纳 shared-modal 栈式 API** —— 本仓 `cdb1a96f` 用 `dependencyModalZIndex = 1450` + 先关父弹窗再开新弹窗规避，没有上游 `{stack:true}` / `modalId` 的通用栈。若后续再出现三层以上嵌套弹窗，需要重新评估。
5. **`4be99375` 清理 `is_first_run` 死参数** —— 本仓 `src-tauri/src/instance/helpers/misc.rs:140` 形参与 `:417` 的 `refresh_and_update_instances(app, is_first_run)` 仍在，属纯清理，未做。
6. **`84f643a1` 给下载服务补 JSDoc** —— 本仓没有 `src/services/download.ts`（任务服务是 `src/services/task.ts`），文档缺口无法对齐上游形态。

### 4.2 上游也还没修、我们已知道的问题

1. **扩展宿主的下载命令别名是悬空映射** —— `1160482d` 按上游 `94561910` 在 `src/contexts/extension/host.tsx:459-463` 注册了 `retrieve_download_groups` / `retrieve_download_tasks` 两个别名，但本仓没有 `plugin:sjmcl-downloader|snapshot` 与 `plugin:sjmcl-downloader|list_tasks` 这两个目标命令（全仓 grep 无其它出现），调用只会落到不存在的 plugin。上游因为 downloader_v2 才需要这层兼容，本仓引入 downloader_v2 之前这层别名没有意义。
2. **下载子系统已与上游分叉** —— 上游 1.4.0 的 52 条核心提交都围绕 `sjmcl-downloader` crate 与 `src-tauri/src/download.rs` 展开，本仓这两个路径都不存在，任务引擎是自研的（`src-tauri/src/tasks/`，含 `engine/`、`source_health.rs` 与专用下载 client）。这批提交不能 cherry-pick，上游后续对下载器的修复也只能按语义重写。
3. **上游新增的 `logo-loader` 组件自己没接入** —— `113e6d24` 只新增 `src/components/common/logo-loader.tsx` 与对应 CSS module，在上游 `main` 上 `git grep logo-loader` 只命中这两个文件本身，没有使用方。本仓不移植，等上游接入后再说。
4. **上游 changelog 以 PR 为单位汇总，不能反推提交数** —— `d3a732fe` 由 bot 生成，1.4.0 条目按 PR 号（#1952、#2023、#2070、#2095 等）聚合，一条条目往往对应本区间内多条提交（例如下载系统重构一段涵盖 40 余条）。判断某条修复是否存在于某个提交，仍须以 `git show` 的实际改动为准，这也是本文档逐条核对 stat 的原因。

## 五、后续建议

1. 下一轮优先处理 `ab6e16ca`（第三方账号纹理容错），与本次「皮肤/披风列表加载失败」诊断同源，改动集中在 `authlib_injector/texture.rs`，风险低、收益直接。
2. 其次处理 `0779ac43`（Java 清单校验）。本仓的 `PTaskParam::Download` 结构与上游 `DownloadTask` 不同，需要重写为「逐条校验 + 汇总错误」而不是逐行对照补丁。
3. `e4225b04` 与 `04a2f218` 涉及 Cleanroom 安装与启动两侧，建议单独开一轮，先确认 `{BINPATCH}` 是否仍被启动参数使用，再决定是删预处理还是改路径。
4. 纯清理型条目（`4be99375`、`ef8bd812`、`04a2f218`）可合并成一个 `chore` 提交批量处理，避免与功能修复混在一起。
5. `4ad85ab6` 的栈式弹窗 API 暂不引入；若后续出现更深层嵌套，再按本仓 `shared-modal` 的现状评估，不要为了对齐上游而整体替换。
6. downloader_v2 只做只读跟踪：每次上游发版后更新本文档第三部分的分类，不尝试合并代码；若上游未来把下载器拆成独立可替换的 crate 接口，再评估接入成本。
7. 清理 `1160482d` 的悬空别名，或补一个「本仓确实提供这两个命令」的实现，二选一，避免扩展开发者拿到永远失败的兼容入口。
8. 本文档作为下一次上游发版盘点的基线：新版本区间从本次的 `d3a732fe` 开始取 `git log`，表格格式与分类口径保持不变。

## 附：复核命令

```bash
# 区间内提交总数应为 82
git log --oneline ba24b6dc..d3a732fe | wc -l

# 逐条查看实际改动
git show --stat <hash>

# 本仓落地提交是否存在
git cat-file -e <本仓 hash>^{commit} && echo ok

# 文档自检
python .owv/checks/verify_upstream_triage.py
```
