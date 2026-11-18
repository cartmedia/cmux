#!/usr/bin/env python3
"""Builds Quillet Sans, the sans of the Quillet family, from Hanken Grotesk (SIL OFL 1.1, no Reserved Font Name).

The sans sets the cmux agent chat's interface (13-14 px), its composer and the user's own messages (15 px). Every
change is made by this code on Hanken Grotesk's own OFL outlines, through build/common.py, and each is written
down with its reason in docs/design-sans.md:

- x-height (X_HEIGHT): lowercase stretched to a little more of the em, so 13 px UI text reads a size up;
- a tailed l (TAIL): Hanken's l is a bare stem, like its I; the foot becomes a hook, so I l 1 differ by shape;
- default forms (DEFAULT_FEATURES): Hanken's own alternates drawn without a feature, the plain forms kept
  behind the same feature;
- softer corners (SOFTEN): the sharp stroke ends and stem feet of letters and figures get a small round;
- tracking (TRACKING): a little more room between letters at 13-15 px;
- the shared Quillet line box (common.set_line_metrics) and the name.

    python3 build/sans.py --out fonts      writes fonts/quillet-sans.woff2, fonts/quillet-sans-italic.woff2
                                            and build/out/sans.json (the TTFs go to build/out/sans/)
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
import unicodedata
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import common  # noqa: E402
from fontTools.ttLib import TTFont  # noqa: E402
from fontTools.ttLib.tables._g_l_y_f import flagOnCurve  # noqa: E402

FAMILY = "Quillet Sans"
FACES = [
    ("HankenGrotesk[wght].ttf", "Regular", "quillet-sans"),
    ("HankenGrotesk-Italic[wght].ttf", "Italic", "quillet-sans-italic"),
]
DESCRIPTION = (
    "Quillet Sans is the sans of Quillet, cmux's type family for the agent chat: a modified version of Hanken "
    "Grotesk under the SIL Open Font License 1.1, with a taller x-height, a tailed l, a single-storey g, "
    "softened stroke ends and slightly open spacing for interface text at 13-15 px."
)

# --- The design numbers (docs/design-sans.md says why) ---------------------------------------------------

X_HEIGHT = 520  # units of 1000 (Hanken: see sans.json before.xHeight)
TRACKING = 10  # units added to every advance, 5 each side: +0.15 px at 15 px, +0.13 px at 13 px
SOFTEN = {"fraction": 0.22, "cap": 18}  # radius = 22 % of the shorter edge at that master, at most 18 units
# Hanken's own forms made the default: ss02 is the single-storey g (g.ss02, gbreve.ss02, ...).
DEFAULT_FEATURES = ["ss02"]
# The l's hook, in shares of the stem width at each master.
TAIL = {"foot": 0.88, "inner": 0.62, "reach": 1.05, "end_side": 0.55}
TAIL_FLAGS = [flagOnCurve, flagOnCurve, flagOnCurve, 0, 0, flagOnCurve, flagOnCurve, flagOnCurve, flagOnCurve, 0, 0, flagOnCurve]
# Glyphs whose corners stay sharp (beyond what soften_targets leaves out: marks, punctuation, composites).
SHARP: set[str] = set()


def build(source_name: str, style: str, stem: str, out: Path, work: Path) -> dict:
    font = common.source(source_name)
    before = common.report(font)
    before_points = point_counts(font)
    changes = []

    # 1. Latin subset first, so every later edit touches only shipped glyphs. All layout features are kept.
    common.subset(font)

    # 2. x-height.
    xh = common.set_x_height(font, X_HEIGHT)
    changes.append({"change": "x-height", **xh, "why": "13 px UI text reads a size up; see design-sans.md"})

    # 3. The tailed l (in place, so lacute, lcaron, lcommaaccent and ldot, composites of l, follow).
    tail = tailed_l(font)
    changes.append({"change": "tailed l", **TAIL, **tail})

    # 4. Default forms, the plain forms kept behind the same feature.
    mapping = common.feature_mapping(font, DEFAULT_FEATURES)
    if not mapping:
        raise SystemExit(f"{source_name}: no single substitutions in {DEFAULT_FEATURES}")
    swapped = common.make_default(font, mapping)
    reachable = keep_forms_reachable(font, mapping)
    changes.append({"change": "default forms", "features": DEFAULT_FEATURES, "characters": "".join(swapped), "rules_added": reachable})

    # 5. Softer corners.
    glyphs = soften_targets(font)
    rounded = common.soften_corners(font, glyphs, **SOFTEN)
    changes.append({"change": "soften corners", **SOFTEN, "glyphs": len(glyphs), "corners": rounded})

    # 6. Tracking.
    n = common.track(font, TRACKING)
    changes.append({"change": "tracking", "units": TRACKING, "glyphs": n})

    # 7. HVAR from the outlines' phantom points: the l's advance now varies differently with weight.
    if "HVAR" in font:
        from fontTools.varLib.hvar import add_HVAR

        del font["HVAR"]
        add_HVAR(font)
        changes.append({"change": "HVAR rebuilt from gvar"})

    # 8. Shared line box, names.
    common.set_line_metrics(font)
    changes.append({"change": "line metrics", "ascender": common.LINE_ASCENDER, "descender": common.LINE_DESCENDER, "lineGap": 0})
    original = font["name"].getDebugName(0)
    common.rename(font, FAMILY, style, original, DESCRIPTION)

    sizes = common.write(font, work, stem)
    shutil.copyfile(work / f"{stem}.woff2", out / f"{stem}.woff2")
    final = TTFont(work / f"{stem}.ttf")
    return {
        "source": source_name,
        "file": f"{stem}.woff2",
        "before": {**before, "sCapHeight": before_cap(source_name)},
        "after": common.report(final),
        "points": {"before": before_points, "after": point_counts(final)},
        "woff2_before": common.woff2_size_of_source_subset(source_name),
        "woff2_after": sizes["woff2"],
        "changes": changes,
    }


def before_cap(source_name: str) -> int:
    return common.source(source_name)["OS/2"].sCapHeight


def point_counts(font: TTFont, chars: str = "aglrt") -> dict:
    cmap, glyf = font.getBestCmap(), font["glyf"]
    out = {}
    for c in chars:
        g = glyf[cmap[ord(c)]]
        out[c] = f"{cmap[ord(c)]}: " + ("composite" if g.isComposite() else str(len(g.coordinates)))
    return out


def tailed_l(font: TTFont, name: str = "l") -> dict:
    """Hanken's l is a bare stem, the same shape as its I, one cap-to-ascender step taller: at 13 px the two
    differ by about 1 px of height. This turns the stem's foot into a hook to the right, sized from the stem's
    own width at every master (so it thickens with weight like the rest of the face), built in upright space
    (the italic is unslanted first and slanted back). The advance grows by the hook's reach, per master."""
    slant = math.tan(math.radians(-font["post"].italicAngle))
    glyph = font["glyf"][name]
    if glyph.isComposite() or glyph.numberOfContours != 1 or len(glyph.coordinates) != 4:
        raise SystemExit(f"{name}: expected a four-point stem")
    coords, _, _, masters = common._masters(font, name)
    n = len(coords) - 4
    phantoms = []
    for m in masters:
        m = list(m)
        up = [(x - y * slant, y) for x, y in m[:n]]
        xr = max(x for x, _ in up)
        s = xr - min(x for x, _ in up)
        rsb = m[n + 1][0] - xr
        phantoms.append([m[n], (xr + TAIL["reach"] * s + rsb * TAIL["end_side"], m[n + 1][1]), m[n + 2], m[n + 3]])
    shape: dict = {}

    def edit(masters, end_pts, flags):
        new_masters = []
        for m in masters:
            up = [(x - y * slant, y) for x, y in m]
            xl, xr = min(x for x, _ in up), max(x for x, _ in up)
            top, bottom = max(y for _, y in up), min(y for _, y in up)
            s = xr - xl
            tb = TAIL["foot"] * s  # the foot is a horizontal: a little thinner than the stem
            ri = TAIL["inner"] * s
            xe = xr + TAIL["reach"] * s
            rox, roy = ri + s, ri + tb  # outer curve concentric with the inner one
            k = 0.45  # off-curve points 0.45 r from the corner: a near-circular quadratic quarter
            pts = [
                (xl, top), (xr, top),
                (xr, bottom + tb + ri), (xr, bottom + tb + ri * k), (xr + ri * k, bottom + tb), (xr + ri, bottom + tb),
                (xe, bottom + tb), (xe, bottom),  # the hook ends in a vertical cut, as the stem ends in a level one
                (xl + rox, bottom), (xl + rox * k, bottom), (xl, bottom + roy * k), (xl, bottom + roy),
            ]
            new_masters.append([(x + y * slant, y) for x, y in pts])
            if not shape:
                shape.update(stem=round(s), foot=round(tb), inner_radius=round(ri), end_x=round(xe))
        return new_masters, [len(TAIL_FLAGS) - 1], list(TAIL_FLAGS), phantoms

    advance = font["hmtx"][name][0]
    common.edit_glyph(font, name, edit)
    shape["advance"] = [advance, font["hmtx"][name][0]]
    return shape


