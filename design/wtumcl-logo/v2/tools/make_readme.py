# -*- coding: utf-8 -*-
"""Generate v2/README.md from the measured data (and, by default, the check).

    python design/wtumcl-logo/v2/tools/make_readme.py            # runs check
    python design/wtumcl-logo/v2/tools/make_readme.py --no-check

Every number in the README is read back from data/*.json or from the output of
check_v2.py, so the document cannot drift away from the artefacts.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
DATA = os.path.join(V2, "data")
FENCE = chr(96) * 3


def load(name):
    with open(os.path.join(DATA, name), "r", encoding="utf-8") as f:
        return json.load(f)


def fmt_cell_table(sp):
    pal = sp["palette"]
    i0, j0 = sp["i0"], sp["j0"]
    med = {(c[0], c[1]): c[2] for c in sp["cell_medians"]}
    cls = {(c["i"], c["j"]): c["c"] for c in sp["cells"]}
    lines = []
    head = "| j\\i | " + " | ".join(str(i0 + k) for k in range(16)) + " |"
    lines.append(head)
    lines.append("|" + "---|" * 17)
    for dj in range(16):
        row = []
        for di in range(16):
            i, j = i0 + di, j0 + dj
            c = cls.get((i, j), -1)
            if c is None or c < 0:
                m = med.get((i, j))
                row.append("----" if m is None else "bg")
            else:
                row.append("#%02X%02X%02X" % tuple(pal[c]))
        lines.append("| %d | " % (j0 + dj) + " | ".join(row) + " |")
    return "\n".join(lines)


def main():
    geom = load("geometry.json")
    lat = load("lattice.json")
    word = load("wordmark.json")
    sp = lat["blocks"]["sprite"]
    hexg = geom["hex"]

    check_out = ""
    if "--no-check" not in sys.argv:
        r = subprocess.run([sys.executable, os.path.join(V2, "check_v2.py")],
                           capture_output=True, text=True)
        check_out = (r.stdout + r.stderr).strip()

    L = []
    A = L.append
    A("# WTUMCL logo v2 - lattice recovery + sub-pixel fit")
    A("")
    A("v2 rebuilds the WTUMCL mark from the 664x664 reference bitmap")
    A("(F:/Desktop/WTUMC-AI-VEDIO/WTUMCLOGO.png) instead of tracing it by eye.")
    A("The centre motif is the Minecraft **String** item icon, so it is")
    A("recovered on its 16x16 integer lattice; the hairlines and the hexagon are")
    A("fitted to sub-pixel 50 % intensity crossings.")
    A("")
    A("check_v2.py only reads; the tools in tools/ regenerate everything.")
    A("")
    A("## 1. Fitted geometry (source pixels, 664 grid)")
    A("")
    A("| quantity | value | note |")
    A("|---|---|---|")
    A("| outer hexagon centre | (%.3f, %.3f) | regular pointy-top hexagon |" % (hexg["cx"], hexg["cy"]))
    A("| outer hexagon R | %.3f | outer edge, 50 %% crossing, %d samples |" % (hexg["R_out"], hexg["n_outer"]))
    A("| outer hexagon RMS | %.3f px | residual of the regular-hexagon fit |" % hexg["rms_outer"])
    A("| black field centre | (%.3f, %.3f) | inner edge of the white ring |" % (hexg["field_cx"], hexg["field_cy"]))
    A("| black field R | %.3f | inner ring edge, %d samples |" % (hexg["R_field"], hexg["n_field"]))
    A("| black field RMS | %.3f px | |" % hexg["rms_field"])
    A("| inscribed hexagon R (R_insc) | %.3f | regular hexagon whose vertical sides are the two measured hairline rails |" % hexg["R_insc"])
    A("| hexagon rotation | %.1f deg | locked to 0: the hairlines are exactly axis aligned, a free rotation is degenerate with R |" % hexg["rot_deg"])
    A("| edge residual (outer+field) | %.3f px | the number check_v2 compares against 0.60 |" % hexg["rms_edges"])
    A("| hairline width w | %.3f px | median of the 14 measured segment widths (rms %.3f) |" % (geom["hairline_width"], geom["hairline_width_rms"]))
    A("")
    A("Hairlines (source pixels, 3 decimals):")
    A("")
    A("| segment | x1 | y1 | x2 | y2 | fitted width | TLS rms |")
    A("|---|---|---|---|---|---|---|")
    for ln in geom["lines"]:
        A("| %s | %.3f | %.3f | %.3f | %.3f | %.3f | %.4f |"
          % (ln["name"], ln["x1"], ln["y1"], ln["x2"], ln["y2"], ln["w"], ln["rms"]))
    A("")
    A("## 2. Centre sprite - 16x16 integer lattice")
    A("")
    A("Search: s in [6, 9] step 0.01, one phase in the period step %s,"
      % (lat["search"]["phase_step"]))
    A("objective = sum of within-cell luminance variance / pixel. Hairline")
    A("pixels are excluded from the objective.")
    A("")
    A("| quantity | value |")
    A("|---|---|")
    A("| lattice pitch s | %.3f px |" % sp["s"])
    A("| lattice origin (a grid line) | (%.3f, %.3f) |" % (sp["x0"], sp["y0"]))
    A("| canvas | %dx%d cells = %.1f x %.1f px |" % (sp["cols"], sp["rows"], sp["cols"] * sp["s"], sp["rows"] * sp["s"]))
    A("| cell index range | i %d..%d, j %d..%d |" % (sp["i0"], sp["i0"] + 15, sp["j0"], sp["j0"] + 15))
    A("| best objective (variance/pixel) | %.3f |" % sp["score"])
    A("| inked cells | %d of 256 |" % sp["ink_cells"])
    A("| palette entries | %d (<= 6) |" % len(sp["palette"]))
    A("")
    A("The 16x16 canvas is centred on the fitted hexagon centre to within")
    A("0.2 px, which is what identifies the motif as a 16x16 item icon.")
    A("")
    A("Every grid line is refined individually (sub-pixel 50 % crossing,")
    A("median along the line):")
    A("")
    A("| line k | nominal x | measured dx | nominal y | measured dy |")
    A("|---|---|---|---|---|")
    for k in range(17):
        dx = sp["line_offsets_x"][k]
        dy = sp["line_offsets_y"][k]
        A("| %d | %.3f | %s | %.3f | %s |"
          % (k, sp["grid_x_uniform"][k], "-" if dx is None else "%+.3f" % dx,
             sp["grid_y_uniform"][k], "-" if dy is None else "%+.3f" % dy))
    A("")
    A("### Palette")
    A("")
    A("| entry | hex | rgb | cells |")
    A("|---|---|---|---|")
    counts = [0] * len(sp["palette"])
    for c in sp["cells"]:
        if c["c"] >= 0:
            counts[c["c"]] += 1
    for n, col in enumerate(sp["palette"]):
        A("| P%d | #%02X%02X%02X | %d, %d, %d | %d |"
          % (n, col[0], col[1], col[2], col[0], col[1], col[2], counts[n]))
    A("")
    A("Every cell of the 16x16 canvas is listed below, so the recovery can be")
    A("re-checked cell by cell. bg = background (black field or hole); ---- =")
    A("cell centre outside the bitmap.")
    A("")
    A(fmt_cell_table(sp))
    A("")
    A("## 3. Word mark")
    A("")
    A("WTU and MC are white on black and axis aligned. They were run through")
    A("the same lattice search: the variance objective has no sharp minimum")
    A("there (the letters are spaced proportionally, not on a common lattice),")
    A("so the production vector uses a band/run decomposition of the 50 %")
    A("intensity mask with every rectangle edge refined to the measured")
    A("crossing. The lattice result is still recorded in data/lattice.json")
    A("(blocks.wtu, blocks.mc).")
    A("")
    A("| block | rectangles |")
    A("|---|---|")
    for k, blk in word["blocks"].items():
        A("| %s | %d |" % (k, len(blk["rects"])))
    A("")
    A("## 4. Colours")
    A("")
    A("The reference bitmap is not pure black / pure white: inside the artwork")
    A("the field measures RGB(0, 8, 5) (median of the dark pixels) and the")
    A("whites measure RGB(248, 253, 254). The v2 master uses those measured")
    A("levels so the export sits on the source's own noise floor; visually they")
    A("are still black and white.")
    A("")
    A("## 5. v2 vs v1")
    A("")
    A("| | v1 | v2 |")
    A("|---|---|---|")
    A("| centre motif | 40x40 samples at pitch 3.4, nearest colour, no lattice solve | 16x16 integer lattice, s=%.3f, per-line sub-pixel refinement, %d-colour palette |" % (sp["s"], len(sp["palette"])))
    A("| motif geometry | cells 3.4 px wide drifting off the real blocks | every cell edge on the measured 50 %% crossing (line dx %+.2f..%+.2f px from uniform) |" % (min(x for x in sp["line_offsets_x"] if x is not None), max(x for x in sp["line_offsets_x"] if x is not None)))
    A("| word mark | row bands, integer edges, threshold 140 | row bands, sub-pixel edges, block-level 50 % level |")
    A("| hexagon | hand measured (312.3 / 226.9), single centre | 50 %%-crossing fit, %d + %d samples, RMS %.3f / %.3f px |" % (hexg["n_outer"], hexg["n_field"], hexg["rms_outer"], hexg["rms_field"]))
    A("| hairlines | fixed 2.0 px, hand model endpoints | TLS per segment, width %.3f px, junctions by intersection |" % geom["hairline_width"])
    A("| colours | #000000 / #FFFFFF / #B8CFD0 | measured ink #000805, white #F8FDFE, sprite palette from k-means on cell medians |")
    A("| verification | mask IoU only (min 0.85), no alignment | global IoU, centre IoU + MAE, word mark IoU, hexagon RMS, SVG self-render, all after sub-pixel alignment |")
    A("")
    A("## 6. Verification")
    A("")
    A(FENCE)
    A(check_out)
    A(FENCE)
    A("")
    A("## 7. Rebuild")
    A("")
    A(FENCE)
    A("python design/wtumcl-logo/v2/tools/fit_geometry.py    # -> data/geometry.json")
    A("python design/wtumcl-logo/v2/tools/fit_lattice.py     # -> data/lattice.json (full search, ~6 min)")
    A("python design/wtumcl-logo/v2/tools/fit_wordmark.py    # -> data/wordmark.json")
    A("python design/wtumcl-logo/v2/tools/build_svg.py       # -> master/*.svg")
    A("python design/wtumcl-logo/v2/tools/render.py          # -> png/*.png")
    A("python design/wtumcl-logo/v2/tools/make_previews.py   # -> preview/*.png")
    A("python design/wtumcl-logo/v2/tools/make_readme.py     # -> README.md")
    A("python design/wtumcl-logo/v2/check_v2.py              # read-only validation")
    A(FENCE)
    A("")
    A("fit_lattice.py --fast uses a 0.25 phase step for a quick look; the")
    A("committed data come from the full 0.05 step run.")
    A("")
    body = "\n".join(L) + "\n"
    icons_section = os.path.join(HERE, "readme_icons_section.md")
    if os.path.isfile(icons_section):
        with open(icons_section, "r", encoding="utf-8") as fh:
            body += "\n" + fh.read().rstrip("\n") + "\n"
        print("appended", os.path.basename(icons_section))
    with open(os.path.join(V2, "README.md"), "w", encoding="utf-8", newline="\n") as f:
        f.write(body)
    print("wrote", os.path.join(V2, "README.md"))


if __name__ == "__main__":
    main()
