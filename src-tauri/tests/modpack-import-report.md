# 整合包导入兼容性：实测、改造与测试报告

任务：t12 整合包导入格式兼容性增强（Farming Tales + PCL 参考）
范围：src-tauri/src/instance/helpers/modpack/、src-tauri/tests/
参考（只读）：F:\Desktop\_pcl-ref\PCL2\Plain Craft Launcher 2\Modules\Minecraft\ModModpack.vb、
F:\Desktop\_pcl-ref\PCL2-CE\Plain Craft Launcher 2\Modules\Minecraft\ModModpack.cs

## 1. 样例整合包实测（FarmingTales(含结构重置).zip）

样例结构（5969 条目）：根目录 modrinth.index.json + overrides/（5968 个文件），
Modrinth 规范格式；dependencies = {"minecraft":"1.20.1","forge":"47.4.10"}；files 共 299 条
（mods 286、resourcepacks 11、shaderpacks 2），每条均含 sha1/sha512/downloads/fileSize，
无 env 字段；zip 内文件名 UTF-8/ASCII 混用，无路径穿越项。

复现命令（临时探针在改造过程中执行；结论用同一命令在改造前后各跑一次）：

    $env:SJMCL_MODPACK_PROBE='F:\Desktop\FarmingTales(含结构重置).zip'
    $env:SJMCL_MODPACK_PROBE_EXTRACT='1'
    cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib probe_external_modpack -- --ignored --nocapture

改造前输出（在旧 parser 上以临时单元测试直接调用 get_parsers 链）：

    parser 0 (CurseForge): ERR SJMCLError("specified file not found in archive")
    parser 1 (Modrinth):   OK overrides="overrides/" mc=Ok("1.20.1") loader=Ok((Forge, "47.4.10"))
    parser 2 (MultiMC):    ERR SJMCLError("MODPACK_MANIFEST_PARSE_ERROR")
    modrinth name="FarmingTales_ForgeⅡ v1.5.2fix2" version="1.0.0"
      summary=Some("正式版 1.20.1, Forge 47.4.10") files=299
    extract OK files=5968 elapsed=6.3975255s
      has config/forge-client.toml=true / has servers.dat=true

改造后输出（忽略测试，环境变量提供外部包路径，不写死桌面路径）：

    overrides="overrides/" extra=["client-overrides/"] mc=Ok("1.20.1") loader=Ok((Forge, "47.4.10"))
    modrinth name="FarmingTales_ForgeⅡ v1.5.2fix2" version="1.0.0"
      summary=Some("正式版 1.20.1, Forge 47.4.10") files=299
    meta=Ok(ModpackMetaInfo { name: "FarmingTales_ForgeⅡ v1.5.2fix2", version: Some("1.0.0"),
      description: Some("正式版 1.20.1, Forge 47.4.10"), author: None,
      modpack_source: Modrinth, client_version: "1.20.1",
      mod_loader: Some(ModLoader { status: Installed, loader_type: Forge,
      version: "47.4.10", branch: None }) })
    download_params=299
    extracted_files=5968 elapsed=8.345478s

### 结论（第 1 条验收）

**该样例在改造前就能被现有解析器链识别**：manifest 解析、overrides 解压（5968 个文件）、
下载参数（改造后实测 299 条）都没有失败。失败点不在样例本身，而在解析器对同格式
*变体* 的假设过窄；具体实测/代码证据如下：

1. Modrinth 只按 zip 根目录查找 modrinth.index.json（旧 modrinth.rs:67），
   根目录多包一层目录即无法识别；CurseForge 同样只查根目录 manifest.json。
2. Modrinth 结构体把 name/versionId/files/dependencies 全部设为必填（旧 modrinth.rs:56-60），
   缺失 name/versionId 时直接 serde 失败，整个包无法识别；PCL 对 name 缺失是回退/询问。
3. hashes.sha1 与 sha512 均必填、downloads 不得为空（旧 modrinth.rs:35-44、135-138）：
   任一条目缺 sha512 或没有可用 URL 时，整个 get_download_params 返回错误。
4. get_mod_loader_type_version 在 HashMap 迭代中遇到未知 key 立即
   Err(UnsupportedModLoader)（旧 modrinth.rs:112-124），HashMap 顺序随机，可能抢先于
   已知 loader 命中，导致识别失败。
5. env.client == "unsupported" 的文件仍会被下载（旧实现完全忽略 env）；PCL 会跳过。
6. file.path 直接 join 到实例目录（旧 modrinth.rs:142），无路径穿越校验；
   overrides 用 mangled_name + 字符串前缀匹配，也无穿越防护。
7. client-overrides 从未解压（PCL 先 overrides 后 client-overrides）。
8. CurseForge 的 author/overrides 必填（旧 curseforge.rs:41-42），overrides 缺失即识别失败；
   MultiMC 必须有 instance.cfg（旧 multimc.rs:89-92），缺省即无法导入。
9. get_meta_info 会联网解析 loader branch（旧 import.rs:60-72），网络失败/版本列表不含该
   loader 时会连带整包识别失败。

## 2. 改造内容（借鉴 PCL/PCL2-CE，保持 trait 与 parser 结构）

- 格式嗅探：新增 locate_pack_file（根目录 → 一层子目录、大小写不敏感），
  CurseForge/Modrinth/MultiMC 都通过它定位 manifest，并记录 base_path 供 overrides 使用
  （对应 PCL ModModpack.vb 39-77 的 ArchiveBaseFolder 逻辑）。
