# WTUMCL 走 `mcmirror.bvvd.mom` 的镜像请求：返回数据与正确格式对比

> 范围：WTUMCL（本仓库分支）在"自定义镜像"下载策略下会发出的每一类请求，逐类取回镜像实测数据，并与上游官方返回的正确格式比对。
> 结论先行：**内容层几乎全对（逐字节一致），传输层有一处硬缺陷（JSON 前缀丢弃 `Content-Length`、无视 `Range`）与一处稳定性缺陷（大文件 200 静默截断）**。

## 1. WTUMCL 的请求生成方式（代码依据）

- 下载源策略 `custom`：优先级为 `CustomMirror → BMCLAPIMirror → Official`（`src-tauri/src/resource/helpers/misc.rs:71-81`）。
- 自定义镜像按"官方 URL 前缀 → 镜像前缀"映射（`misc.rs:204-234`）：`launchermeta/`、`piston-meta/`、`piston-data/`、`launcher/`、`resources/`、`libraries/`、`forge-files/maven/`、`forge-maven/`、`neoforge-maven/`、`fabric-maven/`。
- JSON 里内嵌的 Mojang URL 由客户端自己改写目标源（`convert_url_to_target_source`，`misc.rs:277-311`；调用点 `instance/commands.rs:1111,1163`、`launch/helpers/file_validator.rs:100,193`、`launcher_config/helpers/java.rs:511`）。**镜像本身不回写 JSON 内的 URL**（响应体与上游逐字节一致，反证成立）。
- 下载器固定 `Accept-Encoding: identity`、续传发 `Range: bytes=N-` 并要求 **206**，否则报"Server ignored the Range request"（`tasks/download.rs:148-155,344-350`）；写入后按 SHA-1 校验（`download.rs:254-256,425-428`）。
- 超时：连接 10 s / 响应头 30 s / 读 30 s（`download.rs:40-51`）；共享连接池，并发上限 32。

## 2. 版本清单这一层的字段契约

`VersionManifest` 的条目结构带 `deny_unknown_fields`，只接受 `id,type,releaseTime,time,url`（`resource/helpers/version_manifest.rs:16-25`）。

实测镜像 `/launchermeta/mc/game/version_manifest.json` 的条目的**键恰为 `id,type,url,time,releaseTime`（无 `sha1`、无 `complianceLevel`）**，与上游逐字节一致（SHA-1 `eb16e8cb3cb361e68496c1121bbb41c30889a41f`，210058 B）。→ 当前解析安全；一旦上游给 v1 清单补上 `sha1`，WTUMCL 会整体解析失败（镜像会同步带过来）。

---

## 3. 测试前提与方法

| 项 | 内容 |
| --- | --- |
| 被测对象 | `https://mcmirror.bvvd.mom`（Azure Blob 存储 + Cloudflare 边缘，前置网关按前缀分流） |
| 对照基线 | 各前缀对应的官方上游（`launchermeta.mojang.com`、`piston-meta.mojang.com`、`piston-data.mojang.com`、`launcher.mojang.com`、`resources.download.minecraft.net`、`libraries.minecraft.net`、`maven.fabricmc.net`、`maven.minecraftforge.net`、`files.minecraftforge.net`、`maven.neoforged.net`） |
| 判定依据 | 状态码、`Content-Type`、`Content-Length`、`Accept-Ranges`/`206`/`Content-Range`、`ETag`/`304`、响应体 SHA-1 与上游是否逐字节一致、JSON 字段集合 |
| 工具 | `curl.exe --http1.1`，`-H "Accept-Encoding: identity"`，`--retry`，输出体做 SHA-1 |
| 采样时间 | 2026-10-22 16:16–16:58（Asia/Shanghai） |
| 上游可达性 | `maven.neoforged.net`、`maven.minecraftforge.net` 在本机多次超时（分别 21 s / 90 s 无字节），相应类别只能核对镜像自身可用性与上游已有样本，已在下文标注 |
| 样本 | MC 1.20.1（客户端 JSON、资源索引 5.json、资源 `icons/icon_16x16.png`、库 `java-objc-bridge-1.1.jar`、`client.jar`）、fabric-loader 元数据与 jar、forge `promotions_slim.json`/`maven-metadata.xml`/`1.20.1-47.2.0-installer.jar`、neoforge 元数据、Java 运行时清单 |

"正确格式"即上游官方端点在同类请求下的返回：**200/206 + 与资源类型相符的 Content-Type + 与上游逐字节一致的响应体 + 稳定的长度与断点续传语义 + 可缓存语义**。

---

## 4. 结论速览

