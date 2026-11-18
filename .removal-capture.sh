set -euo pipefail
# usage in cmux-ci: VARIANTS env: "entry|variant|width|height ..."
cd webviews && bun install --frozen-lockfile >/dev/null && CMUX_GALLERY_OUT_DIR=$PWD/../dist-gallery bun run gallery:build >/dev/null
cd ../scripts/gallery-matrix && bun install --frozen-lockfile >/dev/null
eval "$(../ci/ensure-system-chrome.sh | grep ^CMUX_CHROME_HEADLESS_SHELL= | sed "s/^/export /")" || true
python3 - <<'PY'
import json
cases=[]
for spec in [
  ("pages.settings","new-tab-template-open",900,760),
  ("pages.settings","new-tab",900,760),
  ("agent-pane.new-tab","style-switch",760,560),
  ("agent-pane.new-tab","template-classic",760,560),
  ("agent-pane.new-tab","minimal-default",760,560),
]:
  for scheme in ("light","dark"):
    e,v,w,h=spec
    cases.append({"id":f"{e}--{v}--{scheme}","path_or_url":"frame.html","params":{"entry":e,"variant":v,"locale":"en","colorScheme":scheme,"width":w,"height":h}})
json.dump(cases,open("/tmp/removal-manifest.json","w"),indent=1)
PY
CMUX_BROWSER_TESTS=1 CMUX_GALLERY_IN_VM=1 bun runner.ts --manifest /tmp/removal-manifest.json --gallery-dir ../../dist-gallery --output-dir "$FAST_CHECKS_OUT/shots" --engines chromium || echo "runner exit $?"
ls -R "$FAST_CHECKS_OUT" | head -50
