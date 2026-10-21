# WTUMCL Logo — 矢量母版与高清套件

从 `F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png`（664×664 位图）重建的可缩放矢量母版，
以及由同一份几何模型导出的 1024 / 2048 / 4096 透明底 RGBA PNG、反相色稿和对比验证材料。

**只读校验命令（唯一需要执行的命令）：**

```
python design/wtumcl-logo/check.py
```

当前结果：**10/10 PASS, exit 0**。

---

## 1. 文件树

```
design/wtumcl-logo/
├─ check.py                       # 只读验收脚本（python + Pillow），逐项 PASS/FAIL
├─ README.md                      # 本文件
├─ master/
│  ├─ WTUMCL-logo.svg             # 正稿矢量母版，viewBox="0 0 1024 1024"
│  └─ WTUMCL-logo-inverse.svg     # 反相色稿矢量母版（白底 / 黑六边形 / 白内区）
├─ png/
│  ├─ WTUMCL-logo-1024.png        # RGBA 透明底
│  ├─ WTUMCL-logo-2048.png
│  ├─ WTUMCL-logo-4096.png
│  └─ WTUMCL-logo-inverse-1024.png # 反相色稿，纯白不透明底
├─ preview/
│  ├─ comparison-source-vs-redraw.png  # 源位图 vs 重绘 并排
│  ├─ mascot-detail.png                # 中心像素图案放大对比
│  └─ colorways.png                     # 正稿 / 反相 并排
├─ reference/
│  └─ WTUMCLOGO-reference.png      # 源位图副本（664×664），供 check.py 离线比对
└─ tools/
   ├─ extract_source.py            # 从源位图抽取像素数据（一次性，可选）
   ├─ pixeldata.json               # 抽取结果（吉祥物像素格 + 字标矩形），已提交
   ├─ logo_model.py                # 唯一几何/配色模型（SVG 与 PNG 共用）
   ├─ build.py                     # 模型 -> 两个 SVG
   ├─ render.py                    # 模型 -> 1024/2048/4096 PNG（+ 反相 1024）
   └─ make_previews.py             # 生成 preview/ 对比图
```

未改动 `public/` 与 `src-tauri/` 中任何现有文件；本目录之外只新增文件。

---

## 2. 几何参数表（源图测量值 vs 重绘取值）

源图坐标系：664×664，六边形中心实测在 **(333.5, 333)**（并非画布正中心 331.5,331.5）。
重绘时统一居中到 1024 画布中心 **(512, 512)**，缩放系数 `K = 1024/664 = 1.542169`。

