# 下载 / 导入整合包 CPU 占用偏高：定位与修复简报

- 任务：t17（重点）
- 范围：`src-tauri/src/tasks/`（进度流、下载任务）、`src/services/task.ts`（前端进度事件消费）
- 基准提交：`39eb56ec`（本报告中的“改动前”行号均指该提交）
- 结论摘要：CPU 高不是单一原因。实测与代码路径表明有三层开销：
  1. **后端每 chunk 的固定开销**（8 KiB 拷贝缓冲 + 每块一次写锁/timer 注册）；
  2. **调度阶段 N 条 created 事件 × 前端 O(N) 聚合/渲染 = O(N²)**（实测 3000 个任务仅聚合就 357–437 ms，且每事件一次 React 提交）；
  3. **稳态 1 事件/秒/任务的 IPC → TaskContext 全量重渲染**（并发数 = CPU 核数，事件密度被并发放大）。

  本次修复直接消除了 (1) 的主要开销、把 (2) 的渲染提交合并到每 16 ms 一次，并补上断点续传路径上的两个既有缺陷；(3) 的组件层根因（`src/contexts/task.tsx` 的 O(N) 聚合 + 无 memo 的整列表重渲染）不在本任务写入范围，报告给出实测数据与后续建议。

## 1. 复现与测量边界

本 worker 会话无法启动 Tauri GUI，也没有受控大文件下载源，因此**没有做真实下载的进程级 CPU 采样**。为避免“凭感觉”下结论，采用三类可复现证据：

1. 代码路径逐段开销论证（含 tokio 1.52.3 源码中的缓冲常量和 PCL2/PCL2-CE 对照）；
2. Rust 单测 / 微基准（`cargo test ... -- --nocapture`，本机 Windows debug build）；
3. 前端聚合逻辑的 Node 微基准（`docs/dev/download-cpu-bench.mjs`，复刻 `contexts/task.tsx` 的 reducer）。

真实环境复测步骤见第 6 节；报告中的“预期收益”与“实测收益”分开标注，不把微基准当成端到端数据。

## 2. 根因分析（改动前 file:line + 机制）

### 2.1 后端下载热路径

| # | 位置（改动前） | 机制 | 影响 |
|---|---|---|---|
| A1 | `src-tauri/src/tasks/download.rs:212,214` | 用 `tokio::io::copy` 把响应流写入文件；tokio 1.52.3 的 `DEFAULT_BUF_SIZE = 8 * 1024`（`tokio-1.52.3/src/io/util/mod.rs:88`） | 每 8 KiB 一次 copy 循环：一次 `read` + 一次 `write`（文件）系统调用。**注意**：tokio-util 的 `StreamReader` 会把当前网络 chunk 缓存后分多次 `read` 返回（`tokio-util-0.7.18/src/io/stream_reader.rs` 文档中的 "Read the rest of the current chunk"），所以 `ProgressStream::poll_next` 次数 ≈ 网络 chunk 数而非 8 KiB 次数；8 KiB 缓冲放大的是**写系统调用与 read 方法调用**，不是进度事件数 |
| A2 | `src-tauri/src/tasks/streams/mod.rs:55,71-73` | 每个 chunk 先 `handle.write()` 查状态，再 `handle.write()` 调 `report_progress` | 每块两次写锁；锁内还做 `desc.current` 自增 |
| A3 | `src-tauri/src/tasks/streams/handle.rs:109-119` | `report_progress` 每块调用 `Interval::poll_tick(cx)` | 每块都注册/更新一次 tokio timer waker（即使 1 s 的 interval 远未到点）。**注意：进度事件本身已经是 1 s/任务一次，并非“每块 emit”**——这是本任务的一个常见误判，真正浪费的是每块的 timer 与锁操作 |
| A4 | `src-tauri/src/tasks/streams/handle.rs:111-117` | 到点才 `desc.save()` + emit | 正常，但落盘是同步 `std::fs`，在 async 轮询里执行（保留，见 4.6） |

