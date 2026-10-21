#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Build the human-review sheets for the WTUMCL logo.

    python design/wtumcl-logo/tools/make_compare.py

Outputs (all under compare/)
    compare.png    source | redraw @664 | 50% overlay, labelled, 2208 px wide
    diff.png       difference heat map on a grey base, with metrics
    icon-sheet.png every exported icon size on dark and light backgrounds,
                   plus 8x nearest-neighbour zooms of the 16/24/32 px frames
"""
import os

from PIL import Image, ImageDraw, ImageFont
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SRC_REF = os.path.join(ROOT, "reference", "WTUMCLOGO-reference.png")
SRC_BIG = os.path.join(ROOT, "png", "WTUMCL-logo-4096.png")
ICONS = os.path.join(ROOT, "icons")
OUT = os.path.join(ROOT, "compare")

PANEL = 664
BG = (16, 17, 22)
PANEL_BG = (0, 0, 0)
CARD_BG = (22, 23, 29)
LINE = (52, 54, 64)
TEXT = (234, 236, 241)
MUTED = (150, 154, 166)
ACCENT = (148, 196, 202)
ICO_ZOOM = (16, 24, 32)
SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)


def font(size, bold=False):
    cands = [r"C:\Windows\Fonts\segoeuib.ttf" if bold else r"C:\Windows\Fonts\segoeui.ttf",
             r"C:\Windows\Fonts\arialbd.ttf" if bold else r"C:\Windows\Fonts\arial.ttf"]
    for p in cands:
        if os.path.isfile(p):
            try:
                return ImageFont.truetype(p, size)
            except Exception:
                pass
    return ImageFont.load_default()


def premultiplied(img):
    a = np.asarray(img.convert("RGBA"), dtype=np.float32)
    out = a.copy()
    out[..., :3] = a[..., :3] * (a[..., 3:4] / 255.0)
    return Image.fromarray(np.rint(out).astype(np.uint8), "RGBA")


def unpremultiplied(img):
    a = np.asarray(img, dtype=np.float32)
    af = a[..., 3:4] / 255.0
    rgb = np.divide(a[..., :3], af, out=np.zeros_like(a[..., :3]), where=af > 1e-4)
    out = a.copy()
    out[..., :3] = np.clip(np.rint(rgb), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def lanczos(img, size):
    pre = premultiplied(img)
    small = pre.resize((size, size), Image.LANCZOS)
    pre.close()
    out = unpremultiplied(small)
    small.close()
    return out


def on_black(img):
    bg = Image.new("RGBA", img.size, (0, 0, 0, 255))
    return Image.alpha_composite(bg, img).convert("RGB")


def label(dr, xy, text, fnt, fill=TEXT, anchor="la"):
    dr.text(xy, text, font=fnt, fill=fill, anchor=anchor)


def frame(dr, box, color=LINE, width=1):
    dr.rectangle(box, outline=color, width=width)


def accent_rule(dr, x, y, w, color=ACCENT):
    dr.rectangle([x, y, x + w, y + 3], fill=color)


# ------------------------------------------------------------------ compare --
def build_compare(source, redraw_black):
    margin, gap = 64, 44
    width = margin * 2 + PANEL * 3 + gap * 2
    label_y = 118
    panel_y = 154
    height = panel_y + PANEL + 30 + 26 + 34

    canvas = Image.new("RGB", (width, height), BG)
    dr = ImageDraw.Draw(canvas)
    f_title, f_sub, f_lab, f_foot = font(30, True), font(18), font(22, True), font(18)

    label(dr, (margin, 28), "WTUMCL logo - source vs redraw vs overlay", f_title)
    label(dr, (margin, 76),
          "source: reference/WTUMCLOGO-reference.png (664x664, opaque)   |   "
          "redraw: png/WTUMCL-logo-4096.png Lanczos -> 664x664, composited on black to match the source background"
          "   |   overlay: 50% blend of the two", f_sub, MUTED)

    overlay = Image.blend(source, redraw_black, 0.5)
    panels = [
        ("Left: source", "(reference bitmap, 664x664)", source),
        ("Center: redraw", "(vector master rendered at 664)", redraw_black),
        ("Right: overlay", "(source 50% + redraw 50%)", overlay),
    ]
    for i, (title, sub, img) in enumerate(panels):
        x = margin + i * (PANEL + gap)
        label(dr, (x, label_y), title, f_lab)
        tw = int(dr.textlength(title, font=f_lab))
        label(dr, (x + tw + 10, label_y + 5), sub, f_sub, MUTED)
        accent_rule(dr, x, panel_y - 12, PANEL)
        canvas.paste(img, (x, panel_y))
        frame(dr, [x - 1, panel_y - 1, x + PANEL, panel_y + PANEL], LINE)

    foot = ("panels are pixel-identical in scale; the redraw is centred on the canvas while the source bitmap "
            "sits at its measured centre, so the overlay shows the deliberate centring shift.")
    label(dr, (margin, height - 26), foot, f_foot, MUTED)
    return canvas


# --------------------------------------------------------------------- diff --
def heat_diff(source, redraw_black):
    s = np.asarray(source, dtype=np.float32)
    r = np.asarray(redraw_black, dtype=np.float32)
    d = np.abs(s - r).mean(axis=2) / 255.0
    amp = np.clip(d * 3.0, 0.0, 1.0)
    base = float(90)
    heat = np.zeros_like(s)
    heat[..., 0] = 255.0
    heat[..., 1] = 212.0 - 182.0 * amp
    heat[..., 2] = 40.0 - 30.0 * amp
    out = base * (1.0 - amp[..., None]) + heat * amp[..., None]
    img = Image.fromarray(np.clip(np.rint(out), 0, 255).astype(np.uint8), "RGB")
    return img, d


def build_diff(source, redraw_black, heat, d):
    margin, gap = 64, 44
    width = margin * 2 + PANEL * 2 + gap
    label_y, panel_y = 118, 154
    legend_y = panel_y + PANEL + 20
    height = legend_y + 92
    canvas = Image.new("RGB", (width, height), BG)
    dr = ImageDraw.Draw(canvas)
    f_title, f_sub, f_lab, f_foot = font(30, True), font(18), font(22, True), font(18)

    label(dr, (margin, 28), "WTUMCL logo - source vs redraw difference heat map", f_title)
    label(dr, (margin, 76),
          "per-pixel mean |RGB difference| amplified x3; grey = match, yellow -> red = mismatch. "
          "Source is opaque black-backed, redraw is composited on black first.", f_sub, MUTED)

    gray = source.convert("L")
    gray = gray.point(lambda v: int(56 + v * 0.55))
    gray_rgb = Image.merge("RGB", (gray, gray, gray))

    panels = [("Left: source (grey reference)", gray_rgb), ("Right: diff heat map (x3)", heat)]
    for i, (title, img) in enumerate(panels):
        x = margin + i * (PANEL + gap)
        label(dr, (x, label_y), title, f_lab)
        accent_rule(dr, x, panel_y - 12, PANEL)
        canvas.paste(img, (x, panel_y))
        frame(dr, [x - 1, panel_y - 1, x + PANEL, panel_y + PANEL], LINE)

    lx = margin + PANEL + gap
    steps = 220
    for i in range(steps):
        t = i / float(steps - 1)
        col = (255, int(212 - 182 * t), int(40 - 30 * t))
        dr.rectangle([lx + i, legend_y, lx + i + 1, legend_y + 12], fill=col)
    frame(dr, [lx - 1, legend_y - 1, lx + steps, legend_y + 13], LINE)
    label(dr, (lx, legend_y + 20), "match", f_sub, MUTED)
    label(dr, (lx + steps, legend_y + 20), "difference", f_sub, MUTED, anchor="ra")

    mean_abs = float(d.mean() * 255.0)
    big = d > (8.0 / 255.0)
    stats = ("mean |diff| = %.2f/255   |   pixels with |diff| > 8: %.2f%%   |   "
             "amplification x3, ramp 0..85/255" % (mean_abs, 100.0 * big.mean()))
    label(dr, (margin, legend_y + 58), stats, f_foot, MUTED)
    return canvas


# ---------------------------------------------------------------- icon sheet --
def build_sheet():
    icons = {}
    for s in SIZES:
        p = os.path.join(ICONS, "%d.png" % s)
        im = Image.open(p)
        im.load()
        icons[s] = im.convert("RGBA")

    sizes_row = (16, 24, 32, 48, 64, 128, 256)
    margin, cell, row_h = 64, 280, 320
    width = margin * 2 + cell * len(sizes_row)
    top = 152
    sheet = Image.new("RGB", (width, top + row_h * 4 + 44), BG)
    dr = ImageDraw.Draw(sheet)
    f_title, f_sub, f_cell, f_note = font(30, True), font(18), font(17), font(16)

    label(dr, (margin, 28), "WTUMCL icon kit - exported sizes on dark and light backgrounds", f_title)
    label(dr, (margin, 76),
          "rows 1-2: native sizes 16..256, drawn 1:1 with no resampling; rows 3-4: 8x nearest-neighbour zoom of "
          "16 / 24 / 32 to inspect edge quality.", f_sub, MUTED)
    label(dr, (margin, 102),
          "the light rows put the transparent icon on a grey contrast plate: on plain white the white hexagon rim "
          "disappears - use master/WTUMCL-logo-inverse.svg for white backgrounds.", f_sub, MUTED)

    layouts = [
        ("dark background  #14151a", (20, 21, 26), sizes_row, 1),
        ("light background  #f2f2f5  (grey contrast plate)", (242, 242, 245), sizes_row, 1),
        ("dark background, 8x nearest zoom of 16 / 24 / 32", (20, 21, 26), ICO_ZOOM, 8),
        ("light background, 8x nearest zoom of 16 / 24 / 32", (242, 242, 245), ICO_ZOOM, 8),
    ]
    for r, (name, bg, sizes, scale) in enumerate(layouts):
        y0 = top + r * row_h
        dr.rectangle([margin, y0, width - margin, y0 + row_h - 24], fill=bg)
        label(dr, (margin + 14, y0 + 8), name, f_cell, MUTED if bg[0] < 128 else (70, 72, 80))
        baseline = y0 + row_h - 56
        for c, s in enumerate(sizes):
            cx = margin + c * cell + cell // 2
            im = icons[s]
            shown = im.resize((s * scale, s * scale), Image.NEAREST) if scale > 1 else im
            x = cx - shown.size[0] // 2
            y = baseline - shown.size[1]
            if bg[0] > 128:
                dr.rectangle([x - 8, y - 8, x + shown.size[0] + 7, y + shown.size[1] + 7],
                             fill=(208, 209, 215))
            sheet.paste(shown, (x, y), shown)
            label(dr, (cx, y0 + row_h - 44), "%dpx%s" % (s, " (%dx)" % scale if scale > 1 else ""),
                  f_cell, MUTED if bg[0] < 128 else (70, 72, 80), anchor="ma")
        if r >= 2:
            nx = margin + len(sizes) * cell + 4
            label(dr, (nx, y0 + 44),
                  "nearest-neighbour zoom: no smoothing, so any", f_note, MUTED)
            label(dr, (nx, y0 + 68),
                  "resampling artefact would be visible. The", f_note, MUTED)
            label(dr, (nx, y0 + 92),
                  "hexagon rim survives down to 16 px; hairlines", f_note, MUTED)
            label(dr, (nx, y0 + 116),
                  "drop below one pixel by design.", f_note, MUTED)
    return sheet


def main():
    os.makedirs(OUT, exist_ok=True)
    for p in (SRC_REF, SRC_BIG):
        if not os.path.isfile(p):
            raise SystemExit("missing input: %s" % p)

    source = Image.open(SRC_REF)
    source.load()
    source = source.convert("RGB")
    big = Image.open(SRC_BIG)
    big.load()
    redraw = lanczos(big, PANEL)
    redraw_black = on_black(redraw)

    cmp_img = build_compare(source, redraw_black)
    p = os.path.join(OUT, "compare.png")
    cmp_img.save(p, format="PNG", optimize=True)
    print("wrote %-44s %dx%d" % (os.path.basename(p), cmp_img.size[0], cmp_img.size[1]))

    heat, d = heat_diff(source, redraw_black)
    diff_img = build_diff(source, redraw_black, heat, d)
    p = os.path.join(OUT, "diff.png")
    diff_img.save(p, format="PNG", optimize=True)
    print("wrote %-44s %dx%d" % (os.path.basename(p), diff_img.size[0], diff_img.size[1]))
    print("      mean |diff| = %.3f/255, pixels>8 = %.2f%%" % (float(d.mean() * 255.0),
                                                                100.0 * float((d > 8.0 / 255).mean())))

    sheet = build_sheet()
    p = os.path.join(OUT, "icon-sheet.png")
    sheet.save(p, format="PNG", optimize=True)
    print("wrote %-44s %dx%d" % (os.path.basename(p), sheet.size[0], sheet.size[1]))


if __name__ == "__main__":
    main()