| 项目 | 源图测量（664 坐标系） | 测量方法 | 重绘取值（1024 画布） |
|---|---|---|---|
| 外六边形外接圆半径 R | **312.3** | 亮部 bbox y 21..645（高 624.6 = 2R）；赤道半宽 270.5 = √3R/2 | **481.7**（半宽 417.1） |
| 外六边形宽 × 高 | 541 × 624.6 | bbox x 63..604 / y 21..645 | 834.1 × 963.2（实测渲染 bbox 836×964） |
| 朝向 | pointy-top 正六边形 | 顶点 (333.5,21) 与 (333.5,645)，上下左右对称 | 同（顶点朝上） |
| 内黑六边形外接圆半径 r | **226.9** | 赤道半宽 196.5 = √3r/2；y=150 处半宽 75.9 = √3(r−183) | **349.9**（半宽 303.0） |
| 白框厚度（径向） | **85.4**（= R − r） | 顶部 21 → 106.1 | **131.7** |
| 白框厚度（赤道水平） | **73.95**（= 270.45 − 196.5） | y=332 行白框 63..136 | **114.0**（check.py 实测 115.0，误差 0.83%） |
| 发丝线宽 | ≈ 2.0 px | 中心线 333–334、竖线 264–265 等 | **3.08**（= 2·K） |
| 中心竖脊 | x=333.5，y 21..645 | 整列亮像素 | x=512，全长 |
| 水平横线 | y = 343.5，x 137..530 | 多列出现 343–344 | y = **528.2**，x 从 297.6 到 726.4（内六边形左右边） |
| 长竖线 ×2 | x = 194.5 / 472.5，y 186.4..343.5 | 行游程 193–194 / 471–472 | x = **297.6 / 726.4**，起于内六边形斜边 |
| 吊灯短竖线 ×2 | x = 264.5 / 402.5，y 145.9..228 | 行游程 264–265 / 402–404 | x = **405.6 / 618.4**，y 224.9..350.1 |
| 吊灯斜线 | (264.5,228)→(333.5,262) 及镜像 | Hough + 连通域端点 | 同（对称化，汇于 y=402.5） |
| 上部 X 斜线 | (194.5,343.5)→(333.5,423.8) 及镜像，斜率 tan30° | 逐行游程拟合 | 同 |
| 下部浅斜线 | (333.5,423.8)→(241.5,465) 及镜像 | 逐行游程拟合 | 交点 y=652.0，端点 y=715.6 |
| 篮筐短竖线 ×2 | x = 241.5 / 425.5，y 465..502 | 行游程 239–240 / 424–425 | x = **370.1 / 653.9**，y 715.6..772.7 |
| 吉祥物 bbox | 272..386 × 272..387（115×116） | 浅蓝像素 bbox | 177×179 |
| 吉祥物像素格 | **3.4 px**（MSE 扫描在 8.8 / 4.4 / 3.4 处出现极小值） | 格子方差极小化 | **5.24**（= 3.4·K），共 521 格 → 合并为横向游程 |
| 字标第一行 WTU | x 286..381，y 454..483（高 30），笔画宽 9 | 逐行游程 | 高 46.3，笔画 13.9 |
| 字标第二行 MC | x 302..364，y 495..525（高 31） | 逐行游程 | 高 47.8 |
| 字标重建方式 | — | 阈值 140 的行游程 → 相邻同游程合并成矩形（79 个 rect） | 79 rect |

> 说明：源图是带辉光（glow）的光栅图，所有边界都做了 ±1 px 的阈值判定；表中数值为多行/多列拟合后再对称化的结果，
> 与原始位图逐像素相比误差 ≤ 1.5 px（折算到 1024 画布 ≤ 2.3 px）。

---

## 3. 配色表

| 角色 | Hex | 来源 |
|---|---|---|
| 墨黑（内区 / 反相里的六边形与线） | `#000000` | 源图背景实测 (0,0,0) |
| 白（外框、发丝线、字标、吉祥物亮部） | `#FFFFFF` | 源图实测 (255,255,255)/(254,254,254) |
| 浅蓝强调色（吉祥物主体） | `#B8CFD0` | 源图浅蓝像素均值 (184,207,208) |
| 深蓝强调色（**仅反相稿**使用） | `#6E9AA0` | 同色系派生，保证白底上的可读性 |

正稿只用了 **黑 / 白 / 浅蓝** 三色；反相稿额外引入一个同色系深蓝，仍属“1–2 个浅蓝强调色”范围。
源图里的辉光渐变被有意舍弃（矢量化时不需要），这也是 IoU 未达 0.95+ 的主要原因之一。

---

## 4. 生成步骤（可复跑）

```bash
# 1) 抽取源图像素数据（可选：pixeldata.json 已提交，跳过后结果一致）
python design/wtumcl-logo/tools/extract_source.py

# 2) 由同一模型生成两个 SVG 母版
python design/wtumcl-logo/tools/build.py

# 3) 生成 1024 / 2048 / 4096 透明底 PNG + 反相 1024 PNG
python design/wtumcl-logo/tools/render.py

# 4) 生成 preview/ 对比图
python design/wtumcl-logo/tools/make_previews.py

# 5) 只读验收
python design/wtumcl-logo/check.py
```

