# t2 audit report - v2 icon kit & compare re-export

Date: 2026-10-21. Auditor: t2 worker. Scope audited: design/wtumcl-logo/v2 only.

## State found on entry

The complete v2 icon kit, compare pages, checker and README section 8 were already
present in the workspace when this task started (see "Provenance" below). Nothing
in the kit was regenerated to fake a pass; instead the whole kit was **independently
audited**, then **rebuilt from its own pipeline into a scratch folder and compared
byte for byte**.

## Evidence

### 1. Nine PNGs - size, alpha, corners, centring
Measured independently (own script, not check_icons.py):

| file | size | corners alpha | alpha levels | bbox centre offset from canvas centre |
|---|---|---|---|---|
| 16.png | 16x16 | 0,0,0,0 | 24 | (0.00, 0.00) |
| 24.png | 24x24 | 0,0,0,0 | 32 | - |
| 32.png | 32x32 | 0,0,0,0 | 37 | (0.00, 0.00) |
| 48.png | 48x48 | 0,0,0,0 | 48 | - |
| 64.png | 64x64 | 0,0,0,0 | 67 | (0.00, 0.00) |
| 128.png | 128x128 | 0,0,0,0 | 132 | (0.00, 0.00) |
| 256.png | 256x256 | 0,0,0,0 | 193 | (0.00, 0.00) |
| 512.png | 512x512 | 0,0,0,0 | 199 | (0.00, 0.00) |
| 1024.png | 1024x1024 | 0,0,0,0 | 99 | (0.00, 0.00) |

All are mode RGBA. The 4096 master itself is centred with offset (0.00, 0.00),
so the straight square downscale is an equal-ratio centred export.

### 2. Downscale parity (the anti-"just upscaled" criterion)
Re-derived the reference with an independently written premultiplied-alpha Lanczos
(rounded uint8 premultiply -> PIL LANCZOS -> unpremultiply) and compared per pixel:

| size | MAE (/255) | max abs err |
|---|---|---|
| 16 | 0.0188 | 0.49 |
| 24 | 0.0077 | 0.46 |
| 32 | 0.0012 | 0.13 |
| 48 | 0.0007 | 0.07 |
| 64 | 0.0006 | 0.07 |
| 128 | 0.0236 | (255 at fully transparent px, RGB undefined there) |
| 256 | 0.0254 | ditto |
| 512 | 0.0190 | ditto |
| 1024 | 0.0206 | ditto |

Threshold is 1.0/255; worst measured 0.025/255. The reported max error of 255 only
occurs in pixels with alpha == 0, where RGB is not observable; MAE is unaffected.

### 3. ICO
ICONDIR magic 0/1, 7 frames: 16, 24, 32, 48, 64, 128, 256 (bpp 32, PNG-compressed).
Each frame blob is **byte-identical to the corresponding icons/<size>.png**, and every
frame decodes through Pillow at its advertised size with alpha.

### 4. Optical 16/24
16x16 and 24x24, RGBA, alpha levels exactly {0, 255} (binary - no grey), corners 0,
156 / 338 fully opaque pixels. Hairline policy documented in tools/make_icons.py.
Reviewed at 16x nearest zoom against the standard Lanczos versions: it is a
legitimate whole-pixel rendition, not a filter artefact.

### 5. Compare assets
compare/compare.png 2208x908 (>= 1600), three labelled panels (source 664 /
v2 redraw @664 / 50% blend), pairwise panel diff 36.1 / 56.7 / 30.7 (not duplicates).
compare/diff.png 1500x930 heat map with grey base. compare/icon-sheet.png 2088x1476
with dark + light plates and 8x zooms of 16/24 standard vs optical.
compare/optical-8x.png 1384x1126 with 12x zooms on both plates.

### 6. Reproducibility (strongest evidence)
Rebuilt the entire kit with tools/make_icons.py + tools/make_compare.py into a scratch
folder and hashed everything: **16/16 artifacts byte-identical** (SHA-256) to the
shipped ones. So the shipped files are exactly what the documented pipeline produces
from png/WTUMCL-logo-4096.png + data/*.json.

### 7. check_icons.py
`python design/wtumcl-logo/v2/check_icons.py` -> 19/19 PASS, exit 0.
Read-only proven: SHA-256 of all 16 files under icons/ and compare/ snapshotted before
and after the run - no changes. No writes anywhere in the script source.

### 8. Untouched outside v2
`git status --porcelain public/images/icons src-tauri/assets/icons` -> empty
(no tracked app asset modified). v1 design files and their subfolders still present
and unmodified. Icon files are only *referenced* in README 8.5 as instructions.

### 9. README section 8
Present: file list with byte counts (all 16 verified correct), size table, optical
rationale, verification instructions, replacement guide (public/images/icons/
Logo_128x128.png <- icons/128.png, Logo_32x32.png <- icons/32.png, src-tauri icon.png
<- inverse 512, plus ico/64/128/256), rebuild commands. All target sizes claimed in
the guide were checked against the real files and are correct (128/32/512/32/64/128/256
+ 7-frame icon.ico).

## Conclusion
All five acceptance criteria hold, verified independently of the kit's own checker.
