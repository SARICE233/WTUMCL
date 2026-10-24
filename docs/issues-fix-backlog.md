# 上游未修 issue 与本地缺陷的差距清单（下一轮改动依据）

- 生成时间：2026-10-22 19:12（Asia/Shanghai）
- 数据源：`UNIkeEN/SJMCL` open issue 全量重抓（131 条）
- 本仓基线：`SARICE233/WTUMCL`，HEAD `96f66b12` + 工作区在途改动（57 个文件已脏）
- 边界：**只读核对，未改业务代码、未提 PR、未评论上游**；本轮产出仅本文档
- 上游快照差异：上一轮快照 132 条 → 现 131 条。期间仅 **#2067（收藏置顶，Feature）被关闭**，**无新增 issue**；因此上一轮的判定集合仍然有效，本文只重核「是否仍未修」

> 判定口径：能落到当前生产代码行且缺陷仍在 = 未修；代码已改但未验证 = 已修待回归；上游确认需重构 = 不排期。

---

## 1. 结论摘要

| 状态                           | 数量           | 说明                                                                                    |
| ------------------------------ | -------------- | --------------------------------------------------------------------------------------- |
| 仍未修（可本地修，本文给方案） | 12             | #1632 残留、#1272、#1734、#1933、#1971、#1938、#1914、#1934、#1980、#2026、#1942、#1977 |
| 仍未修（低置信，需先取证）     | 3              | #507、#751、#2062                                                                       |
| 已有静态新线索但仍需复现       | 1              | #2033                                                                                   |
| 已修（待回归验证）             | 1              | #1462                                                                                   |
| 不宜本地修                     | 3              | #1774（重构）、#1865（设计缺陷待重构）、#1685（死代码取舍）                             |
| 需实机/日志，本轮不动          | 其余需复现条目 | 见第 6 节                                                                               |

**主检出在途改动会撞车的文件**：`src-tauri/src/instance/commands.rs`（+123 行，在写周游玩时长）、`src-tauri/src/discover/commands.rs`（加 hitokoto 命令）、`src-tauri/src/utils/fs.rs`（权限枚举 cfg）。**#1971 / #1914 依赖 instance/commands.rs，#1938 依赖 fs.rs，必须在 main 落地后开工或另开工作树后 rebase。**

---

## 2. 未修项（按建议顺序，全部附现行代码证据）

### 2.1 #1938 SHA-1 为空串仍判失败（P2 / S / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1938
- 证据：`src-tauri/src/utils/fs.rs:445` 仍为 `if sha1 != truth {`；调用方 `src-tauri/src/launch/helpers/file_validator.rs:95` 仍把 `artifact.sha1`（可能 `""`）直接传入。
- 方案：`validate_sha1` 入口把空串视同未提供（`sha1.trim().is_empty()` 直接 `Ok(())`），并在 `file_validator` 两个入口同步判空；补单测覆盖 `""`、`"  "`、正确哈希、错误哈希。
- 验收：`cargo test validate_sha1`；构造 sha1=`""` 的库条目，修复后不再重下。
- 注意：`fs.rs` 当前在途改动只涉及权限枚举，冲突面小，可最先做。

### 2.2 #1971 列表为空却提示"服务器已存在"（P2 / S / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1971
- 证据：列表过滤隐藏项 `src-tauri/src/instance/commands.rs:528` `game_servers.retain(|server| !server.hidden);`；判重读全量 `…:573` `.any(|server| server.ip == server_addr)`。
- 方案：判重同样跳过 `hidden`（与列表口径一致）；若产品希望"直接连接"条目可见，另开一题改列表而不是改判重。
- 验收：先制造一条 hidden 服务器，再添加同 IP → 成功；添加重复的非 hidden → 仍报 DuplicateServer。
- 注意：`instance/commands.rs` 在途脏，建议等 main 提交后改。