| # | 请求类别 | 镜像实测 | 判定 |
| --- | --- | --- | --- |
| 1 | 版本清单 v1 `/launchermeta/mc/game/version_manifest.json` | 200 `application/json`，210058 B，SHA-1 `eb16e8cb…` 与上游完全一致 | ✅ |
| 2 | 版本清单 v2 `/launchermeta/mc/game/version_manifest_v2.json` | 200 `application/json`，278138 B，SHA-1 `f09921bb…` 与上游完全一致 | ✅ |
| 3 | 版本 JSON `/piston-meta/v1/packages/<sha1>/1.20.1.json` | 200 `application/json`，34974 B，SHA-1 `38d412af…` 与上游完全一致 | ✅ 内容 / ⚠ 传输头 |
| 4 | 资源索引 `/piston-meta/v1/packages/<sha1>/5.json` | 200 `application/json`，413067 B，SHA-1 `8a45c6be…` 与上游完全一致 | ✅ 内容 / ⚠ 传输头 |
| 5 | 资源对象 `/resources/5f/5ff04807…` | 200 `image/png` 781 B，一致；`Accept-Ranges: bytes`，`Range` → **206** + `Content-Range`；`immutable` 缓存 | ✅ |
| 6 | 库文件 `/libraries/ca/weblite/…/java-objc-bridge-1.1.jar` | 头与上游一致（CL 1330045、`Accept-Ranges: bytes`）；3 轮专测中 2 轮**以 HTTP 200 静默截断**（486665 B、359270 B），仅 1 轮取全且 SHA-1 `1227f9e0…` 正确 | ❌ |
| 7 | 客户端 jar `/piston-data/v1/objects/<sha1>/client.jar` | `Range: 0-262143` → 206、262144 B、SHA-1 `819320d9…` 一致，CL 23028853、`Accept-Ranges: bytes` | ✅ |
| 8 | 旧版客户端 jar `/launcher/v1/objects/<sha1>/client.jar` | `Range: 0-99` → 206 + `Content-Range: bytes 0-99/23028853` | ✅ |
| 9 | Java 运行时清单 `/launchermeta/v1/products/java-runtime/…/all.json` | 200 `application/json`，13385 B，SHA-1 `0f47bc50…` 一致；**首次请求长时间停顿后需重试** | ⚠ |
| 10 | Fabric 元数据 `/fabric-maven/net/fabricmc/fabric-loader/maven-metadata.xml` | 200 `text/xml`，9509 B，SHA-1 `03acf56a…` 一致；fabric-loader jar 1327691 B，SHA-1 `9c2be53d…` 与上游一致 | ✅ |
| 11 | Forge 推广表 `/forge-files/net/minecraftforge/forge/promotions_slim.json` | 200 `application/json`，4153 B，SHA-1 `1c3c166d…` 一致 | ✅ |
| 12 | Forge 安装包 `/forge-maven/…/forge-1.20.1-47.2.0-installer.jar`（5.97 MB） | 3 次尝试：一次连接超时、两次 **HTTP 200 但只给出 3.2 MB / 0.45 MB**（截断） | ❌ |
| 13 | NeoForge 元数据 `/neoforge-maven/…` | 200 `text/xml` 68481 B / API JSON 25751 B（上游 `maven.neoforged.net` 当时本机不可达，无法逐字节比对） | ✅（仅可用性） |
| 14 | 未知前缀 / 裸根路径 | 404 `text/plain` + 前缀使用说明（178 B） | ✅ 有提示 |
| 15 | 前缀内不存在的文件 | 404 且错误体为 **Azure XML / nginx HTML / Reposilite HTML**，从不返回 JSON | ⚠ 与上游同类（Mojang 自身也是 XML），但对解析器不友好 |

---

## 5. 传输语义（与"正确格式"的偏差集中区）

