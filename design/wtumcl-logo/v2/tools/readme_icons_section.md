---

## 8. 图标套件与替换指引（icon kit / replacement guide）

这一节把 v2 母版做成一套可直接打包的应用图标：九个标准尺寸 PNG、一个多帧 ICO、16/24 的整像素
光学版，以及可肉眼复核的对照页。全部产物由 `tools/make_icons.py` 与 `tools/make_compare.py`
从 `png/WTUMCL-logo-4096.png` + `data/*.json`（几何模型）生成，**本节只新增 `v2/` 下的文件，
没有替换 `public/`、`src-tauri/` 与 v1 目录中的任何现存资源**（替换步骤见 8.5，仅指引）。

### 8.1 文件清单（括号内为实测字节数）

| 文件 | 尺寸 | 字节 | alpha>0 像素 | 说明 |
|---|---|---|---|---|
| `icons/16.png` | 16×16 | 606 | 192 | RGBA 透明底，等比居中，四角 alpha=0 |
| `icons/24.png` | 24×24 | 1 049 | 424 | 同上 |
| `icons/32.png` | 32×32 | 1 576 | 740 | 同上 |
| `icons/48.png` | 48×48 | 2 763 | 1 532 | 同上 |
| `icons/64.png` | 64×64 | 3 891 | 2 652 | 同上 |
| `icons/128.png` | 128×128 | 9 959 | 10 024 | 同上 |
| `icons/256.png` | 256×256 | 20 863 | 38 894 | 同上 |
| `icons/512.png` | 512×512 | 42 067 | 153 671 | 同上 |
| `icons/1024.png` | 1024×1024 | 87 344 | 609 877 | 同上（应用图标首选源） |
| `icons/WTUMCL.ico` | 多尺寸 | 40 825 | — | 7 帧：16 / 24 / 32 / 48 / 64 / 128 / 256，每帧 PNG 压缩 |
| `icons/optical/16.png` | 16×16 | 229 | 156（全不透明） | 整像素光学版，见 8.3 |
| `icons/optical/24.png` | 24×24 | 298 | 338（全不透明） | 整像素光学版，见 8.3 |
| `compare/compare.png` | 2208×908 | 403 465 | — | 左：源图 664 ／ 中：v2 重绘 @664 ／ 右：50% 叠加 |
| `compare/diff.png` | 1500×930 | 291 624 | — | 灰底差异热图（×3 增强）+ 指标行 |
| `compare/icon-sheet.png` | 2088×1476 | 159 653 | — | 16..256 实尺预览（深/浅底板）+ 16/24 的 8× 放大 |
| `compare/optical-8x.png` | 1384×1126 | 63 864 | — | 16/24 的"标准版 vs 光学版"12× 放大对照 |
| `tools/make_icons.py` | — | — | — | 生成 `icons/`（九个 PNG + ICO + optical） |
| `tools/make_compare.py` | — | — | — | 生成 `compare/` 四张对照页 |
| `check_icons.py` | — | — | — | 只读校验（8.4） |

### 8.2 尺寸表与导出规则

| 尺寸 | 来源 | 重采样 | 用途 |
|---|---|---|---|
| 16 / 24 | 4096 母版 | 预乘 alpha Lanczos | favicon、标题栏、托盘 |
| 32 / 48 / 64 | 同上 | 同上 | 任务栏、文件列表、Dock 备选 |
| 128 / 256 | 同上 | 同上 | 应用列表、商店图 |
| 512 / 1024 | 同上 | 同上 | 安装包、关于页、图标母版 |
| ICO 帧 16..256 | 上述同名 PNG 直接封装 | 无二次缩放 | Windows 可执行文件/快捷方式 |

规则：所有 PNG 为正方形 RGBA、透明底、等比居中、不裁切；下采样一律走**预乘 alpha** 再
unpremultiply，避免半透明边像素吸入透明区的黑色而出现暗边。ICO 每帧是 PNG 压缩块，
目录项宽高与帧实际尺寸一致（256 在头里写 0）。九档 PNG 与"对 4096 做预乘 alpha Lanczos
下采样"的结果逐像素一致（MAE = 0.00/255，见 8.4）。

### 8.3 小尺寸光学版（`icons/optical/`）

16 / 24 px 时一条 2.276 源像素宽的发丝线只占 0.05–0.08 px，Lanczos 版会把整个线网摊成灰色
薄雾（`compare/optical-8x.png` 左列可见）。光学版不再对母版重采样，而是把同一份几何模型
**直接画在整像素网格上**：

1. 每个形状用 8× 超采样覆盖率、以 50% 为阈值二值化——每个输出像素要么全透明，要么是
   一个不透明的调色板颜色，没有小数边、没有灰边。