**渲染原理**：`logo_model.build()` 返回 z 序的 `(颜色键, [多边形...])` 列表，
`build.py` 直接把多边形写成 `<path>` / `<rect>`；`render.py` 把同一批多边形按
`目标尺寸 × 超采样系数`（1024→4×、2048→2×、4096→1.5×）画进 8bit 覆盖遮罩，
再用 Lanczos 缩到目标尺寸、按 z 序做直通 alpha 合成。因此 SVG 与 PNG 的几何**必然一致**。
已用 Chromium（`playwright screenshot`）单独渲染 SVG 复核：与 PNG 掩膜 IoU = 0.997，平均像素差 0.42/255。

---

## 5. 两套色稿的用法

| 场景 | 用哪套 | 说明 |
|---|---|---|
| 深色 UI / 黑底横幅 / 启动图 | `master\WTUMCL-logo.svg` + `png/WTUMCL-logo-*.png` | 透明底，白框黑内区，浅蓝吉祥物 |
| 白底文档 / 浅色主题 / 打印 | `master\WTUMCL-logo-inverse.svg` + `png/WTUMCL-logo-inverse-1024.png` | 纯白不透明底，黑色六边形环 + 白色内区 + 黑线 |
| 应用图标（src-tauri / 网页 favicon） | 正稿（透明底） | 与现有 `src-tauri/assets/icons/icon.png` 的观感一致：透明底 + 白框 + 黑内区 |

反相稿的“反相”指**明暗关系翻转**：正稿是“黑底白图”，反相稿是“白底黑图”。
正稿中心区域平均亮度 ≈ 49（黑内区），反相稿中心区域平均亮度 ≈ 233（白内区），差值 183 —— check.py 会验证这一点。

---

## 6. 校验项（check.py）

| # | 检查 | 阈值 | 当前值 |
|---|---|---|---|
| 1 | SVG 存在 / 可解析 / viewBox / 元素数 / 体积 | = `0 0 1024 1024`，≥8 元素，≤500 KB | 14 946 B，230 元素 |
| 2 | 反相 SVG | 同上 | 232 元素 |
| 3–5 | 1024 / 2048 / 4096 PNG | 尺寸精确、含 alpha、四角 alpha=0 | 全部通过 |
| 6 | 结构一致性 | 1024→664 后 `alpha>16 且亮度>160` 掩膜 vs 源图 `亮度>160` 掩膜，IoU ≥ 0.85 | **0.913** |
| 7 | pointy-top 正六边形比例 | 前景 bbox 宽/高 ≈ √3/2 ±3% | 836×964 → 0.8672（理论 0.8660） |
| 8 | 白框厚度 | 赤道水平白框 vs 测量值 114.0，误差 ≤3% | 115.0（0.83%） |
| 9 | 反相色稿存在且中心明暗相反 | 中心区域亮度差 > 40 | 49.4 → 232.8（+183.5） |
| 10 | 中心图案/字标存在性 | 中心区域（x 0.36–0.64, y 0.36–0.62）浅色/彩色像素 > 500 | 16 695 |

check.py 全程只读；缺文件、尺寸不符、SVG 损坏都会打印 FAIL 并返回 exit 1，不会抛 traceback。

---

## 7. 替换回应用时的目标文件清单（**仅指引，本次未执行**）

本仓库当前存在的图标资源（已核对）：