- 可选字段默认值：
  - Modrinth：formatVersion=1、game=minecraft、versionId/name/files/dependencies 默认可缺省；
    name 缺失时回退 versionId → "Modrinth Modpack"；summary 可空。
  - 文件条目：hashes 整个可缺省，sha1/sha512 各自默认空；downloads/fileSize 可缺省；
    env.client/server 缺失按 required 处理。
  - CurseForge：name/author/overrides/files 可缺省，minecraft 可缺省（缺失时给出明确
    MODPACK_MANIFEST_PARSE_ERROR）；loader 优先 primary，找不到 primary 用第一个。
  - MultiMC：instance.cfg 可缺省；overrides 目录自动选择 minecraft/ 或 .minecraft/
    （PCL 同款优先规则）；name 回退 "MultiMC Modpack"。
- loader 依赖：按固定顺序匹配 forge/neoforge/neo-forge/fabric-loader/fabric/quilt-loader，
  未知 key 跳过而不是中断；loader branch 联网解析失败时回退为无 branch 的 loader，
  不再阻断整包识别（branch 本来就是 Option）。
- 文件 URL：逐条挑选第一个可解析的 http(s) URL；env.client=unsupported 跳过；
  缺 URL / 相对路径非法的条目跳过；单条坏数据不再使整个整合包导入失败。
- 路径安全：新增 safe_relative_path 拒绝绝对路径、盘符与 ".."；overrides 解压同时使用
  zip 的 enclosed_name() 与 safe_relative_path 校验解压后的相对路径（实测：仅靠
  enclosed_name + strip_prefix 仍会放过 "overrides/../escape.txt"，现已在测试中固定），
  先 overrides 后 client-overrides（后者覆盖前者）。
- 未知格式/损坏 zip：detect_manifest 先校验 zip，损坏包返回
  ZIP_FILE_PROCESS_FAILED: <zip error>；合法 zip 但无已知 manifest 返回
  MODPACK_MANIFEST_PARSE_ERROR，不会 panic。
- 未做：HMCL（modpack.json）与纯 mods 压缩包。OtherResourceSource 没有 HMCL 变体，
  加它需要改 resource/models.rs（超出本任务写作用域）；纯 mods 包无法确定 MC 版本，
  强行识别只会产生不可安装实例，故按“改动小、风险低”原则暂不支持，明确记录取舍。

## 3. 测试（src-tauri/src/instance/helpers/modpack/import.rs 内 tests 模块）

    cargo test --manifest-path src-tauri/Cargo.toml -p WTUMCL --lib

37 passed / 0 failed / 1 ignored（ignored 为外部样例探针，需 SJMCL_MODPACK_PROBE）。
覆盖：
- modrinth_sample_shape_parses_full_meta：名称/版本/MC/Forge 47.4.10 完整 meta。
- modrinth_missing_optional_fields_do_not_block_recognition：只给 minecraft 依赖也能识别。
- modrinth_missing_minecraft_version_is_a_clear_error：缺 MC 版本返回明确错误。
- modrinth_unknown_dependency_does_not_mask_known_loader：未知依赖不遮蔽 Forge。
- modrinth_nested_archive_root_and_client_overrides：多一层目录 + overrides/client-overrides
  覆盖顺序 + 实际解压结果 + "overrides/../escape.txt" 不会写出实例目录。
- modrinth_manifest_entry_is_case_insensitive：manifest 文件名大小写容错。
- modrinth_download_params_skip_unsupported_and_unsafe_entries：299 条同类条目的
  PTaskParam 生成；env.unsupported/空 downloads/"../" 被跳过；optional 保留；sha1 传递。
- malformed_known_manifest_is_a_clear_error_without_panic：已知格式但 JSON 损坏。
- unknown_archive_format_is_a_clear_error / corrupt_zip_is_a_clear_error_without_panic。
- curseforge_manifest_still_parses_with_defaulted_optional_fields /
  curseforge_manifest_in_nested_folder_parses：CurseForge 回归。
- multimc_manifest_still_parses_without_instance_cfg /
  multimc_instance_cfg_name_is_preserved：MultiMC 回归。
- safe_relative_path_rejects_escapes：路径穿越负路径。

## 4. 额外修复：Windows 下本 crate 的测试进程无法启动

本仓库 lib 测试二进制在 Windows 上原本无法启动（exit code 0xc0000139
STATUS_ENTRYPOINT_NOT_FOUND）：tauri-runtime-wry 静态导入了
comctl32!TaskDialogIndirect，而 tauri-build 只给 bin 嵌入 Common Controls v6 manifest，
测试 harness 未嵌入，加载器绑定 comctl32 v5 后找不到该导出。已在
src-tauri/src/instance/helpers/modpack/mod.rs 用 cfg(all(test, windows)) 链接
tauri-build 生成的 resource.lib（其中带有同一 manifest），只影响测试目标，不影响
应用二进制。验证：cargo test --lib 正常启动并全部通过；cargo check 0。

## 5. 已知限制 / 后续建议

- get_meta_info 仍会尝试联网补 branch；失败已降级为无 branch，但安装 Forge 时 branch
  可能为空（样例本身 branch 也是空，PCL 同样接受）。
- 未支持 HMCL / 纯 mods 包 / MCBBS 包（需扩展 OtherResourceSource 与无版本包策略）。
- 建议后续把 detect_manifest 暴露为 pub(crate) 供 MCP 工具复用，并用真实 zip fixture
  （小体积、脱敏）替代忽略测试。
