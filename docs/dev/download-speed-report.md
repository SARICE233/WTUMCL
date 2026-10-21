# 整体网络下载速度偏慢：定位与修复简报

- 任务：t21
- 范围：`src-tauri/src/tasks/download.rs`、`src-tauri/src/tasks/monitor.rs`、`docs/dev/`
- 基准：工作区 `39eb56ec` + 同一轮已完成的 t17 下载热路径优化（64 KiB 拷贝缓冲、进度流节流、续传可写）
- 依赖确认：`cargo tree --manifest-path src-tauri/Cargo.toml -i reqwest@0.12.28` 显示 `tauri-plugin-http v2.8.1` → `reqwest v0.12.28`，即本仓库下载路径实际使用的版本；下述 timeout 语义以 0.12.28 源码为准。
- 结论摘要：**主因不是带宽，也不是请求头/Client 重建，而是共享 `reqwest::Client` 的 10 秒“总时限”被应用到了响应体**。reqwest 的 `ClientBuilder::timeout` 是包含 body 的总 deadline，任何超过 10 秒的文件传输都会在半途被中断；再加上旧代码把 body 错误吞成空 chunk，慢速源表现为“下载很慢/大文件反复失败”。本次为下载面单独建立长生命周期的 Client（连接 10 s / 单次读 30 s / 无总时限）、显式连接池、错误传播、Range 语义校验，并把自动并发从“CPU 核数”调整为 `2×核数 ∈ [8, 32]`。

## 1. 测量方法

本 worker 会话不能启动 Tauri GUI，托管环境的出网也被限速，**没有做真实 CDN 到打包应用的端到端 MB/s 对比**。为避免“凭感觉”，使用三类可复现证据：

1. **本地可控慢速 HTTP 服务 + Rust 单测**（不依赖外网）：用 `tokio::net::TcpListener` 起一个按固定间隔吐字节的 HTTP/1.1 服务，分别用“旧策略 Client”和“新下载 Client”下载同一 16 KiB 文件，比较实际收到的字节数与错误行为；另用 TCP accept 计数验证 keep-alive 连接复用。
2. **本机出网探针**（仅用于标定 10 秒窗口的大小，不作为应用速度基准）：`curl.exe` 访问 Cloudflare 测速端点。
3. **PCL2 / PCL2-CE 下载层源码对照**（只借鉴逻辑）与本仓库代码逐段核对。

### 1.1 本机网络探针（托管环境，结果仅用于说明 10 s 窗口有多小）

```
curl.exe -sS -o NUL --max-time 25 -w 'code=%{http_code} bytes=%{size_download} time=%{time_total} speed=%{speed_download} connect=%{time_connect} tls=%{time_appconnect}' \
  "https://speed.cloudflare.com/__down?bytes=5000000"
```

实测：

| URL | 结果 |
|---|---|
| `speed.cloudflare.com/__down?bytes=5000000` | HTTP 200，5,000,000 B / 23.95 s，**208,780 B/s**，connect 0.27 s，TLS 0.52 s |
| `speed.cloudflare.com/__down?bytes=3000000` | HTTP 200，3,000,000 B / 13.88 s，**216,097 B/s**，connect 0.27 s，TLS 0.52 s |
| `raw.githubusercontent.com`（小文件） | HTTP 200，connect 0.16 s，TLS 0.32 s |
| `bmclapi2.bangbang93.com` manifest | HTTP 302，connect 0.08 s，TLS 1.25 s |

托管环境单连接约 **0.2 MB/s**、RTT 约 0.27 s。也就是说 **10 秒最多只能传约 2 MB**；任何大于 2 MB 的文件在“10 s 总时限”下都不可能一次传完。该数值只用于标定，正式结论由 1.2 的可复现单测给出。

补充：同环境用 4 条 curl 连接并行各拉 1 MB（`--parallel-max 4`），总耗时约 11.27 s，聚合约 **0.35 MB/s**，比单连接约 0.21 MB/s 高约 1.7 倍。说明该托管环境对单连接限速明显、并发有收益；但这是代理/沙箱环境，不能外推到真实用户链路，因此本次把自动并发从“CPU 核数”调整为 `2×核数`（下限 8、上限 32），低核机器收益最大；但不据此宣称真实用户链路上成倍提速。

### 1.2 可复现单测（核心证据）