### 2.3 #1980 目录名留空触发校验后弹窗错位（P3 / S / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1980
- 证据：`src/components/modals/edit-game-directory-modal.tsx:363` `{isDirNameEmpty && (` 条件插入错误提示，撑高 ModalBody；Modal 顶部对齐，后续控件整体下移。
- 方案：错误提示槽位常驻（固定高度 + `visibility` 切换），或在 `onBlur` 前不即时展示；避免点击"浏览"时布局跳动。
- 验收：名称留空 → 点"浏览"→ 返回后原坐标仍可点中目标控件；提示出现不改变容器高度。

### 2.4 #1734 字体列表排序不固定（P3 / S / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1734
- 证据：后端原序 `src-tauri/src/utils/commands.rs:42` `let sysfonts = system_fonts::query_all();`；前端直接拼接 `src/pages/settings/appearance.tsx:61` `setFonts(["%built-in", ...res.data]);`。
- 方案：前端 `setFonts(["%built-in", ...[...res.data].sort((a,b)=>a.localeCompare(b, undefined, {numeric:true}))])`，内置项固定置顶；不改后端。
- 验收：反复进入外观页，下拉顺序稳定。

### 2.5 #1933 取消未入队启动会杀掉正在运行的游戏（P1 / S-M / 中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1933
- 证据：`src-tauri/src/launch/commands.rs:413` 仍 `if let Some(launching) = launching_queue.last_mut()`，取消命令不带本次启动标识。
- 方案：`create_instance`/`launch` 返回或前端生成 `launchId`，取消命令改为 `cancel_launch(launch_id)` 按 id 定位；队尾无匹配则只清 pending 状态、不 kill。
- 验收：A 运行中 → 启动 B 并在 Java 选择阶段取消 → A 存活；再验证同实例重复取消幂等。
- 风险：前后端签名同时改，必须回归"正常启动—取消—再启动"时序。

### 2.6 #2026 单源失败阻塞整批新闻（P2 / S-M / 低-中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/2026
- 证据：`src-tauri/src/discover/commands.rs:109` `let results = futures::future::join_all(tasks).await;`；重试总预算 `src-tauri/src/utils/web.rs:93` 3600s。
- 方案：每个源套 `tokio::time::timeout`（如 8s），聚合改 `buffer_unordered`，允许部分结果先回；失败源在结果里带状态而不是整体报错。
- 验收：把任一源改成不可达，首屏仍按时返回其余源；新闻行 UI 显示"某源失败"。
- 注意：`discover/commands.rs` 在途脏（hitokoto），冲突面在文件尾部，可控。

### 2.7 #1942 镜像不可达时长重试、不切源（P2 / S-M / 中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1942
- 证据：下载请求被 `with_retry` 包裹 `src-tauri/src/tasks/download.rs:338`；重试预算同上 `web.rs:93`。
- 方案：下载路径单独给短预算（连接失败 3 次内切源），或按"错误类型"分层（连接/超时 → 快速失败切源；字节流中断 → 续传重试）。
- 验收：伪造一个不可达镜像源，5s 内切到下一源；正常源行为不变。
- 关联：#1914、#796 同属下载调度，建议与 2.8 一并排期。

### 2.8 #1914 取消下载后残留目录导致同名冲突（P2 / M / 中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1914
- 证据：`src-tauri/src/instance/commands.rs:1285` `dir_guard.commit();` 在排队后即放弃 RAII 清理；重试命中 `…:1066` `if version_path.exists() {` → ConflictNameError。
- 方案：取消/失败回调回收只含配置的半成品目录；或允许"空壳目录"复用（判定条件：仅存在 `<name>.json` 且无 `<name>.jar`/mods）。
- 验收：下载版本→取消→同位置重建成功；已完成实例不被误删。
- 注意：`instance/commands.rs` 在途脏。

### 2.9 #1934 全局 toast 锁导致并行安装漏收尾（P2 / S-M / 中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1934
- 证据：`src/contexts/task.tsx:473` `if (modLoaderLoadingToastRef.current) return newTasks;`，A 的 toast 存在时 B 的完成事件直接丢弃。
- 方案：锁按 `instanceId` 去重（`Map<string, ToastId>`），或完成事件一律入队、toast 仅做展示。
- 验收：两实例几乎同时装完加载器，两条都执行 `finishModLoaderInstall`；回归 Forge/NeoForge/Cleanroom/OptiFine 四分支。