| 目标文件 | 当前尺寸 | 建议来源 / 处理 |
|---|---|---|
| `public/images/icons/Logo_128x128.png` | 128×128 RGBA 透明底 | `png/WTUMCL-logo-1024.png` 缩放到 128（Lanczos） |
| `public/images/icons/Logo_32x32.png` | 32×32 RGBA 透明底 | 同上缩放到 32（小尺寸建议关掉发丝线或加粗到 ≥1 px，否则 0.1 px 线宽会消失） |
| `src-tauri/assets/icons/icon.png` | 512×512 RGBA 透明底 | `png/WTUMCL-logo-1024.png` 缩放到 512 |
| `src-tauri/assets/icons/32x32.png` | 32×32 | 同上缩放到 32 |
| `src-tauri/assets/icons/64x64.png` | 64×64 | 缩放到 64 |
| `src-tauri/assets/icons/128x128.png` | 128×128 | 缩放到 128 |
| `src-tauri/assets/icons/128x128@2x.png` | 256×256 | 缩放到 256 |
| `src-tauri/assets/icons/icon.ico` | 多尺寸 ICO | **不要手改**：用 `npx tauri icon design/wtumcl-logo/png/WTUMCL-logo-1024.png` 统一重生成 |
| `src-tauri/assets/icons/icon.icns` | macOS ICNS | 同上由 `tauri icon` 生成 |

替换前请确认：源图透明底（四角 alpha=0）在浅色背景上会露出白色外框；若目标场景是白底，
改用 `png/WTUMCL-logo-inverse-1024.png`，或先用 `master/WTUMCL-logo-inverse.svg` 重新导出所需尺寸。

---

## 8. 已知偏差与设计取舍

1. **居中**：源图图形中心在 (333.5, 333)，比 664 画布中心右移 2 px、下移 1.5 px；重绘统一居中到 (512,512)。
   这会让 IoU 从理论上的 ~0.99 掉到 0.913，但成品四边留白对称，更适合做图标母版。
2. **吉祥物**：按 3.4 px 像素格重建（521 格 → 合并为横向游程），保留像素风与可缩放性，
   但源图的辉光过渡和 1–2 px 的碎细节被抹平。
3. **字标**：按阈值 140 的行游程重建为 79 个矩形，笔画比源图略粗约 1 px（阈值把抗锯齿边缘也算进去了）。
4. **反相稿语义**：反相稿取“明暗翻转”（白底黑图），而不是把正稿的每个颜色直接取反 255−v；
   目的是保证白底场景下的可读性与描边完整。

---

## 9. 图标套件（`icons/` 与 `compare/`）

这一节是新增的“可打包”交付层：把矢量母版的高清渲染 `png/WTUMCL-logo-4096.png` 导出为九个标准尺寸的
应用图标 PNG + 一个多帧 ICO，并给出可肉眼复核的源图 / 重绘 / 叠加 / 差异材料。
**本节只新增文件，没有替换 `public/`、`src-tauri/` 下任何现有资源**（替换步骤见 9.6，仅指引）。

### 9.1 文件清单

| 文件 | 尺寸 | 字节 | 非透明像素 | 说明 |
|---|---|---|---|---|
| `icons/16.png` | 16×16 | 565 | 192 | RGBA 透明底，等比居中，未裁切 |
| `icons/24.png` | 24×24 | 926 | 424 | 同上 |
| `icons/32.png` | 32×32 | 1 340 | 736 | 同上 |
| `icons/48.png` | 48×48 | 2 312 | 1 528 | 同上 |
| `icons/64.png` | 64×64 | 3 176 | 2 648 | 同上 |
| `icons/128.png` | 128×128 | 7 669 | 9 996 | 同上 |
| `icons/256.png` | 256×256 | 17 511 | 38 788 | 同上 |
| `icons/512.png` | 512×512 | 35 875 | 152 766 | 同上 |
| `icons/1024.png` | 1024×1024 | 71 849 | 607 250 | 同上（应用图标首选源） |
| `icons/WTUMCL.ico` | 多尺寸 | 33 617 | — | 7 帧：16 / 24 / 32 / 48 / 64 / 128 / 256，每帧 PNG 压缩 |
| `compare/compare.png` | 2208×908 | 390 105 | — | 左源图 / 中重绘 / 右 50% 叠加，带文字标签 |
| `compare/diff.png` | 1500×930 | 299 403 | — | 灰底差异热图（x3 增强）+ 指标行 |
| `compare/icon-sheet.png` | 2088×1476 | 172 383 | — | 九个尺寸在深/浅背景上的实尺展示 + 16/24/32 的 8× 最近邻放大 |
| `tools/make_icons.py` | — | — | — | 生成 `icons/`（PNG + ICO） |
| `tools/make_compare.py` | — | — | — | 生成 `compare/` 三张图 |
| `check-icons.py` | — | — | — | 图标套件只读验收（见 9.5） |