结论：事件频率不是主因（1 Hz/任务），但**每 chunk 固定成本**在“并发数 = CPU 核数”（`monitor.rs:50-56` 的 `available_parallelism`，16/32 路同时下载）时被成倍放大。按 reqwest `bytes_stream` 常见的 8–64 KiB chunk 估算，每 GiB 有 1.6 万–13 万个 chunk，对应同量级的“两次写锁 + timer 注册”；64 KiB 写入缓冲则把每 GiB 的写系统调用从约 13.1 万次降到约 1.6 万次。

### 2.2 断点续传与校验路径的两个既有缺陷

| # | 位置（改动前） | 机制 |
|---|---|---|
| B1 | `download.rs:202` | `tokio::fs::OpenOptions::new().open(...)` 未设置 `write(true)`。std/tokio 的 OpenOptions 在 read/write 都未设置时按**只读**打开；续传分支（`current > 0`）随后 `seek` 再写入会失败，即**应用重启后的断点续传实际不可写** |
| B2 | `download.rs:222` | 校验用 `param.dest` 而非实际写入的 `self.dest_path`；当 manifest 给的是相对 cache 目录的路径时，校验的是进程 CWD 下的同名文件。且 `validate_sha1` 同步读全文件、直接在 async worker 线程上跑，大文件哈希会阻塞运行时线程 |

### 2.3 调度阶段的 O(N²)（CPU 尖峰的主因）

- `src-tauri/src/tasks/commands.rs:41-47`：每个下载任务 `DownloadTask::new` 都调用 `retrieve_launcher_config`（`download.rs:53` / `download.rs:91`），该方法 `clone()` 整个 `LauncherConfig`（`launcher_config/commands.rs:32-36`）。下载游戏（数千个 assets）或导入整合包（数百个 mod）时，调度命令里同步做数千次全量配置深拷贝。
- `monitor.rs:184-234` 的 `enqueue_task_group` 为每个任务同步 `emit_created`；前端 `src/contexts/task.tsx:248-409` 的监听器对每条事件执行 `setTasks` + `[...prevTasks]` + `updateGroupInfo`。
- `src/contexts/task.tsx:70-129` 的 `updateGroupInfo` 每次做 4~5 次全量遍历（filter/reduce/map）并 `sort` 整个 `taskDescs`；Created 分支还会 `unshift`（`task.tsx:286`）后立刻再聚合一次（`task.tsx:288`）。
- `src/pages/downloads.tsx:56-65` 在 `tasks` 变化时再对全量任务做一次 `map`；`taskDescs` 默认展开（`prev.find(...)?.[1] ?? true`），每次渲染都会为全部任务创建 React 元素（`downloads.tsx:284-332`）。

N 条事件 × O(N) 聚合/渲染 ⇒ **O(N²)**。用 `docs/dev/download-cpu-bench.mjs` 复刻该 reducer 实测（Node 22，本机；下面是一局高负载运行，另一局如下）：

```
node docs/dev/download-cpu-bench.mjs 3000
{ "createdEvents": 3000, "createdTotalMs": 436.8, "createdPerEventMs": 0.146,
  "progressEvents": 200, "progressTotalMs": 51.5, "progressPerEventMs": 0.258 }
node docs/dev/download-cpu-bench.mjs 500
{ "createdEvents": 500, "createdTotalMs": 21.3, "createdPerEventMs": 0.043,
  "progressEvents": 200, "progressTotalMs": 11.1, "progressPerEventMs": 0.055 }
```

多次运行 3000 任务的 created 聚合为 **201–437 ms**（机器负载不同），500 任务为 **8.7–21.3 ms**；单事件成本随 N 增长（0.017–0.043 → 0.067–0.146 ms），呈 O(N²)。第 9 节记录的是低负载一局的数值。这还未计入 React 为每条事件提交一次的全树协调。下载游戏时“created 风暴”与“稳态每秒十几条进度事件”都会触发该路径。

### 2.4 稳态 IPC / 重渲染

