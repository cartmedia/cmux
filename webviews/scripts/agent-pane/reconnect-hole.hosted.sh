#!/usr/bin/env bash
# Manual hosted runner for reconnect-hole.harness.ts: records the fix (the checkout as is), then
# the base (direct.ts from BASE_SHA, the same harness), into ${FAST_CHECKS_OUT:-$PWD/reconnect-hole}.
set -euo pipefail
base_sha="${BASE_SHA:-c74a1844964f93b165d5a5eb5cecfae74880ce9d}"
cd "$(git rev-parse --show-toplevel)/webviews"
out="${FAST_CHECKS_OUT:-$PWD/reconnect-hole}"
mkdir -p "$out"
git cat-file -e "$base_sha^{commit}" 2>/dev/null || git fetch --no-tags origin "$base_sha"
bun scripts/pages/gen-strings.mjs --best-effort
run() {
  set +e
  CMUX_BROWSER_TESTS=1 CMUX_RECONNECT_CAPTURE_DIR="$out" CMUX_RECONNECT_LABEL="$1" \
    bun scripts/agent-pane/reconnect-hole.harness.ts > "$out/$1.log" 2>&1
  local status=$?
  set -e
  printf 'RECONNECT_HOLE_%s_STATUS=%s\n' "$1" "$status"
  cat "$out/$1.log"
}
run after
git checkout "$base_sha" -- src/agent-session/acpmux/direct.ts
run before
git checkout HEAD -- src/agent-session/acpmux/direct.ts
ls -la "$out"