2. 发丝线按**整像素最小可视线宽（1 px）**绘制，不再出现亚像素细线；线网按尺寸做等量简化：
   16 px 保留两条主轴（`vcenter`、`hmid`，占黑内区 14.3%），24 px 再加左右导轨与
   提篮轮廓（`rail_l/rail_r`、`basket_l/basket_r`，占 16.2%）。1 px 线在 16 px 图标里
   已占 1/16，全画 14 条会连成白团，所以对称成组地按尺寸递增。
3. 中央 16×16 线团（Minecraft 线/String 图标）在 16/24 下每格不足 1 px，改为"覆盖率加权
   平均色 → 最近调色板色"取值，保持像素实色而不产生混色灰。

光学版只在 16/24 提供；32 px 及以上仍用标准 Lanczos 版（`icons/32.png` 起）。

### 8.4 校验

```
python design/wtumcl-logo/v2/check_icons.py
```

只读脚本，逐项 PASS/FAIL 并打印实测值，全部通过才 exit 0（当前 19/19 通过）。检查项：
九档 PNG 尺寸/alpha/四角透明；ICO ≥6 帧且含 256、每帧可解码带 alpha；optical 16/24 存在、
尺寸正确且 alpha 为 0/255 二值；`compare.png` 宽 ≥1600 且三栏互不相同；`diff.png` 为灰底热图；
`icon-sheet.png` 同时含深色板与浅色板；九档 PNG 与"对 4096 重算的预乘 alpha Lanczos 下采样"
逐像素一致（MAE ≤ 1/255，实测 0.00）。

### 8.5 替换回应用（**仅指引，本次未执行**）

应用里现存的目标文件（尺寸已核对）：

| 目标文件（现有尺寸） | 建议来源（v2） |
|---|---|
| `public/images/icons/Logo_128x128.png`（128×128） | `design/wtumcl-logo/v2/icons/128.png` |
| `public/images/icons/Logo_32x32.png`（32×32） | `design/wtumcl-logo/v2/icons/32.png`（光学版只提供 16/24 两档） |
| `src-tauri/assets/icons/icon.png`（512×512） | **反相 512**：由 `png/WTUMCL-logo-inverse-1024.png` 预乘 alpha Lanczos 缩到 512（正稿白框在白底会消失，故 Tauri 主图标用反相稿） |
| `src-tauri/assets/icons/32x32.png`（32×32） | `icons/32.png` |
| `src-tauri/assets/icons/64x64.png`（64×64） | `icons/64.png` |
| `src-tauri/assets/icons/128x128.png`（128×128） | `icons/128.png` |
| `src-tauri/assets/icons/128x128@2x.png`（256×256） | `icons/256.png` |
| `src-tauri/assets/icons/icon.ico`（多帧） | `icons/WTUMCL.ico`（7 帧，含 256） |
| `src-tauri/assets/icons/icon.icns`（macOS） | 本套不产出，需 `npx tauri icon` 从 1024 母版重生成 |

步骤（示意，PowerShell；执行前请自行核对目标路径并先备份）：

1. 备份：`Copy-Item src-tauri\assets\icons src-tauri\assets\icons.bak -Recurse`。
2. 网页侧：`Copy-Item design\wtumcl-logo\v2\icons\128.png public\images\icons\Logo_128x128.png -Force`；
   `Copy-Item design\wtumcl-logo\v2\icons\32.png public\images\icons\Logo_32x32.png -Force`。
3. Tauri 侧：`Copy-Item design\wtumcl-logo\v2\icons\64.png src-tauri\assets\icons\64x64.png -Force`、
   `... 256.png ... 128x128@2x.png`、`... WTUMCL.ico ... icon.ico`；
   `icon.png` 用反相 512（见上表）。
4. 重新构建并肉眼确认（`pnpm tauri build`）；回滚时把 `icons.bak` 拷回。

**白底注意**：本套图标是"白色六边形外框 + 透明底"。放到纯白背景上时白色外框会与背景融为一体，
只看到内部黑色六边形；白底/浅色主题请改用反相色稿（`master/WTUMCL-logo-inverse.svg`、
`png/WTUMCL-logo-inverse-1024.png`）。

### 8.6 重建

```
python design/wtumcl-logo/v2/tools/make_icons.py      # -> icons/ + icons/optical/
python design/wtumcl-logo/v2/tools/make_compare.py    # -> compare/*.png
python design/wtumcl-logo/v2/check_icons.py           # 只读校验
```

`make_readme.py` 会在重建 README 时自动把本节（`tools/readme_icons_section.md`）追加到文末。