### 2.10 #1272 字体数量大时外观页卡顿（P1 / M / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1272
- 证据：`src/pages/settings/appearance.tsx:208` `options={fonts.map((font) => ({`，MenuSelector 全量渲染；进页已由 `isLazy`(:222) 缓解，展开仍卡。
- 方案：字体选择改带搜索的弹窗 + 虚拟列表（Chakra 无内置虚拟化，可用 `react-window` 或自建窗口化），保留 `%built-in` 置顶。
- 验收：安装 2000+ 字体时展开菜单首帧 < 200ms，可键盘检索。

### 2.11 #1977 损坏 Java 触发 jli.dll 系统弹窗并卡住扫描（P2 / M / 中）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1977
- 证据：`src-tauri/src/launcher_config/helpers/java.rs:423` 对每个候选执行 `Command::new(java_path)`（`-XshowSettings:properties -version`），损坏安装由 Windows 加载器弹框阻塞。
- 方案：Windows 下先查 `bin/jli.dll`（或 `jvm.dll`）存在性，缺失直接跳过并记录；命令加超时，超时也跳过。
- 验收：放置一个删掉 `jli.dll` 的假 Java，打开 Java 列表不弹框、不阻塞，条目标记"不可用"。
- 说明：本轮无条件在 Windows 实机复现，改完需人工确认。

### 2.12 #1632 残留两处不走代理的客户端（P0 / S / 低）

- issue：https://github.com/UNIkeEN/SJMCL/issues/1632（主链路已修，仅剩两处）
- 证据：Forge 安装器 URL 预检自建客户端 `src-tauri/src/instance/helpers/loader/forge.rs:35` `let client = Client::builder().redirect(Policy::limited(5)).build()?;`（不读代理、无 UA）；匿名统计 `src-tauri/src/utils/sys_info.rs:18` `_ = reqwest::Client::new()`。
- 方案：forge 预检改用 `app.state::<reqwest::Client>()` 或在同函数复用 `build_sjmcl_client`；sys_info 若按产品决定"上报不走代理"，加注释固化，否则同样复用。
- 验收：开启代理且断直连，Forge 安装器预检仍能取到 installer 头；`cargo clippy` 无未用告警。

---

## 3. 低置信未修项（先取证再改）

| #     | 证据（现行代码）                                                                                                                      | 取证动作                                                                                          | 若坐实的改法                                                    |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| #507  | `src-tauri/src/launcher_config/commands.rs:270` 只用 `!directory.exists()`；`:267` 用字面 `PathBuf` 比较去重                          | 用 `C:\foo\..`、`\\`、`名称含 \$#*&%` 各试一次                                                    | `canonicalize` 后校验存在性与去重；非法字符前置校验             |
| #751  | `src-tauri/src/launcher_config/helpers/misc.rs:56-68` 启动时只要 `random_custom` 为真就无条件覆盖 `choice`                            | 按 issue 步骤：开随机→选回内置"银淞雪镇"→重启                                                     | `choice` 是显式内置项时不覆盖；仅在 `choice` 为空或非内置时随机 |
| #2062 | `src/components/modals/download-specific-resource-modal.tsx:268` `if (!selectedItem) return;` 静默返回；依赖弹窗回调再进入 (:270-277) | 控制台打点：第二次点"安装到实例"时 `selectedItem` 是否为空、`downloadOriginalResource` 是否被调用 | 点击时捕获 item 快照传入 action；失败给出 toast 而非静默 return |

## 4. 需复现但有新静态线索

### #2033 Fabric 实例被推荐 NeoForge 版本（P2）

