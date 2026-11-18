#!/usr/bin/env python3
"""Builds Quillet Serif, the reply face of the cmux agent chat, from Newsreader (SIL OFL 1.1, no Reserved
Font Name). It sets agent prose at 15 px with a 1.6 line height. What changes, and why: docs/design-serif.md.

    python3 build/serif.py --out fonts

Writes OUT/quillet-serif.woff2 and OUT/quillet-serif-italic.woff2 (variable wght, Latin) and
build/out/serif.json (before/after metrics and sizes, the change list, the checks).
"""

from __future__ import annotations

import argparse
import copy
import io
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common  # noqa: E402
from fontTools.pens.recordingPen import DecomposingRecordingPen  # noqa: E402
from fontTools.ttLib import TTFont  # noqa: E402
from fontTools.varLib import instancer  # noqa: E402

FAMILY = "Quillet Serif"
COPYRIGHT = "Copyright 2020 The Newsreader Project Authors (https://github.com/productiontype/Newsreader)"
DESCRIPTION = (
    "Quillet Serif is a modified version of Newsreader: one optical size cut for 15 px screen prose, a taller "
    "x-height, looser spacing, old-style figures and softened corners by default."
)

# --- The design (each number is argued in docs/design-serif.md) -----------------------------------------
OPSZ = 12  # optical size pinned for 15 px screen text: sturdier hairlines and serifs, looser fit than opsz 16
X_HEIGHT = 500  # from the measured x-height at OPSZ, up to 0.50 em
TRACK = {"upright": 8, "italic": 6}  # units added to every advance (1000 upm), half each side
# Corners: blunt the sharp convex straight-to-straight corners (stem feet, serif tips, wedge terminals).
SOFTEN = {"fraction": 0.3, "cap": 10.0, "min_turn": 60.0, "max_turn": 120.0, "min_edge": 16.0}
# Glyphs the corner pass leaves alone: figures (their tabular rhythm and old-style shapes are drawn to be
# crisp), punctuation and symbols (small, a rounded corner reads as blur at 15 px).
DEFAULT_FEATURES: dict[str, list[str]] = {"upright": [], "italic": []}  # filled from the GSUB study

FACES = {
    "upright": {"source": "Newsreader[opsz,wght].ttf", "stem": "quillet-serif", "style": "Regular"},
    "italic": {"source": "Newsreader-Italic[opsz,wght].ttf", "stem": "quillet-serif-italic", "style": "Italic"},
}
POINT_GLYPHS = "aegry"


def points(font: TTFont, chars: str = POINT_GLYPHS) -> dict:
    cmap, glyf = font.getBestCmap(), font["glyf"]
    out = {}
    for c in chars:
        name = cmap[ord(c)]
        g = glyf[name]
        out[c] = {"glyph": name, "points": "composite" if g.isComposite() else len(g.coordinates), "contours": g.numberOfContours}
    return out


def x_height_glyphs(font: TTFont) -> set[str]:
    """Lowercase letters with their alternates and ligatures, plus the old-style figures, which are drawn on the
    x-height and would read short beside taller lowercase."""
    glyphs = common.lowercase_glyphs(font)
    for name in font.getGlyphOrder():
        if ".osf" in name or ".onum" in name or name.endswith(".tosf") or ".oldstyle" in name:
            if not any(part in name for part in common.NOT_ON_X_HEIGHT):
                glyphs.add(name)
    return glyphs


def softenable(font: TTFont) -> list[str]:
    """Letters (Unicode L*, by the cmap) with their alternates and ligatures. Not figures (old-style and
    tabular shapes are drawn crisp and their rhythm is the point), not punctuation or symbols (small marks,
    where a rounded corner reads as blur at 15 px)."""
    import unicodedata

    letters = {name for code, name in font.getBestCmap().items() if unicodedata.category(chr(code)).startswith("L")}
    keep = []
    for name in font.getGlyphOrder():
        stem = name.split(".", 1)[0]
        if name in letters or stem in letters or ("_" in stem and all(p in letters for p in stem.split("_"))):
            keep.append(name)
    return keep


def verify(woff2: Path, reference: dict) -> dict:
    """Reopens the woff2, instances it at the weight axis's min, default and max and draws every glyph."""
    font = TTFont(woff2)
    axis = next(a for a in font["fvar"].axes if a.axisTag == "wght")
    out = {"reopened": True, "glyphs": len(font.getGlyphOrder()), "axes": common.axis_order(font), "instances": {}}
    gs = None
    for wght in (axis.minValue, axis.defaultValue, axis.maxValue):
        inst = instancer.instantiateVariableFont(TTFont(woff2), {"wght": wght})
        gs = inst.getGlyphSet()
        errors = []
        for name in inst.getGlyphOrder():
            try:
                pen = DecomposingRecordingPen(gs)
                gs[name].draw(pen)
            except Exception as e:  # noqa: BLE001
                errors.append(f"{name}: {e}")
        x = common.measured(inst, "x")
        out["instances"][str(wght)] = {"glyph_errors": errors[:10], "n_errors": len(errors), "xHeight": x,
                                         "n_advance": inst["hmtx"][inst.getBestCmap()[ord("n")]][0]}
    out["ok"] = all(v["n_errors"] == 0 for v in out["instances"].values())
    return out


