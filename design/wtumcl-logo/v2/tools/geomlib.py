# -*- coding: utf-8 -*-
"""Shared helpers: source bitmap loading, bilinear sampling, edge profiles.

All geometry in this project is expressed in the 664x664 source bitmap
coordinate system unless a function says otherwise.
"""
import json
import math
import os

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
DATA = os.path.join(V2, "data")

SRC_DEFAULT = r"F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png"
REF_LOCAL = os.path.join(os.path.dirname(V2), "reference", "WTUMCLOGO-reference.png")
SRC_SIZE = 664


def source_path():
    if os.path.isfile(SRC_DEFAULT):
        return SRC_DEFAULT
    if os.path.isfile(REF_LOCAL):
        return REF_LOCAL
    raise FileNotFoundError("WTUMCLOGO.png not found")


def load_lum(path=None):
    a = np.asarray(Image.open(path or source_path()).convert("RGB")).astype(np.float64)
    if a.shape[0] != SRC_SIZE or a.shape[1] != SRC_SIZE:
        raise ValueError("expected %dx%d source, got %s"
                         % (SRC_SIZE, SRC_SIZE, a.shape[:2]))
    return 0.299 * a[:, :, 0] + 0.587 * a[:, :, 1] + 0.114 * a[:, :, 2]


def load_rgb(path=None):
    return np.asarray(Image.open(path or source_path()).convert("RGB")).astype(np.float64)


def sample(lum, x, y):
    """Bilinear luminance sample, border clamped. x/y may be arrays."""
    h, w = lum.shape
    x = np.clip(np.asarray(x, dtype=np.float64), 0.0, w - 1.000001)
    y = np.clip(np.asarray(y, dtype=np.float64), 0.0, h - 1.000001)
    x0 = np.floor(x).astype(int)
    y0 = np.floor(y).astype(int)
    fx = x - x0
    fy = y - y0
    x1 = np.minimum(x0 + 1, w - 1)
    y1 = np.minimum(y0 + 1, h - 1)
    v00 = lum[y0, x0]
    v10 = lum[y0, x1]
    v01 = lum[y1, x0]
    v11 = lum[y1, x1]
    return (v00 * (1 - fx) * (1 - fy) + v10 * fx * (1 - fy)
            + v01 * (1 - fx) * fy + v11 * fx * fy)


def crossing(prof, params, level):
    """First position where prof crosses level; linear interpolation.

    prof/params are 1-D arrays.  Returns None when there is no crossing.
    """
    s = prof - level
    idx = np.nonzero((s[:-1] < 0.0) & (s[1:] >= 0.0))[0]
    if len(idx) == 0:
        return None
    i = int(idx[0])
    d = s[i + 1] - s[i]
    t = 0.0 if d == 0 else -s[i] / d
    return float(params[i] + t * (params[i + 1] - params[i]))


def line_from_points(pts):
    """Total-least-squares line fit: returns (point, unit direction, rms)."""
    p = np.asarray(pts, dtype=np.float64)
    c = p.mean(axis=0)
    q = p - c
    cov = q.T @ q / max(len(p), 1)
    evals, evecs = np.linalg.eigh(cov)
    d = evecs[:, int(np.argmax(evals))]
    n = np.array([-d[1], d[0]])
    rms = math.sqrt(float(np.mean((q @ n) ** 2)))
    return c, d, rms


def load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def save_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        json.dump(obj, f, indent=1, sort_keys=True)
        f.write("\n")
