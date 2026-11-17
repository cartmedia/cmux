#!/bin/bash
# One-off fleet run (not committed): tests, a real gallery build with the route check, and the replay.
set -o pipefail
rc=0
(cd webviews && bun test test/gallery-touched-rank.test.ts test/gallery-route-catalogs.test.ts) || rc=1
python3 tests/test_ci_gallery_health_merge_ref.py > "$FAST_CHECKS_OUT/py.log" 2>&1 || rc=1
tail -25 "$FAST_CHECKS_OUT/py.log"
python3 tests/test_ci_warm_sources.py 2>&1 | tail -15 || rc=1
python3 tests/test_runs_on_labels.py 2>&1 | tail -8 || rc=1
cd webviews
CMUX_WEBVIEWS_METAFILE="$FAST_CHECKS_OUT/meta.json" CMUX_GALLERY_OUT_DIR=/tmp/web-route-gallery bun run gallery:build > "$FAST_CHECKS_OUT/build.log" 2>&1 || { tail -30 "$FAST_CHECKS_OUT/build.log"; rc=1; }
bun scripts/gallery/route-check.ts --metafile "$FAST_CHECKS_OUT/meta.json" --workflow ../.github/workflows/gallery-health.yml > "$FAST_CHECKS_OUT/route-check.txt" 2>&1
echo "route-check exit $?"; head -40 "$FAST_CHECKS_OUT/route-check.txt"
time bun scripts/gallery/web-route-replay.ts web-route-commits.json "$FAST_CHECKS_OUT/web-route-replay.json" || rc=1
exit $rc