新增在 `src-tauri/src/tasks/download.rs` 的 `#[cfg(test)]`：

| 测试 | 复现内容 | 实测输出 |
|---|---|---|
| `total_deadline_truncates_slow_transfers_but_download_client_finishes_them` | 16 KiB 文件，每 25 ms 吐 1 KiB；旧策略 Client 用 200 ms 总时限（= 10 s 的 1/50 缩放），新下载 Client 用正式参数 | 旧：**7168/16384 B（43.8%）后 body 报错中断**；新：**16384/16384 B 完整完成** |
| `download_client_reuses_keep_alive_connections` | 同一 Client 连续 3 次 GET，同一 host，服务端统计 accept 次数 | **3 次下载 = 1 条 TCP 连接** |
| `truncated_body_surfaces_an_error_instead_of_a_silent_eof` | 服务端声明 `Content-Length: 1024` 但只发 128 B 就关闭 | 流收到 128 B 后返回 `ConnectionReset` 错误，而非静默 EOF |
| `resumed_downloads_require_partial_content` | `current > 0` 时服务端返回 200 / 206 | 200 被拒绝，206 通过；`current == 0` 不受影响 |
| `header_deadline_fails_when_server_never_responds` | 服务端接受请求但永不返回响应头 | 150 ms 上限触发，**154.5 ms 后返回错误**，不会永久挂起 |
| `resume_offset_clamps_corrupt_descriptor_values` | `-1`、`i64::MIN`、`0`、`42` | 负数一律归 0，正数原样保留 |
| `auto_concurrency_scales_with_cores_and_is_bounded` | 0/1/4/8/16/64/`usize::MAX` 核 | 8/8/8/16/32/32/32 |

运行命令与输出：

```
cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib tasks:: -- --nocapture
...
truncated body: received 128/1024 bytes before the stream error
3 sequential downloads used 1 TCP connection(s)
shared client (200 ms simulated total deadline): received 7168/16384 bytes, timed_out=true
download client (read timeout, no total deadline): received 16384/16384 bytes
silent server: failed=true after 154.4994ms
test result: ok. 17 passed; 0 failed; ...
```

旧策略在 43.8% 处中断不是测试误差：`ClientBuilder::timeout` 在 reqwest 0.12.28 中被包装进响应体（`reqwest-0.12.28/src/async_impl/body.rs` 的 `TotalTimeoutBody`，以及 `client.rs` 的 `total_timeout.fetch(&extensions)`），期限到达后下一次 `poll_frame` 直接返回 `TimedOut`。用 200 ms 只是把 10 s 等比缩小，让测试在 1 秒内跑完。

## 2. PCL2 / PCL2-CE 对比要点

| 维度 | SJMCL（改动前） | PCL2（VB） | PCL2-CE |
|---|---|---|---|
| 下载 Client | 全应用共享 1 个 Client（`utils/web.rs:37-66`），`timeout(10s)` **总时限含 body** | 下载引擎持有 1 个长生命周期 `HttpClient`（`Modules/Base/ModNet.vb:304-306`） | 独立下载器/连接池（`Modules/Network/Downloader/*`） |
| 超时 | 10 s 总时限（含 body） | 连接超时按平均连接时间动态取 15–30 s，且只约束每次请求/读（`ModNet.vb:907-919`） | 每次 `ReadAsync` 15 s（`AdaptiveRangeDownloader.cs:21,315-329`） |
| 单文件多连接 | 无：一个文件一条连接（仅支持续传 Range） | 有：大文件按“最大剩余分片再切 40%”动态加线程，单进程下载线程上限默认 **63+1**（`ModNet.vb:311-314,849-863,917`） | 有：≥4 MiB 文件按 **8 MiB/段** 切分，连接数 `NetTaskSingleFileConnectionLimit`，并发文件数默认 **7**（`PCL.Core/App/Config.cs:80`） |
| Range 校验 | 不校验，200 也会被当作续传内容追加 | 校验 Content-Length 与 `FileSize - DownloadStart`（`ModNet.vb:964-971`） | 校验 206 + `Content-Range` 三段（`AdaptiveRangeDownloader.cs:433-442`） |
| 拷贝缓冲 | 64 KiB（t17 已改） | 80 KiB（`ModNet.vb:269`） | 64 KiB（`AdaptiveRangeDownloader.cs:20`） |
| 压缩 | 未显式声明 | 默认 gzip/deflate（`ModNet.vb:304-306`） | Range 请求显式 `Accept-Encoding: identity`（`AdaptiveRangeDownloader.cs:150-151`） |
| 连接复用 | 依赖共享 Client 默认池；无显式池参数 | 单例 `HttpClient` 天然复用 | 独立 ResourceManager / 连接管理 |
| body 错误 | 吞成空 chunk，可能把截断文件当完成 | 抛异常并重试 | 抛异常，分段重试/拆分 |