| 语义 | 上游官方 | 镜像 | 影响 |
| --- | --- | --- | --- |
| `Content-Length`（JSON 前缀 `/launchermeta`、`/piston-meta`） | **有**（如 34974、278138） | **无，`Transfer-Encoding: chunked`**；连 `HEAD` 也不给 CL、不给 `Accept-Ranges` | 进度显示退化为不确定；客户端无法用长度判断截断 |
| `Range`（JSON 前缀） | 支持，返回 **206** + `Content-Range` | **被忽略**：返回 **200** 全量体，无 `Content-Range`，无 `Accept-Ranges` | 分块/断点续传在这些路径上失效，客户端按 206 校验续传会报错 |
| `Range`（二进制前缀 `/resources`、`/libraries`、`/piston-data`、`/launcher`） | 支持 206 | **同样支持 206** + `Content-Range` + `Accept-Ranges` | 无偏差 |
| `ETag` / `If-None-Match` | 有 ETag | 有 ETag，条件请求返回 **304** | 一致 |
| `If-Modified-Since`（用资源自身 `Last-Modified`） | 通常 304 | 返回 **200** 全量 | 时间戳条件缓存失效 |
| `Cache-Control` | `max-age=120`（清单）/ `max-age=1209600`（包） | 清单 `max-age=3600` / 资源 `max-age=2592000, immutable` | 缓存更激进，元数据可能滞后最多 1 小时 |
| CORS | `Access-Control-Allow-Origin: *` | 相同；`OPTIONS` → 204 + `Allow-Methods/Headers` | 一致（浏览器内可用） |
| 内容压缩 | 支持 gzip/br | 支持；`gzip, br` + 解压后 SHA-1 仍与上游一致 | 一致 |
| 路径大小写 | 区分 | 区分（`/Piston-Meta/…` → 404） | 一致 |
| 双斜杠 | 容忍 | **404**（`//piston-meta//…` → 404） | 拼接需注意 |
| 查询串 | 忽略 | 忽略（`?t=1` → 200 且字节一致） | 一致 |
| 纯 HTTP | 通常 301 至 HTTPS | **直接 200**，无跳转且字节一致 | 可用（无需跟随跳转） |
| 并发 20（并行度 10） | — | 19×200 + 1 次连接超时，**无 429** | 未观测到限流 |
| 首字节延迟（同一资源 4 次采样） | 0.34 / 0.34 / 0.35 / 0.37 s | **0.83 / 0.84 / 2.29 / 6.77 s**，且多次 20–70 s 停顿 | 超时与重试策略必须放宽 |

### 5.1 最关键缺陷：HTTP 200 的"静默截断"

分块传输在数据发完前被提前关闭时，客户端看到的是**干净的 EOF**，而非错误：

- `forge-1.20.1-47.2.0-installer.jar`：55 s 内 3 次尝试分别得到 `000`（超时）、`200 / 3219456 B`、`200 / 453650 B`，均未取全 5973950 B。
- `java-objc-bridge-1.1.jar`（1330045 B）专测 3 轮：仅第 1 轮取全，另两轮分别以 **`200 / 486665 B`** 和 **`200 / 359270 B`** 干净结束；同一时段官方源一次成功、字节与 SHA-1 全对。
- 由于 JSON 前缀没有 `Content-Length`，截断的 JSON 只能靠 `serde`/`JSON.parse` 报错发现；二进制则只能靠 SHA-1 兜底。
- 结论：截断是**常态级**（大文件约 2/3 的尝试受影响），且与上游可达性无关。

---

## 6. 错误返回的三种形态（均非 JSON）

| 来源 | 状态 | Content-Type | 错误体 |
| --- | --- | --- | --- |
| 网关（未知前缀、裸根、双斜杠、大小写不符） | 404 | `text/plain; charset=utf-8` | `Not Found: 请使用 /launchermeta、/piston-meta、… 前缀。`（178 B） |
| Azure Blob（`/launchermeta`、`/piston-meta`、`/piston-data`、`/launcher`、`/resources`、`/libraries` 下缺文件） | 404 | `application/xml` | `<Error><Code>BlobNotFound</Code>…`（215 B，与 Mojang 上游同款） |
| 反向代理（`/fabric-maven`） | 404 | `text/html` | nginx 默认 404 页（146 B） |
| 反向代理（`/forge-maven`、`/neoforge-maven`，Reposilite） | 404 | `text/html` | Reposilite 404 页（1245–1302 B） |

> 注：`/fabric-maven`、`/forge-maven`、`/neoforge-maven` 是**实时反代**（错误页来自 nginx/Reposilite，元数据每次与上游一致），并非本地快照；`/forge-files` 下缺文件则落到网关文案。

---

## 7. 镜像侧修复清单（按优先级）

1. **让 JSON 前缀也走与二进制相同的存储直出**：至少补回 `Content-Length` 与 `Accept-Ranges`，并让 `Range` 返回 206 —— 这是唯一同时解决"截断不可见"和"续传不可用"的改法。
2. **消除静默截断**：确认边缘到 Blob 的回源在 chunked 收尾前不提前关连接；大文件（≥1 MB）优先 `Content-Length` 直出。
3. **错误体统一加 `Content-Type: application/json`**（或在 `application/xml` 时补 `X-Error-Format`），让启动器能明确区分"文件不存在"与"网关配置错误"。
4. **统一 `If-Modified-Since` 与 `ETag` 的 304 行为**；清单 `Cache-Control` 收到 `max-age=120` 量级。
5. **容忍双斜杠**（`//piston-meta//…`），并在网关返回 301。