### 9.2 尺寸表与导出规格

| 项目 | 取值 |
|---|---|
| 源 | `png/WTUMCL-logo-4096.png`（RGBA，矢量母版渲染） |
| 尺寸集合 | 16, 24, 32, 48, 64, 128, 256, 512, 1024（九个） |
| 重采样 | **预乘 alpha 的 Lanczos**（先 `RGB×A`，Lanczos 缩放到目标尺寸，再除以 alpha 还原直通 alpha） |
| 几何 | 等比缩放、居中、不裁切、不拉伸；输出始终为正方形、RGBA |
| 透明底 | 四角 alpha = 0（`check-icons.py` 逐张验证） |
| ICO 帧 | 16 / 24 / 32 / 48 / 64 / 128 / 256（Windows 单帧上限 256，故 512/1024 不进入 ICO） |

**为什么必须预乘**：源图是“白色外框 + 黑色内区 + 透明外部”。直接把 RGBA 交给 Lanczos，会在透明边缘把
未定义颜色的黑像素混进来，小尺寸外框会出现灰黑色毛边。预乘后边缘只剩“白 × 覆盖度”，16/24 px 的六边形
外框边缘干净、不脏。

**小尺寸可辨识性**：16 / 24 / 32 px 下 pointy-top 六边形轮廓完整可辨；2 px 发丝线（1024 画布上的 3.08 px）
在这些尺寸下必然落到 1 px 以下而被有意丢弃，这是图标化时的正常取舍。可看 `compare/icon-sheet.png`
第 3/4 行的 8× 最近邻放大自行判断边缘质量。

### 9.3 ICO

`icons/WTUMCL.ico` 为手写的 ICONDIR + 7 个 PNG 压缩帧（非 BMP 帧），每帧由对应的九个 PNG 中的
同尺寸文件写入，未做二次缩放。`check-icons.py` 既解析 ICONDIR 头，又用 Pillow 逐帧解码核对尺寸。

### 9.4 对比材料

- `compare/compare.png`（2208 px 宽）：三个 664 px 面板同尺度并排 ——
  **Left: source**（`reference/WTUMCLOGO-reference.png`，664×664 不透明黑底）、
  **Center: redraw**（`png/WTUMCL-logo-4096.png` 经同一套预乘 Lanczos 缩到 664，再合成到黑底以对齐源图背景）、
  **Right: overlay**（两者 50% 混合）。顶部与每栏都有文字标签，底部注明面板同尺度与居中偏移说明。
- `compare/diff.png`（1500 px 宽）：逐像素 `mean |ΔRGB|` 放大 3×，灰底（RGB 90）为“一致”，
  黄→红为“不一致”；当前指标 **mean |diff| = 8.98/255，|diff| > 8 的像素占 8.68%**。
  热图里最亮的红边来自三处已知偏差：整体居中平移（源图中心 333.5,333 → 画布中心）、
  源图辉光被矢量化舍弃、字标笔画比源图粗约 1 px。
- `compare/icon-sheet.png`：第 1/2 行为原生尺寸实尺展示（深色 / 浅色对比底板），
  第 3/4 行为 16/24/32 的 8× 最近邻放大，用来判断边缘是否干净。

### 9.5 只读校验

```
python design/wtumcl-logo/check-icons.py
```

当前结果：**14/14 PASS，exit 0**。逐项内容：

