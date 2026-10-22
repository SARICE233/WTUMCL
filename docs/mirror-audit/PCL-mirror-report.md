# PCL（Plain Craft Launcher）走 `mcmirror.bvvd.mom` 的镜像请求：返回数据与正确格式对比

> 范围：PCL 以 `mcmirror.bvvd.mom` 作为**自定义镜像源**时会发出的每一类请求，逐类取回实测数据并与正确格式比对。
> 说明：PCL2 闭源，其"自定义镜像源"按本仓库同款前缀契约拼接（本仓库把它称作 "PCL-style custom mirror"，`src-tauri/src/resource/models.rs:43`、`misc.rs:204`），故请求集合与 WTUMCL 基本同构；差异在**PCL 用 v2 清单、依赖 `Content-Length` 做进度、按分块 `Range` 并发下载**。带 ※ 的行为为按公开行为推断，未在本机反编译复核。

## 1. PCL 的请求生成方式

- 自定义镜像 BaseUrl + 前缀：`launchermeta/`、`piston-meta/`、`piston-data/`、`launcher/`、`resources/`、`libraries/`、`forge-maven/`、`forge-files/maven/`、`neoforge-maven/`、`fabric-maven/`。
- 版本列表用 **`version_manifest_v2.json`**，条目含 `sha1`、`complianceLevel`，PCL 用条目 `sha1` 校验下载到的版本 JSON（※）。
- 下载引擎按区间取块、并发连接、以 `Content-Length` 计算总进度与剩余时间（※，块大小见本仓库注释 `tasks/download.rs:33-36`：PCL2 为 80 KiB）。
- 加载器/Java 分别走 `forge-maven`、`neoforge-maven`、`fabric-maven` 与 `launchermeta/v1/products/java-runtime/…`。
- 缺文件时 PCL 会切下一个源或报错，不会把 XML/HTML 当 JSON 解析（※）。

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

## 8. 对 PCL 的具体影响

| 现象 | 与 PCL 预期的冲突 | 后果 |
| --- | --- | --- |
| JSON 前缀无 `Content-Length` | PCL 用 CL 算总大小/进度/限速配比 | 进度条空转、剩余时间失真；并发分块无法切分 |
| JSON 前缀 `Range` 被忽略（200 全量） | PCL 分块下载期望 206 + `Content-Range` | 分块退化为每块整段下载，流量与耗时成倍放大；若 PCL 严格校验 206，会直接判失败 |
| 大文件 200 静默截断 | PCL 以"收到 EOF 即完成"写入 | 版本 JSON/安装包被截断后解析失败或装出坏文件；仅 `sha1` 能兜住（安装包缺 `sha1` 时是死路） |
| 3–9 s 首字节、20–70 s 停顿 | PCL 的下载超时较紧 | 大量重试甚至超时中止 |
| `/assets/…`、`/maven/…`、`/version/<id>/json`、`/forge/download/…`、`/fabric/list`、`/minecraft/java/list` | 这些是 BMCLAPI 路径；本镜像**全部 404**（网关文案） | 把 BaseUrl 配成"BMCLAPI 风格"的镜像地址会全站失败；只能配前缀风格 |
| 404 返回 XML/HTML | PCL 的 JSON 解析 | 报"返回内容不是有效 JSON"，而非"文件不存在" |
| 双斜杠 404 | BaseUrl 末尾多斜杠 | 全站 404 |

> 结论：**作为"前缀风格"自定义镜像，PCL 的内容层可用；但传输层两处偏差（无 CL、忽略 Range）直接打在 PCL 的下载引擎上，属于会明显劣化体验、并在分块校验严格时导致失败的级别。**

### 8.1 建议（PCL 使用者侧）

1. 仅把本镜像用于**元数据与少量必需文件**，大文件（安装包、客户端 jar、库）优先直连官方或 BMCLAPI。
2. 若 PCL 允许，把下载源顺序设为 `自定义镜像 → BMCLAPI → 官方`，并在镜像 404/截断时快速切换。
3. 关闭"校验 206"类的严格选项（若版本提供），避免 JSON/元数据路径上的 200 触发误判。

---

## 9. 样本原始记录（可复现）

与 WTUMCL 同一批实测，重点差异项：

| 请求 | 状态 | Content-Type | 字节 | 与上游关系 |
| --- | --- | --- | --- | --- |
| `/launchermeta/mc/game/version_manifest_v2.json` | 200 | application/json | 278138 | SHA-1 完全一致（f09921bb…） |
| `/launchermeta/mc/game/version_manifest_v2.json` + `Range: 0-10` | **200**（非 206） | application/json | 278138（全量） | 无 `Content-Range`、无 `Content-Length` |
| `/piston-meta/v1/packages/38d412af…/1.20.1.json` | 200 | application/json | 34974 | 一致；`Range: 0-99` 仍返回 200 |
| `/piston-meta/v1/packages/8a45c6be…/5.json` | 200 | application/json | 413067 | 一致 |
| `/piston-data/v1/objects/0c3ec587…/client.jar` | 200（CL 23028853）/ 206（Range） | application/octet-stream | — / 262144 | 206 正常，`Accept-Ranges: bytes` |
| `/libraries/…/java-objc-bridge-1.1.jar` | 200 ×3（2 次截断：486665、359270） | application/octet-stream | 1330045 | 1227f9e0…（取全时） | 1227f9e0… |
| `/forge-maven/…/forge/maven-metadata.xml` | 200 | text/xml | 213292 | c6f14d1c… | c6f14d1c…（一致） |
| `/forge-maven/…/forge-1.20.1-47.2.0-installer.jar` | 200 ×2（3219456、453650 B）/ 000 ×1 | application/java-archive | 应为 5973950；官方源同批 90 s 无字节 | 两次静默截断 |
| `/neoforge-maven/…/neoforge/maven-metadata.xml` | 200 | text/xml | 68481 | 上游不可达，仅证实可用 |
| `/assets/5f/5ff04807…`、`/maven/…`、`/version/1.20.1/json`、`/forge/download/…`、`/fabric/list`、`/minecraft/java/list` | 404 | text/plain | 178 | BMCLAPI 路径不存在 |