---

## 8. 对 WTUMCL 的具体影响

| 风险 | 触发点 | 后果 | 现有兜底 |
| --- | --- | --- | --- |
| 大文件 200 截断 | 库文件、安装包、客户端 jar | 文件不完整 | SHA-1 校验 → 任务失败 → 重试；不会污染目录 |
| 读超时 30 s 撞上 20–70 s 停顿 | 所有下载 | 反复重试、耗时翻倍 | 有重试与连接池，可恢复 |
| JSON 前缀无 `Content-Length` | 版本清单/版本 JSON/资源索引（走共享 client，非下载任务） | 进度不可知、截断只能靠解析报错 | 依赖 `serde` 报错 + 源回退 |
| JSON 前缀 `Range` 被忽略 | 元数据走整段 GET | 目前不影响（元数据不续传）；若将来对元数据做续传，206 校验会失败 | `ensure_resumed_response` 会明确报错 |
| 资源缺失时返回 XML/HTML | 任意前缀下的缺文件 | `serde_json` 报 "expected value"，错误信息不指向 404 | 状态码先判，可接受 |
| 双斜杠 404 | 用户把 BaseUrl 填成 `https://host//` | 全站 404 | `normalize_custom_mirror_base` 只裁尾部斜杠 |

### 8.1 建议（WTUMCL 侧）

1. 自定义镜像回退链已存在，但**单个文件 200 截断应视为可重试错误**（当前走 SHA-1 失败 → 建议直接判定为 `Retryable` 而非普通失败）。
2. 续传前先看 `Accept-Ranges`；JSON 前缀无此头时直接整段重下，避免 206 校验误报。
3. 元数据请求（清单、版本 JSON、资源索引）加 `If-None-Match` 与本地缓存，规避 3–9 s 首字节与截断风险。
4. 错误体按 `Content-Type` 分流：`application/xml`/`text/html` 视为"镜像侧缺文件"，直接切下一个源，不做 JSON 解析重试。

---

## 9. 样本原始记录（可复现）

| 请求 | 状态 | Content-Type | 字节 | SHA-1 | 上游 SHA-1 |
| --- | --- | --- | --- | --- | --- |
| `/launchermeta/mc/game/version_manifest.json` | 200 | application/json | 210058 | eb16e8cb… | eb16e8cb… |
| `/launchermeta/mc/game/version_manifest_v2.json` | 200 | application/json | 278138 | f09921bb… | f09921bb… |
| `/piston-meta/v1/packages/38d412af…/1.20.1.json` | 200 | application/json | 34974 | 38d412af… | 38d412af… |
| `/piston-meta/v1/packages/8a45c6be…/5.json` | 200 | application/json | 413067 | 8a45c6be… | 8a45c6be… |
| `/resources/5f/5ff04807…` | 200 / 206 | image/png | 781 / 100 | 5ff04807… | 5ff04807… |
| `/libraries/…/java-objc-bridge-1.1.jar` | 200 ×3（2 次截断：486665、359270） | application/octet-stream | 1330045 | 1227f9e0…（取全时） | 1227f9e0… |
| `/piston-data/v1/objects/0c3ec587…/client.jar`（Range 0-262143） | 206 | application/octet-stream | 262144 | 819320d9… | 819320d9… |
| `/launcher/v1/objects/0c3ec587…/client.jar`（Range 0-99） | 206 | application/octet-stream | 100 | — | — |
| `/launchermeta/v1/products/java-runtime/…/all.json` | 200（首次 50 s 停顿） | application/json | 13385 | 0f47bc50… | 0f47bc50… |
| `/fabric-maven/…/fabric-loader/maven-metadata.xml` | 200 | text/xml | 9509 | 03acf56a… | 03acf56a… |
| `/forge-files/…/promotions_slim.json` | 200 | application/json | 4153 | 1c3c166d… | 1c3c166d… |
| `/forge-maven/…/forge-1.20.1-47.2.0-installer.jar` | 200 ×2 / 000 ×1 | application/java-archive | 3219456、453650（应为 5973950） | a2fad049…、a5d86782… | 官方源同批 90 s 无字节，未能比对 |
| `/forge-maven/…/forge/maven-metadata.xml` | 200 | text/xml | 213292 | c6f14d1c… | c6f14d1c…（一致） |
| `/neoforge-maven/…/neoforge/maven-metadata.xml` | 200 | text/xml | 68481 | 777ffd4d… | 上游不可达 |