- `src-tauri/src/tasks/events.rs:107-127`：每次进度 emit 都序列化一个 `PEvent` 并走 IPC；
- 每任务 1 s 一次（`tasks/commands.rs:46`、`DownloadTask::new` 中的 `Duration::from_secs(1)`），并发 16 时约 16 次/s；
- 每条事件都会让所有 `useTaskContext()` 的组件重渲染（`head-navbar`、`download-indicator`、`instances-layout`、`downloads`……），且 `contexts/task.tsx:401` 每次都返回新数组。

### 2.5 日志、序列化与描述符落盘

- 下载热路径**没有** per-chunk 日志：`tasks/` 下的 `info!/warn!` 仅在任务失败（`monitor.rs:313`）或描述符加载失败（`monitor.rs:107`）时触发，因此日志频率不是本次 CPU 高的原因（逐项核对结论）。
- 每个到点报告会做一次 `PDesc::save`（`streams/handle.rs:142-151`）：同步 `std::fs::File::create` + `serde_json::to_writer`；频率 1 s/任务，3000 任务并发 16 时约 16 次/s，量级可接受，本次保留（改为异步落盘会引入描述符写入乱序、影响续传语义，属大改）。
- 进度事件本身很小（id + group + percent/current/eta/speed，`tasks/events.rs:23-28,107-127`），序列化与 IPC 成本随**事件条数**线性增长，而不是随字节数；本次没有改动事件协议。

## 3. PCL2 / PCL2-CE 参考（只借鉴逻辑）

| 参考点 | PCL2-CE | PCL2 (VB) |
|---|---|---|
| 拷贝缓冲 | `Modules/Network/Downloader/FileDownloader.cs:124` `const int bufferSize = 64 * 1024` | `Modules/Base/ModNet.vb:269` `CopyToAsync(Stream, 81920, ...)`（80 KiB） |
| 进度节流 | `FileDownloader.cs:227-243` 仅当 `now - lastProgressTick >= Stopwatch.Frequency / 5`（**200 ms，5 Hz**）才更新 UI，且速度按真实 elapsed 计算 | 进度为聚合属性，UI 侧按帧刷新 |
| 校验时机 | `FileDownloader.cs:213-225` 下载完成后在 `Task.Run` 中校验（不阻塞 async 线程），先写 `.tmp` 再 `PromoteTempFile` | 下载完成后校验 |
| 分块/限速 | `DownloadResourceManager.cs:94` 令牌桶 burst 128 KiB，`ThrottleAsync` 按字节计费 | 单连接顺序下载 |

SJMCL 与它们的差异集中在：**8 KiB 拷贝缓冲**、**每 chunk 的 timer 注册**、**校验在 async 线程**、**每任务重复深拷贝配置**。本次只按这四点做等价语义的最小改动。

## 4. 改动项（含参数说明）

### 4.1 下载写入：64 KiB 合并缓冲

`src-tauri/src/tasks/download.rs:36`

```rust
/// 64 KiB matches the sequential download buffer used by PCL2 (80 KiB) and
/// PCL2-CE (64 KiB) ...
const DOWNLOAD_BUFFER_SIZE: usize = 64 * 1024;
```

`download.rs:65-82` 新增 `copy_to_file`：固定 64 KiB 栈外缓冲 + `write_all`，替换 `download.rs:261,264` 两处 `tokio::io::copy`（限速分支仍然走 `limiter.limit(...)`，限速语义不变）。

预期：文件写次数从“每 8 KiB 一次”降到“最多每 64 KiB 一次”（上游 chunk 更小时按实际 chunk 写）。每 MiB 的 `read` 方法调用与 `write` 系统调用 128 → 16。该缓冲**不改变** `ProgressStream` 的 poll 次数（见 2.1 对 `StreamReader` 的说明），也不改变进度事件频率。

### 4.2 断点续传打开方式修正

`download.rs:85-97` `prepare_dest_file(dest_path, current)`：