- 证据：`getRecommendedFiles` 本身按加载器过滤（`download-specific-resource-modal.tsx:429-439`，`item.loader === curInstanceResourceModLoader`），逻辑正确；能推荐出 NeoForge 版本只可能是 **`curInstanceResourceModLoader` 的值来自错误实例/错误来源**，或 pack 的 `loader` 命名与实例值不一致（如 `neoforge` vs `neo_forge`）。
- 取证：在 :429 打点打印 `routeInstanceId`、`curInstanceResourceModLoader`、命中的 `pack.items[].loader`；对照实例 JSON 的 `modLoader`。
- 若为命名不一致：在比较处做归一化（lowercase + 去下划线）；若为来源错误：修 prop 取值，改从 `routeInstanceId` 对应实例读取。

## 5. 已修 / 不排期

| #     | 结论         | 依据                                                                                                         |
| ----- | ------------ | ------------------------------------------------------------------------------------------------------------ |
| #1462 | 已修，待回归 | `src/pages/standalone/game-log.tsx:181` 注释 "to fix issue#1462"，:275 自建剪贴板写入，按选区 slice 整段复制 |
| #1774 | 需上游       | 后端仍单体，`src-tauri/src/lib.rs:88` 单点 `generate_handler!`                                               |
| #1865 | 需上游       | 上游已确认设计缺陷，随下载系统重构                                                                           |
| #1685 | 需上游       | 组件从未随 release 发布，恢复即死代码；是否恢复应由上游定                                                    |

## 6. 其余需复现（本轮不动，仅登记）

#1372（Linux 菜单不显示）、#2056（内核串误判平台）、#2036（LWJGL3ify 误报）、#1954（1.5.1 及更早 Forge 安装）、#1863（模组页无法下滑）、#1784（内存设置页跳动）、#1716（西文弯引号宽度）、#1714（侧栏拉丁字母裁切）、#1008（离线仍报已是最新）、#2062 之外的表格条目（#1622/#1619/#1598/#1596/#1429/#1401/#1385/#1373/#1349/#1302/#1278/#1276/#1262/#1147/#1100/#961/#925/#796/#503/#376）。

## 7. 建议排期

1. **第一批（零冲突）**：#1980、#1734、#1934、#1632 残留、#1933。
2. **第二批（等 main 落地 `instance/commands.rs`/`fs.rs`）**：#1938、#1971、#1914。
3. **第三批（下载与新闻调度）**：#2026、#1942（可与 #1914/#796 合并成一次下载调度改动）。
4. **第四批（前端性能）**：#1272。
5. **取证后再排**：#507、#751、#2062、#2033。
6. **随时可做但需 Windows 实机**：#1977。

## 8. 本轮已知限制

- 未改代码、未跑端到端复现，全部判定来自当前工作树静态代码 + issue 正文（少数读了评论）。
- 引用行号对应当前工作树；若 main 提交后行号漂移，按函数名而非行号定位。
- 上一轮已给出 `.owv/issues-analysis.md` 的完整 27 条根因，本文只写"仍未修"与"怎么改"，两者配合使用。

## 9. 后续排期：内部标识符去 SJMCL 化（2026-10-24 追加）

仓库已于 2026-10-24 转为公开。以下内部标识符仍沿用上游拼写：

- `package.json` 的 `name`（`sjmcl`）；
- 用户数据目录里的 `sjmcl.conf.json` / `sjmcl.account.json`（`src-tauri/src/launcher_config/constants.rs:1`、`src-tauri/src/account/constants.rs:1`）；
- crate 名 `sjmcl-types` / `sjmcl-macros`（`src-tauri/crates/`）。

判定：这些属于内部实现名，不违反 LICENSE.EXTRA 第 1 条（该条约束的是分发软件名），但公开后容易让用户误以为「这只是 SJMCL 的皮肤」。

- **风险**：直接改配置文件名会让老用户读不到现有配置与账户，必须先做迁移层（读旧文件 → 写新文件 → 旧文件保留一个版本周期再删）。
- **建议顺序**：① crate 名（`sjmcl-types` → `wtumcl-types`，纯编译期，零迁移成本）→ ② 前端常量与 `window` 键 → ③ 配置文件名（带迁移）。