def keep_forms_reachable(font: TTFont, mapping: dict[str, str]) -> list[str]:
    """The cmap now draws the alternates (mapping: plain -> alternate). In every GSUB single substitution that
    knows the plain form but not the alternate, the alternate gets the same rule: the feature that made it the
    default now swaps it back to the plain form, and the other features (the other g, the round dots...) still
    reach their forms from it. Returns the rules added, as 'alternate>target'."""
    gsub = font["GSUB"].table
    added = set()
    for lookup in gsub.LookupList.Lookup:
        for table in lookup.SubTable:
            if lookup.LookupType == 7:
                table = table.ExtSubTable
            single = getattr(table, "mapping", None)
            if not single or table.LookupType != 1:
                continue
            for plain, alt in mapping.items():
                if plain in single and alt not in single:
                    target = single[plain]
                    single[alt] = plain if target == alt else target
                    added.add(f"{alt}>{single[alt]}")
    return sorted(added)


def soften_targets(font: TTFont) -> set[str]:
    """Letters and figures and their alternates, simple outlines only. Marks, punctuation and symbols keep their
    corners (their shapes are small and the corners are what make them read), and so do SHARP glyphs."""
    cmap = font.getBestCmap()
    names = {name for code, name in cmap.items() if unicodedata.category(chr(code))[0] in "LN"}
    bases = {name.split(".", 1)[0] for name in names}
    hmtx, glyf = font["hmtx"], font["glyf"]
    out = set()
    for name in font.getGlyphOrder():
        if name.split(".", 1)[0] not in bases or name in SHARP or hmtx[name][0] == 0:
            continue
        if any(part in name for part in (".numr", ".dnom", "superior", "inferior")):
            continue  # fraction and super/subscript figures: too small at text size for a round to show
        glyph = glyf[name]
        if glyph.isComposite() or glyph.numberOfContours <= 0:
            continue
        out.add(name)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    work = common.ROOT / "build" / "out" / "sans"
    faces = [build(name, style, stem, out, work) for name, style, stem in FACES]
    report = {
        "family": FAMILY,
        "numbers": {"x_height": X_HEIGHT, "tracking": TRACKING, "soften": SOFTEN, "default_features": DEFAULT_FEATURES, "tail": TAIL},
        "faces": faces,
    }
    path = common.ROOT / "build" / "out" / "sans.json"
    path.write_text(json.dumps(report, indent=1, default=str) + "\n")
    for face in faces:
        print(f"{face['file']}: {face['woff2_before']} -> {face['woff2_after']} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