- `current == 0`：`File::create`（截断，与旧行为一致）；
- `current > 0`：`OpenOptions::new().write(true).open(...)` + 一次性 `seek(Start(current))`（再没有每块 seek）。

> 这是对既有缺陷 B1 的修复：旧代码以只读方式打开续传文件，写入必然失败。修复后续传从持久化的 `current` 精确续写，前缀字节保持不变（有单测）。

### 4.3 校验：阻塞池 + 校验真正写入的文件

`download.rs:100-105` `validate_dest_sha1`：`tokio::task::spawn_blocking` 包装既有的 `validate_sha1`；`download.rs:274` 改用 `self.dest_path`。SHA1 算法、期望值来源、失败即 `Err` 的语义完全不变。

### 4.4 每任务配置深拷贝消除

`download.rs:56-60` `download_cache_dir` 只取 `config.download.cache.directory.clone()`，替换 `download.rs:53,91` 两处 `retrieve_launcher_config(...).clone()`。语义相同（同一把 `Mutex<LauncherConfig>`），每任务少一次全量配置深拷贝。

### 4.5 进度记账：每块只做累加，到点才合并/落盘/emit

- `streams/handle.rs:22`：`PHandle.interval: Interval` 改为 `report_interval: Duration`；
- `handle.rs:124` `add_progress`：仅 `desc.increment_progress`，不落盘、不发事件；
- `handle.rs:142` `flush_progress`：落盘 + 发**一次**进度事件；
- `handle.rs:133` `store_unreported_progress`：暂停时把未上报字节直接写入 `desc.current` 并落盘（见 4.6）；
- `streams/mod.rs:24-37`：`ProgressStream` 新增 `report_interval / next_report / pending` 三个字段；
- `streams/mod.rs:69-114` `poll_next`：
  - 每块：一次**读锁**状态检查 + `pending += size` + 一次 `Instant::now()` 比较；
  - 到点（默认仍是 1 s）：写锁、把 pending 合并进 `desc.current`、`save` + emit 一次；
  - 流结束：先把 pending 合并再 `mark_completed`，保证持久化的 `current == total`；
  - 取消/失败：状态既非 Stopped 也非 InProgress 时直接返回 `Ready(None)`，不再消费剩余块、不产生 completion（`streams/mod.rs:86-88`）。
- 事件频率**保持 1 s/任务**（不降低 UI 刷新率，也不增加 IPC），但每块不再碰 timer、不再取两次写锁、不再每块写描述符。

### 4.6 暂停/恢复的精确性

旧实现中 `desc.current` 每块都精确（只是 1 s 才落盘）；新实现把未上报字节留在 stream 内。为了不降低暂停落盘的精度，`streams/mod.rs:74-84` 在检测到 Stopped 时把 pending 写入 `desc.current` 并落盘（`handle.rs:133`），再挂起 waker。恢复后 pending 继续累计，行为与旧实现一致（单测覆盖）。

### 4.7 前端事件合并派发

`src/services/task.ts:18-19` 新增参数：

```ts
const TASK_PROGRESS_BATCH_INTERVAL_MS = 16;   // 一个 60 fps 帧
const TASK_PROGRESS_BATCH_MAX_SIZE = 256;     // 突发上限，达到即立刻派发
```

`task.ts:148-201`：`onProgressiveTaskUpdate` 把事件先推入缓冲，16 ms 后（或缓冲达到 256 条）一次性按序调用回调。React 18 会把同一 tick 内的多个 `setTasks` 合并为一次提交，因此 created 风暴期间的重渲染次数从 O(N) 降到 O(总时长 / 16 ms)。回调顺序、负载内容、API 签名均不变；卸载时清空缓冲，避免向已卸载组件派发。

## 5. 收益与实测数据

