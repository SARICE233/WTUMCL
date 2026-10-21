# WTUMCL logo v2 - lattice recovery + sub-pixel fit

v2 rebuilds the WTUMCL mark from the 664x664 reference bitmap
(F:/Desktop/WTUMC-AI-VEDIO/WTUMCLOGO.png) instead of tracing it by eye.
The centre motif is the Minecraft **String** item icon, so it is
recovered on its 16x16 integer lattice; the hairlines and the hexagon are
fitted to sub-pixel 50 % intensity crossings.

check_v2.py only reads; the tools in tools/ regenerate everything.

## 1. Fitted geometry (source pixels, 664 grid)

| quantity | value | note |
|---|---|---|
| outer hexagon centre | (333.671, 332.831) | regular pointy-top hexagon |
| outer hexagon R | 312.925 | outer edge, 50 % crossing, 210 samples |
| outer hexagon RMS | 0.306 px | residual of the regular-hexagon fit |
| black field centre | (332.589, 331.804) | inner edge of the white ring |
| black field R | 225.632 | inner ring edge, 206 samples |
| black field RMS | 0.386 px | |
| inscribed hexagon R (R_insc) | 161.109 | regular hexagon whose vertical sides are the two measured hairline rails |
| hexagon rotation | 0.0 deg | locked to 0: the hairlines are exactly axis aligned, a free rotation is degenerate with R |
| edge residual (outer+field) | 0.349 px | the number check_v2 compares against 0.60 |
| hairline width w | 2.276 px | median of the 14 measured segment widths (rms 0.214) |

Hairlines (source pixels, 3 decimals):

| segment | x1 | y1 | x2 | y2 | fitted width | TLS rms |
|---|---|---|---|---|---|---|
| vcenter | 333.682 | 19.985 | 333.646 | 645.468 | 2.467 | 0.0350 |
| hmid | 137.185 | 343.561 | 527.992 | 343.561 | 2.138 | 0.0069 |
| rail_l | 193.085 | 186.714 | 193.089 | 343.561 | 2.344 | 0.0017 |
| rail_r | 472.138 | 186.740 | 472.135 | 343.561 | 2.623 | 0.0016 |
| short_l | 264.436 | 145.519 | 264.429 | 226.471 | 2.291 | 0.0039 |
| short_r | 402.862 | 146.744 | 402.859 | 226.350 | 2.797 | 0.0020 |
| cup_l | 264.429 | 226.471 | 334.261 | 266.258 | 2.082 | 0.1776 |
| cup_r | 402.859 | 226.350 | 334.261 | 266.258 | 2.047 | 0.1553 |
| x_ul | 187.877 | 343.561 | 333.564 | 427.261 | 2.082 | 0.1479 |
| x_ur | 478.975 | 343.561 | 333.564 | 427.261 | 2.138 | 0.1936 |
| x_ll | 333.085 | 426.986 | 239.549 | 465.430 | 2.262 | 0.1605 |
| x_lr | 333.868 | 427.086 | 424.320 | 464.425 | 2.324 | 0.1419 |
| basket_l | 239.549 | 465.430 | 239.553 | 590.813 | 2.123 | 0.0035 |
| basket_r | 424.320 | 464.425 | 424.317 | 593.847 | 2.403 | 0.0032 |

## 2. Centre sprite - 16x16 integer lattice

Search: s in [6, 9] step 0.01, one phase in the period step 0.05,
objective = sum of within-cell luminance variance / pixel. Hairline
pixels are excluded from the objective.

| quantity | value |
|---|---|
| lattice pitch s | 8.760 px |
| lattice origin (a grid line) | (272.150, 262.750) |
| canvas | 16x16 cells = 140.2 x 140.2 px |
| cell index range | i -1..14, j 0..15 |
| best objective (variance/pixel) | 6.526 |
| inked cells | 86 of 256 |
| palette entries | 5 (<= 6) |

The 16x16 canvas is centred on the fitted hexagon centre to within
0.2 px, which is what identifies the motif as a 16x16 item icon.

Every grid line is refined individually (sub-pixel 50 % crossing,
median along the line):