def build_face(key: str, out: Path, work: Path) -> dict:
    face = FACES[key]
    source = common.source(face["source"])
    original_points = points(source)
    font = instancer.instantiateVariableFont(source, {"opsz": OPSZ})
    before = common.report(font)
    before["sxHeight"], before["sCapHeight"] = font["OS/2"].sxHeight, font["OS/2"].sCapHeight
    before_points = points(font)
    changes = [f"opsz pinned at {OPSZ} (instancer); wght stays variable"]
    common.subset(font)  # first: every later pass then touches only the Latin glyphs and what GSUB reaches
    changes.append("Latin subset (Fontsource latin), hinting dropped")

    # Alternates first, while the cmap still names the plain forms and before outlines move.
    alternates = {}
    if DEFAULT_FEATURES[key]:
        mapping = common.feature_mapping(font, DEFAULT_FEATURES[key])
        alternates = {"features": DEFAULT_FEATURES[key], "characters": "".join(common.make_default(font, mapping))}
        changes.append(f"default forms from {', '.join(DEFAULT_FEATURES[key])}: {alternates['characters']}")

    font["OS/2"].sxHeight = common.measured(font, "x")  # the flat x-height of this optical size
    xh = common.set_x_height(font, X_HEIGHT, x_height_glyphs(font))
    changes.append(f"x-height {xh['from']} -> {xh['to']} units (lowercase, ligatures, old-style figures, marks, anchors)")

    targets = softenable(font)
    corners = common.soften_corners(font, targets, **SOFTEN)
    changes.append(f"corners softened: {corners} corners in {len(targets)} letter glyphs, radius {SOFTEN['fraction']} of the shorter edge, at most {SOFTEN['cap']} units")

    tracked = common.track(font, TRACK[key])
    changes.append(f"tracking +{TRACK[key]} units on {tracked} spacing glyphs")

    common.set_line_metrics(font)
    changes.append(f"line box {common.LINE_ASCENDER}/{common.LINE_DESCENDER}, gap 0, USE_TYPO_METRICS")
    common.rename(font, FAMILY, face["style"], source["name"].getDebugName(0) or COPYRIGHT, DESCRIPTION)
    after = common.report(font)
    after["sxHeight"], after["sCapHeight"] = font["OS/2"].sxHeight, font["OS/2"].sCapHeight
    after_points = points(font)
    sizes = common.write(font, work, face["stem"])
    woff2 = out / f"{face['stem']}.woff2"
    shutil.copyfile(work / f"{face['stem']}.woff2", woff2)  # only the woff2 goes to OUT (fonts/ holds no .ttf)
    before_size = common.woff2_size_of_source_subset(face["source"], pin={"opsz": OPSZ})
    full_size = common.woff2_size_of_source_subset(face["source"])
    return {
        "file": woff2.name,
        "source": face["source"],
        "woff2": {"before_same_subset_same_opsz": before_size, "before_same_subset_opsz_variable": full_size, "after": sizes["woff2"]},
        "metrics": {"before": before, "after": after},
        "px_at_15": {"xHeight": round(after["xHeight"] * 15 / 1000, 2), "capHeight": round(after["capHeight"] * 15 / 1000, 2)},
        "points": {"source": original_points, "before_pinned": before_points, "after": after_points},
        "alternates": alternates,
        "changes": changes,
        "verify": verify(woff2, after),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    work = common.ROOT / "build" / "out" / "serif"
    report = {"family": FAMILY, "opsz": OPSZ, "x_height": X_HEIGHT, "track": TRACK, "soften": SOFTEN, "faces": {}}
    for key in FACES:
        report["faces"][key] = build_face(key, args.out, work)
    path = common.ROOT / "build" / "out" / "serif.json"
    path.write_text(json.dumps(report, indent=1, default=str) + "\n")
    ok = all(f["verify"]["ok"] for f in report["faces"].values())
    for key, f in report["faces"].items():
        print(f"{f['file']}: {f['woff2']['after']} bytes (before {f['woff2']['before_same_subset_same_opsz']}), verify {'ok' if f['verify']['ok'] else 'FAILED'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
