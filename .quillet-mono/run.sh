#!/usr/bin/env bash
# Fleet wrapper: install fontTools, build Quillet Mono, print outputs as gzip+base64 lines.
set -u
cd "$(dirname "$0")"
export PATH=$HOME/.local/bin:$HOME/.cargo/bin:$PATH
PY=""
if command -v uv >/dev/null 2>&1; then
  PY="uv run --no-project --with fonttools[woff]==4.60.1 --with brotli==1.1.0 python3"
else
  python3 -m venv /tmp/qmono-venv && /tmp/qmono-venv/bin/pip -q install "fonttools[woff]==4.60.1" brotli==1.1.0 && PY=/tmp/qmono-venv/bin/python
fi
$PY src/probe_mono.py; $PY src/mono.py --out out-fonts
rc=$?
for f in out-fonts/quillet-mono.woff2 out-fonts/quillet-mono-italic.woff2 build/out/mono.json; do
  [ -f "$f" ] && echo "QMONO-FILE $(basename $f) $(gzip -9c "$f" | base64 -w0)"
done
if [ -n "${FAST_CHECKS_OUT:-}" ]; then cp out-fonts/*.woff2 build/out/mono.json "$FAST_CHECKS_OUT"/ 2>/dev/null; fi
echo "QMONO-RC $rc"
exit $rc