共同点：PCL 系都不使用“覆盖整个 body 的总时限”，而是连接/单次读超时 + 失败重试；这正是 SJMCL 缺失的部分。

## 3. 根因（按优先级）

### P0：10 s 总时限把慢速传输截断

`utils/web.rs:39` 的 `.timeout(Duration::from_secs(10))` 被 reqwest 解释为**从建连到 body 读完的总 deadline**。慢速源上 10 s 只能收到固定字节数（本机约 2 MB），之后：

1. 若清单带 SHA1（游戏资源/模组基本都带），校验失败 → 任务失败 → 用户感知为“下载慢/总失败”；
2. 若不带 SHA1，旧代码把 body 错误 `Err(_) => Ok(Bytes::new())` 吞成 EOF，于是把截断文件标记为完成 → 文件损坏；
3. 若用户手动“继续”，会从持久化 offset 继续，但每个 10 s 窗口仍只前进固定字节数。

这解释了“CPU 问题已解决，但整体下载速度仍偏慢”：吞吐上限不是链路，而是 10 s × 源站速率。

### P1：body 错误被静默吞掉

`download.rs:227-231`（改动前）把所有 `reqwest` body 错误映射为空 chunk。超时、连接重置、半截响应都会伪装成正常结束，使“慢”与“损坏”都不可观测。

### P2：自动并发绑定 CPU 核数

`monitor.rs:50-56`（改动前）在 `auto_concurrent` 时用 `available_parallelism()`。下载是 I/O 密集，4 核机器只有 4 个文件并行；PCL2-CE 默认 7 个文件连接，PCL2 默认 64 个下载线程。大量小文件（assets/libraries）时并发不足会放大 RTT 与 TLS 握手占比。

### P3：续传/协议健壮性

- 续传只发 `Range`，不检查响应是否为 206：源站忽略 Range 时会把整文件追加到已存前缀后，产生损坏文件；
- 描述符里的 `current` 若为负（损坏/旧格式），会被 `as u64` 变成巨大偏移，seek 失败；
- 配置里 `concurrent_count = 0` 会让信号量永远拿不到许可，所有下载永久卡住（属于同一处边界）。

### 明确排除的怀疑项

| 怀疑 | 结论 |
|---|---|
| 每个 chunk 新建请求头 | 否，每文件只构造一次请求 |
| 每个文件新建 Client / TLS | 否，`state::<reqwest::Client>()` 是共享实例，`with_retry(...clone())` 共享连接池；未发现按文件的 Client 重建 |
| DNS 每次重解析 | 否，连接池复用期间不重新解析；只有新 host/新连接才解析 |
| 代理/镜像缺失 | 共享 Client 已读 `download.proxy`；镜像选择在资源层，不在下载任务层。新 Client 完整复刻了 UA/代理逻辑 |
| 进度事件/限速拖慢 | 否，事件 1 Hz/任务；限速是全局 Limiter，未启用时不参与 |

## 4. 改动

全部位于 `src-tauri/src/tasks/`：

1. **专用下载 Client，摆脱 10 s 总时限**（`download.rs:78-139`）
   - `connect_timeout = 10 s`、`read_timeout = 30 s`（每次成功读后重置）、**无总时限**；
   - `send()` 单独包 30 s 头等待上限（连接 + TLS + 重定向 + TTFB），因此“连上但永不响应”的服务器仍会失败，不会永久挂起；body 不受这个上限约束；
   - `pool_idle_timeout = 5 min`、`pool_max_idle_per_host = 64`、`tcp_keepalive = 30 s`、`tcp_nodelay`；
   - `static OnceLock<Client>`：所有下载任务共享同一 Client/连接池，不按文件重建；User-Agent 与代理配置在首次下载时读取一次（与原共享 Client 启动时构建的行为一致，改配置需重启）；
   - 构建失败时回退 `Client::new()` 并 `log::warn!`，不让下载功能整体不可用。