| # | 检查 | 阈值 / 期望 |
|---|---|---|
| 1 | `icons/` 存在 | 目录可读 |
| 2–10 | 九个 PNG | 文件名 `<size>.png`、尺寸精确等于 N×N、含 alpha、四角 alpha = 0、非全透明、存在不透明像素 |
| 11 | 目录洁净 | PNG 恰好九个，无多余文件 |
| 12 | ICO | ICONDIR 合法、≥6 帧、含 256、帧为正方形 PNG 载荷、逐帧可解码 |
| 13 | `compare.png` | 宽度 ≥ 1600、三等分区域各有 ≥3000 亮像素、两两平均差 ≥ 2.0（证明确实是三个不同面板） |
| 14 | `diff.png` | 存在、尺寸合理、≥2000 个暖色高亮像素、≥20% 中性灰底 |

脚本只读，缺文件/尺寸不符/ICO 损坏都会打印 FAIL 并返回 exit 1，不抛 traceback（已用删文件的临时副本验证）。

### 9.6 替换回应用（**仅指引，本次未执行**）

目标文件与建议来源（已核对现有文件的尺寸，全部为 RGBA）：

| 目标文件（现有尺寸） | 建议来源 |
|---|---|
| `public/images/icons/Logo_128x128.png`（128×128） | `design/wtumcl-logo/icons/128.png` |
| `public/images/icons/Logo_32x32.png`（32×32） | `design/wtumcl-logo/icons/32.png` |
| `src-tauri/assets/icons/icon.png`（512×512） | `design/wtumcl-logo/icons/512.png` |
| `src-tauri/assets/icons/32x32.png`（32×32） | `design/wtumcl-logo/icons/32.png` |
| `src-tauri/assets/icons/64x64.png`（64×64） | `design/wtumcl-logo/icons/64.png` |
| `src-tauri/assets/icons/128x128.png`（128×128） | `design/wtumcl-logo/icons/128.png` |
| `src-tauri/assets/icons/128x128@2x.png`（256×256） | `design/wtumcl-logo/icons/256.png` |
| `src-tauri/assets/icons/icon.ico`（现有多帧 ICO） | `design/wtumcl-logo/icons/WTUMCL.ico`（7 帧）或 `npx tauri icon design/wtumcl-logo/icons/1024.png` 统一重生成 |
| `src-tauri/assets/icons/icon.icns`（macOS） | 只能由 `npx tauri icon design/wtumcl-logo/icons/1024.png` 生成（本项目不产出 .icns） |

步骤（示意命令，PowerShell；执行前请自行确认目标路径）：

1. 备份现有图标目录：`Copy-Item src-tauri\assets\icons src-tauri\assets\icons.bak -Recurse`。
2. 逐项复制（尺寸一一对应，无需再缩放）：
   `Copy-Item design\wtumcl-logo\icons\128.png public\images\icons\Logo_128x128.png -Force`
   `Copy-Item design\wtumcl-logo\icons\32.png public\images\icons\Logo_32x32.png -Force`
   `Copy-Item design\wtumcl-logo\icons\512.png src-tauri\assets\icons\icon.png -Force`
   `Copy-Item design\wtumcl-logo\icons\64.png src-tauri\assets\icons\64x64.png -Force`
   `Copy-Item design\wtumcl-logo\icons\256.png src-tauri\assets\icons\128x128@2x.png -Force`
3. 需要 Windows 安装包图标时，复制 `icons/WTUMCL.ico` 为 `src-tauri/assets/icons/icon.ico`，
   或直接跑 `npx tauri icon design/wtumcl-logo/icons/1024.png` 让 Tauri 自己生成 ico/icns/png 全套。
4. 重新构建并肉眼确认：`pnpm build` / `pnpm tauri build`（本次未执行任何构建）。
5. 回滚：把 `icons.bak` 拷回即可。

**白底注意**：本套图标是“白色六边形外框 + 透明底”。放到纯白背景上时白色外框会与背景融为一体，
只看到内部黑色六边形。若目标场景是白底/浅色主题，请改用反相色稿（`master/WTUMCL-logo-inverse.svg`、
`png/WTUMCL-logo-inverse-1024.png`）另行导出，或先给正稿加一层深色底再打包。