| 指标 | 改动前 | 改动后 | 证据 |
|---|---|---|---|
| 文件写次数（300,000 B） | 37 次（8 KiB/次） | 5 次（64 KiB×4 + 37856 B） | `tasks::download::tests::copy_to_file_batches_writes_into_download_buffer` |
| 每 MiB 写系统调用 | 128 | 16 | `DOWNLOAD_BUFFER_SIZE` + 上一条实测 |
| 每 chunk 进度记账 | 写锁 ×2 + `Interval::poll_tick` + 自增 ≈ **132–369 ns** | 读锁 + 自增 + `Instant::now` ≈ **43–86 ns** | `per_chunk_accounting_microbench`（debug，200k chunks，不同负载下多次运行区间） |
| 1000 个 chunk 的进度事件数 | （interval 未到点则 1 次，但每块都注册 timer） | 恰好 1 次 | `progress_events_are_coalesced_instead_of_emitted_per_chunk` |
| 进度事件间隔 | 1 s | 1 s（不变） | `progress_events_follow_the_report_interval`（测试用 100 ms 间隔） |
| 每任务配置深拷贝 | 1 次全量 | 0 | `download.rs:56-60` |
| 调度期 React 提交次数（N 条 created） | N | ⌈N / 16 ms 窗口⌉（缓冲 256 上限） | `task.ts:18-19,177-183` |
| 调度期纯 JS 聚合（3000 任务） | 357–437 ms | **未变**（见第 7 节） | `docs/dev/download-cpu-bench.mjs` |

说明：第 3 行是单核微基准（同一进程内先后跑两段循环，绝对值受机器负载影响，只能看相对趋势），非端到端；第 1/2 行是**确定性**的写入次数断言，与机器无关。前端微基准同理：同一台机器不同负载下，3000 任务的 created 聚合在 201–437 ms 之间波动、单事件 0.067–0.146 ms，500 任务在 8.7–21.3 ms 之间；关键是**单事件成本随 N 增长**（0.017→0.067、0.043→0.146）这一 O(N²) 趋势稳定复现，不是某一组绝对值。

## 6. 复测方法

