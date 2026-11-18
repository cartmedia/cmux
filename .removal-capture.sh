set -euo pipefail
# Gallery shots for the PR; the fleet keeps no files, so each PNG is printed into the log as base64.
OUT="${FAST_CHECKS_OUT:-$PWD/removal-out}"
mkdir -p "$OUT/shots"
cd webviews && bun install --frozen-lockfile >/dev/null && CMUX_GALLERY_OUT_DIR=$PWD/../dist-gallery bun run gallery:build >/dev/null 2>&1
cd ../scripts/gallery-matrix && bun install --frozen-lockfile >/dev/null
eval "$(../ci/ensure-system-chrome.sh | grep ^CMUX_CHROME_HEADLESS_SHELL= | sed "s/^/export /")" || true
python3 - <<'PY'
import json
cases=[]
for spec in [
  ("pages.settings","new-tab-template-open",900,760),
  ("agent-pane.new-tab","style-switch",760,560),
  ("agent-pane.new-tab","template-classic",760,560),
  ("agent-pane.new-tab","minimal-default",760,560),
]:
  for scheme in ("light","dark"):
    e,v,w,h=spec
    cases.append({"id":f"{e}--{v}--{scheme}","path_or_url":"frame.html","params":{"entry":e,"variant":v,"locale":"en","colorScheme":scheme,"width":w,"height":h}})
json.dump(cases,open("/tmp/removal-manifest.json","w"),indent=1)
PY
CMUX_BROWSER_TESTS=1 CMUX_GALLERY_IN_VM=1 bun runner.ts --manifest /tmp/removal-manifest.json --gallery-dir ../../dist-gallery --output-dir "$OUT/shots" --engines chromium >/dev/null 2>&1 || echo "runner exit $?"
find "$OUT" -name '*.png' | sort | while read -r f; do
  echo "=====PNG-BEGIN $(basename "$f")"
  base64 -w 0 "$f"; echo
  echo "=====PNG-END"
done