2. **响应头等待上限**（`download.rs:45,182-193,362`）：只约束到响应头到达为止，body 另由读超时保护；没有总时限但也不给死连接留悬挂机会。
3. **body 错误传播**（`download.rs:169-180,386`）：`Err` 变成 `io::Error(ConnectionReset)`，任务失败并可续传，不再静默吞掉截断。
4. **Range 语义**（`download.rs:152-164,368`）：续传必须 206，否则明确报错；`download.rs:144-146,397` 把负 offset 归零。
5. **请求头**（`download.rs:350`）：显式 `Accept-Encoding: identity`，保证 Range/续传拿到真实字节偏移（与 PCL2-CE 一致）。
6. **自动并发**（`monitor.rs:41-71`）：`2 × 可见核数` 并夹在 `[8, 32]`；手改配置的 0 值被 `.max(1)` 夹住，避免信号量死锁。
7. **保留** 64 KiB 拷贝缓冲、进度节流、断点续传文件打开/seek、SHA1 校验和全局限速器，语义不变。

## 5. 断点续传 / 校验 / 限速未被破坏

- **续传**：`resume_offset(desc.current)` 仍从持久化 offset 打开文件并 `seek`；新增“必须 206”只是拒绝会写坏文件的响应。单测 `prepare_dest_file_resumes_at_persisted_offset` 保留并通过。
- **校验**：仍在校验实际写入的 `dest_path`，SHA1 在 `spawn_blocking` 上执行；单测 `validate_dest_sha1_checks_written_file` 保留并通过。
- **限速**：`limiter.limit(reader)` 仍包在同一个读取路径上，限速器仍是 `TaskMonitor` 里的全局实例，语义未变。
- **取消/暂停**：`ProgressStream` 未改动；取消时仍删除目标文件，暂停时仍持久化未上报字节（`tasks::streams` 的既有测试全部通过）。
- **新失败路径**：连接中断→任务失败而非“成功”；服务端忽略 Range→失败而非写坏；坏 offset→从 0 重下而不是 seek 越界；0 并发→至少 1；连上但永不返回响应头→30 s 后失败。

## 6. 验证

本机 Windows / debug：

```
cargo check --manifest-path src-tauri/Cargo.toml
# exit 0（仅有本仓库既有 dead_code 警告）

cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib
# test result: ok. 60 passed; 0 failed; 1 ignored

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
# exit 0
```

## 7. 限制与后续

1. **没有真实 CDN 的端到端 MB/s 数据**。托管环境出网被限速到约 0.2 MB/s，且无法启动 GUI；本报告用“慢速服务 + 缩放总时限”的可复现单测证明机制，不把本地测试当作真实带宽收益。真实复测建议：在同一网络下分别用改动前/后构建下载同一个 >100 MB 文件，记录完成时间与 TCP 连接数（`Get-NetTCPConnection`）。
2. **单文件仍是单连接**。PCL2 会动态加线程切分，PCL2-CE 对 ≥4 MiB 文件按 8 MiB 分段并发。本次按“不做大重构”的要求没有实现多 Range 分段（它需要重做单文件进度聚合、暂停/续传和失败回收）；因此超大单文件在“单连接带宽受限”的链路上仍有上限，这是后续可做的最大优化点。
3. **读取超时 30 s / 响应头 30 s**。极慢但仍在吐字节的源（<1 B/30 s 的停顿）会被判失败，可续传重试；PCL2-CE 用 15 s，更激进。
4. **代理/UA 快照**：下载 Client 在首次下载时按当时配置构建，之后修改代理需重启（与原共享 Client 相同）。
5. **未改其他下载路径**：Java 运行时/启动器更新等若走共享 Client，仍受其 10 s 总时限影响，建议后续在 `utils/web.rs` 层面把“总时限”改为“连接 + 按读超时”，或统一复用本下载 Client。

## 8. 复现

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib tasks:: -- --nocapture
```

观察 `shared client ... received 7168/16384` 与 `download client ... 16384/16384` 两行即复现“10 s 总时限截断 vs 无总时限完成”；`3 sequential downloads used 1 TCP connection(s)` 即连接复用指标。