| line k | nominal x | measured dx | nominal y | measured dy |
|---|---|---|---|---|
| 0 | 263.390 | -0.560 | 262.750 | -0.027 |
| 1 | 272.150 | -0.560 | 271.510 | -0.027 |
| 2 | 280.910 | -0.129 | 280.270 | -0.410 |
| 3 | 289.670 | -0.489 | 289.030 | -0.531 |
| 4 | 298.430 | -0.492 | 297.790 | -1.008 |
| 5 | 307.190 | -0.784 | 306.550 | -0.550 |
| 6 | 315.950 | -0.446 | 315.310 | -0.839 |
| 7 | 324.710 | -1.056 | 324.070 | +0.302 |
| 8 | 333.470 | -0.628 | 332.830 | -0.247 |
| 9 | 342.230 | -0.199 | 341.590 | -0.498 |
| 10 | 350.990 | +0.384 | 350.350 | -0.749 |
| 11 | 359.750 | -0.164 | 359.110 | -0.293 |
| 12 | 368.510 | -0.102 | 367.870 | -0.624 |
| 13 | 377.270 | -0.665 | 376.630 | -0.192 |
| 14 | 386.030 | -0.172 | 385.390 | -0.898 |
| 15 | 394.790 | -0.172 | 394.150 | -0.898 |
| 16 | 403.550 | -0.172 | 402.910 | -0.898 |

### Palette

| entry | hex | rgb | cells |
|---|---|---|---|
| P0 | #F8FDFE | 248, 253, 254 | 39 |
| P1 | #B4CFD1 | 180, 207, 209 | 15 |
| P2 | #9CBABB | 156, 186, 187 | 21 |
| P3 | #6B6E6F | 107, 110, 111 | 2 |
| P4 | #363D3D | 54, 61, 61 | 9 |

Every cell of the 16x16 canvas is listed below, so the recovery can be
re-checked cell by cell. bg = background (black field or hole); ---- =
cell centre outside the bitmap.

| j\i | -1 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | bg | bg | bg | bg | bg | bg | bg | bg | #363D3D | #F8FDFE | bg | bg | bg | bg | bg | bg |
| 1 | bg | bg | bg | bg | bg | bg | bg | bg | #F8FDFE | #9CBABB | #F8FDFE | bg | bg | bg | bg | bg |
| 2 | bg | bg | bg | bg | bg | bg | bg | bg | #F8FDFE | bg | #9CBABB | bg | bg | bg | bg | bg |
| 3 | bg | bg | bg | bg | bg | bg | bg | bg | #9CBABB | #F8FDFE | #F8FDFE | bg | bg | bg | bg | bg |
| 4 | bg | bg | bg | #F8FDFE | bg | bg | bg | bg | bg | #9CBABB | #9CBABB | #F8FDFE | bg | bg | bg | bg |
| 5 | bg | bg | #F8FDFE | #9CBABB | bg | bg | bg | bg | bg | bg | bg | #9CBABB | #F8FDFE | bg | bg | bg |
| 6 | bg | #F8FDFE | #9CBABB | bg | bg | #F8FDFE | #F8FDFE | #F8FDFE | #F8FDFE | bg | bg | bg | #9CBABB | #F8FDFE | bg | bg |
| 7 | bg | #F8FDFE | bg | bg | #F8FDFE | #B4CFD1 | #B4CFD1 | #B4CFD1 | #B4CFD1 | #F8FDFE | #F8FDFE | bg | bg | #F8FDFE | bg | bg |
| 8 | bg | #9CBABB | #F8FDFE | bg | #B4CFD1 | #F8FDFE | bg | bg | bg | #B4CFD1 | #B4CFD1 | #F8FDFE | bg | #F8FDFE | bg | bg |
| 9 | #363D3D | #363D3D | #9CBABB | #F8FDFE | #F8FDFE | #B4CFD1 | #B4CFD1 | #363D3D | #6B6E6F | #363D3D | #F8FDFE | #B4CFD1 | #363D3D | #F8FDFE | #363D3D | #363D3D |
| 10 | bg | bg | bg | #9CBABB | #9CBABB | #F8FDFE | #F8FDFE | #F8FDFE | #F8FDFE | #F8FDFE | #B4CFD1 | bg | #F8FDFE | #9CBABB | bg | bg |
| 11 | bg | bg | bg | bg | bg | #9CBABB | #B4CFD1 | #B4CFD1 | #B4CFD1 | #B4CFD1 | bg | #F8FDFE | #9CBABB | bg | bg | bg |
| 12 | bg | bg | bg | bg | bg | bg | bg | #9CBABB | #F8FDFE | #F8FDFE | #F8FDFE | #9CBABB | bg | bg | bg | bg |
| 13 | bg | bg | bg | bg | bg | bg | bg | bg | #9CBABB | #9CBABB | #9CBABB | bg | bg | bg | bg | bg |
| 14 | #6B6E6F | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | #363D3D |
| 15 | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg | bg |