### 6.1 自动化（无需 GUI/网络）

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib
cargo test  --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib tasks:: -- --nocapture
cargo fmt   --manifest-path src-tauri/Cargo.toml -- --check
node docs/dev/download-cpu-bench.mjs 3000
node docs/dev/download-cpu-bench.mjs 500
```

- `--nocapture` 会打印 `per-chunk accounting over 200000 chunks: legacy(...) X ns/chunk, current(...) Y ns/chunk`；
- 若要对比基线：**不要 `git stash`**（共享工作区里会连带移除其他 worker 的未提交改动）。用 `git worktree add ../sjmcl-base 39eb56ec` 建立只读基线工作区，或 `git show 39eb56ec:src-tauri/src/tasks/download.rs` 单独取出基线文件对照；微基准位于改动文件内，基线需从上述 worktree 运行。

### 6.2 真实下载（需要 dev exe + 网络，建议由 Orchestrator/Verifier 执行）

1. 固定条件：同一台机器、同一网络、同一 modpack（例如 `F:\Desktop\FarmingTales(含结构重置).zip`）或同一游戏版本；每次清空下载缓存目录。
2. 基线/修复各跑 3 次，记录：
   - `Get-Process WTUMCL | Select-Object CPU` 在固定 120 s 窗口内的**增量 CPU 秒**（CPU 时间比百分比更稳定）；
   - `Get-Counter '\Process(WTUMCL)\% Processor Time'` 采样均值/峰值；
   - 下载完成后的字节数，用于归一化（CPU 秒 / GiB）。
3. 判定：同一下载量下 CPU 秒下降即收益；若下降集中在“开始下载的前几秒”，说明主要收益来自 created 风暴的渲染合并。
4. 事件频率核对（可选）：在 `src/services/task.ts` 的 `flush` 里临时 `logger.info(batch.length)`，观察 1 s 窗口内的提交次数；修复后应按 16 ms 窗口合并，而不是每条事件一次。

### 6.3 限制

- 本 worker 会话没有 GUI/网络，未产生端到端 CPU 数字；微基准是 debug build，绝对值偏大，**相对趋势**可用。
- 前端收益依赖 React 的自动批处理（项目为 React 18），只能在真实 WebView 中量化。

## 7. 残余瓶颈与建议（不在本次写入范围）

1. `src/contexts/task.tsx:70-129` 的 `updateGroupInfo` 与监听器仍是 O(N)/事件；3000 任务的 created 风暴仍有 357–437 ms 的 JS 聚合成本（实测）。建议后续任务：
   - created 事件按组批量聚合（后端一次性下发组描述，或前端在 16 ms 缓冲内合并后只聚合一次）；
   - `updateGroupInfo` 改为增量统计（维护 `finishedCount/knownTotal`）并去掉每事件 `sort`；
   - `downloads.tsx` 的任务行加 `React.memo` + 只渲染可见窗口。
2. 边下边算 SHA1（当前下载后重新读整文件）可再省一次全文件读；需要处理续传前缀哈希，属中等改动，未做。
3. `create_resp_stream` 把网络错误映射成空 `Bytes`（`download.rs:226-230`），会掩盖中断并在某些情况下空转；属既有问题，本次未改，建议单独修。
4. 并发数默认 = CPU 核数（`monitor.rs:50-56`），在 32 线程机器上会同时跑 32 路哈希/拷贝；可考虑上限（如 `min(cores, 16)`）作为后续可配置项，本次未改以免影响下载速度语义。

## 8. 语义保持清单

| 语义 | 保证方式 | 证据 |
|---|---|---|
| 断点续传（HTTP Range + 文件偏移） | `current > 0` 仍发 `Range: bytes=current-`（`download.rs:188-193`，未改），文件按 `current` seek 续写 | `prepare_dest_file_resumes_at_persisted_offset` |
| 暂停时进度落盘 | 暂停时把未上报字节并入 `desc.current` 后落盘 | `pausing_persists_unreported_bytes_for_resume` |
| 暂停/恢复不丢块 | Stopped 时返回 Pending 并保存 waker，resume 后继续消费 | `stopping_pauses_polling_and_resuming_keeps_progress` |
| 完成后 current == total | 流结束前合并 pending 再 `mark_completed` | `progress_events_are_coalesced_instead_of_emitted_per_chunk` |
| SHA1 校验 | 仍校验实际写入文件，期望值/失败语义不变，仅移到阻塞池 | `validate_dest_sha1_checks_written_file` |
| 限速 | 仍通过 `Limiter::limit(...)` 包裹同一响应流，仅换拷贝循环 | `download.rs:259-266` |
| 事件间隔/协议 | 仍 1 s/任务、事件结构不变 | `progress_events_follow_the_report_interval` |
| 取消 | 取消后 `poll_next` 返回 `None`，不消费剩余块、不报告 completion；下载任务删除半成品文件（`download.rs:267-269`，既有语义） | `cancellation_stops_consumption_and_emits_no_completion` |


## 9. 本次复跑结果（Windows 11, debug build）

| 命令 | 结果 |
|---|---|
| `cargo check --manifest-path src-tauri/Cargo.toml` | 退出码 0（仅既有 dead_code 警告） |
| `cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib` | 47 passed; 0 failed; 1 ignored（`probe_external_modpack` 需外部文件，默认忽略） |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 退出码 0 |
| `pnpm exec eslint src/services/task.ts` | 退出码 0 |
| `node docs/dev/download-cpu-bench.mjs 3000` | createdTotalMs **201.4**，createdPerEventMs 0.067 |
| `node docs/dev/download-cpu-bench.mjs 500` | createdTotalMs **8.7**，createdPerEventMs 0.017 |
| `cargo test ... per_chunk_accounting_microbench -- --nocapture` | legacy **141 ns/chunk**，current **46 ns/chunk** |

前四项是硬门禁；后三项是机器相关的参考值，报告正文给出的是多次运行的区间。新增的取消路径测试（第 8 节最后一行）与原有续传/暂停测试同批通过，说明节流改动没有破坏失败/取消分支。
