#!/bin/bash
# Runs in the fleet checkout (base + bench + after.patch). Same box: base runs, then the patch, then after runs.
set -u
cd webviews
OUT="${FAST_CHECKS_OUT:-/tmp/nt-bench}"; mkdir -p "$OUT"
echo "box: $(nproc) cpus; $(uname -srm)"; grep -m1 "model name" /proc/cpuinfo || true
for i in 1 2 3; do bun test test/new-tab-template-scale.test.tsx 2>&1 | sed "s/^BENCH /BENCH base /" | tee -a "$OUT/base.log"; done
cd .. && git apply --exclude=webviews/test/new-tab-template-scale.test.tsx bench-after.patch && cd webviews || { echo "PATCH FAILED"; exit 1; }
for i in 1 2 3; do bun test test/new-tab-template-scale.test.tsx 2>&1 | sed "s/^BENCH /BENCH after /" | tee -a "$OUT/after.log"; done
# Bundle sizes: after side now (patched tree), then revert and build base.
size() { echo "SIZE $1 main.mjs $(wc -c < "$2/main.mjs") total $(find "$2" -type f \( -name '*.mjs' -o -name '*.js' -o -name '*.css' \) -exec cat {} + | wc -c) gzip_main $(gzip -9c "$2/main.mjs" | wc -c)" | tee -a "$OUT/size.log"; }
node scripts/pages/gen-strings.mjs >/dev/null && CMUX_WEBVIEWS_OUT_DIR=/tmp/nt-dist-after npx vp build >/tmp/nt-build-after.log 2>&1 && size after /tmp/nt-dist-after || { echo "BUILD after FAILED"; tail -20 /tmp/nt-build-after.log; }
cd .. && git apply -R --exclude=webviews/test/new-tab-template-scale.test.tsx bench-after.patch && cd webviews
node scripts/pages/gen-strings.mjs >/dev/null && CMUX_WEBVIEWS_OUT_DIR=/tmp/nt-dist-base npx vp build >/tmp/nt-build-base.log 2>&1 && size base /tmp/nt-dist-base || { echo "BUILD base FAILED"; tail -20 /tmp/nt-build-base.log; }