## 3. Word mark

WTU and MC are white on black and axis aligned. They were run through
the same lattice search: the variance objective has no sharp minimum
there (the letters are spaced proportionally, not on a common lattice),
so the production vector uses a band/run decomposition of the 50 %
intensity mask with every rectangle edge refined to the measured
crossing. The lattice result is still recorded in data/lattice.json
(blocks.wtu, blocks.mc).

| block | rectangles |
|---|---|
| mc | 30 |
| wtu | 37 |

## 4. Colours

The reference bitmap is not pure black / pure white: inside the artwork
the field measures RGB(0, 8, 5) (median of the dark pixels) and the
whites measure RGB(248, 253, 254). The v2 master uses those measured
levels so the export sits on the source's own noise floor; visually they
are still black and white.

## 5. v2 vs v1

| | v1 | v2 |
|---|---|---|
| centre motif | 40x40 samples at pitch 3.4, nearest colour, no lattice solve | 16x16 integer lattice, s=8.760, per-line sub-pixel refinement, 5-colour palette |
| motif geometry | cells 3.4 px wide drifting off the real blocks | every cell edge on the measured 50 % crossing (line dx -1.06..+0.38 px from uniform) |
| word mark | row bands, integer edges, threshold 140 | row bands, sub-pixel edges, block-level 50 % level |
| hexagon | hand measured (312.3 / 226.9), single centre | 50 %-crossing fit, 210 + 206 samples, RMS 0.306 / 0.386 px |
| hairlines | fixed 2.0 px, hand model endpoints | TLS per segment, width 2.276 px, junctions by intersection |
| colours | #000000 / #FFFFFF / #B8CFD0 | measured ink #000805, white #F8FDFE, sprite palette from k-means on cell medians |
| verification | mask IoU only (min 0.85), no alignment | global IoU, centre IoU + MAE, word mark IoU, hexagon RMS, SVG self-render, all after sub-pixel alignment |

## 6. Verification

```
WTUMCL logo v2 acceptance check -- F:\Desktop\SJMCL-main\design\wtumcl-logo\v2
--------------------------------------------------------------------------------
      source      F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png
      alignment   shift=(2.3, 1.4) px @664 (integer seed (2, 1))
--------------------------------------------------------------------------------
PASS  SVG master parses / viewBox / no bitmap        12829 bytes, 169 elements, viewBox ok, 3.000 decimals
PASS  SVG inverse master                             12797 bytes, 169 elements
PASS  PNG 1024 x 1024 RGBA, corners transparent      1024x1024 RGBA, corners alpha=0
PASS  PNG 2048 x 2048 RGBA, corners transparent      2048x2048 RGBA, corners alpha=0
PASS  PNG 4096 x 4096 RGBA, corners transparent      4096x4096 RGBA, corners alpha=0
PASS  PNG inverse 1024 RGBA                          1024x1024 RGBA, corners alpha=0
PASS  Global foreground IoU (aligned)                IoU=0.9693 (min 0.95) shift=(2.3,1.4)
PASS  Centre sprite IoU                              IoU=0.9369 (min 0.90)
PASS  Centre sprite MAE                              MAE=7.940/255 (max 8.0)
PASS  Wordmark IoU                                   IoU=0.9200 (min 0.90)
PASS  Hexagon edge residual RMS                      RMS=0.482 px (max 0.60, n=246)
PASS  SVG self-render 4096->1024 vs 1024             MAE=0.13392/255 (max 1.00000)
--------------------------------------------------------------------------------
12/12 passed
```

## 7. Rebuild

```
python design/wtumcl-logo/v2/tools/fit_geometry.py    # -> data/geometry.json
python design/wtumcl-logo/v2/tools/fit_lattice.py     # -> data/lattice.json (full search, ~6 min)
python design/wtumcl-logo/v2/tools/fit_wordmark.py    # -> data/wordmark.json
python design/wtumcl-logo/v2/tools/build_svg.py       # -> master/*.svg
python design/wtumcl-logo/v2/tools/render.py          # -> png/*.png
python design/wtumcl-logo/v2/tools/make_previews.py   # -> preview/*.png
python design/wtumcl-logo/v2/tools/make_readme.py     # -> README.md
python design/wtumcl-logo/v2/check_v2.py              # read-only validation
```

fit_lattice.py --fast uses a 0.25 phase step for a quick look; the
committed data come from the full 0.05 step run.

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
