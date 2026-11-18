set -uo pipefail
# Temporary fleet check for the removal lane (not committed): format, print the format patch as
# base64, then the webview checks and the targeted tests.
cd webviews
bun install --frozen-lockfile >/dev/null 2>&1
git -C .. add -A >/dev/null 2>&1 || true
git -C .. stash list >/dev/null 2>&1 || true
before=$(git -C .. diff --cached --binary | sha1sum)
npx --no-install vp fmt >/dev/null 2>&1 || bunx vp fmt >/dev/null 2>&1 || echo "fmt could not run"
echo "=====FMT-PATCH-BEGIN"
git -C .. diff --binary | base64 -w 0; echo
echo "=====FMT-PATCH-END"
echo "== typecheck"; bun run typecheck 2>&1 | tail -80; echo "typecheck exit ${PIPESTATUS[0]}"
echo "== lint"; bun run lint 2>&1 | tail -40
for piece in check:catalog-keys check:catalog-unused check:ui-review check:browser-lanes check:pane-selectors check:fixed-waits check:whole-list-rows check:layers; do
  echo "== $piece"; bun run "$piece" 2>&1 | tail -15
done
echo "== tests"
bun test \
  test/new-tab-retired-templates.test.ts \
  test/new-tab-minimal-no-chat-work.test.tsx \
  test/new-tab-minimal-default.test.tsx \
  test/new-tab-no-widgets.test.tsx \
  test/new-tab-style-switch.test.tsx \
  test/new-tab-recent-sessions.test.ts \
  test/new-tab-shared-chats.test.tsx \
  test/new-tab-activity-time.test.ts \
  test/new-tab-composer-layout.test.tsx \
  test/new-tab-recent-project.test.tsx \
  test/new-tab-now-playing.test.tsx \
  test/composer-combinations.test.tsx \
  test/chat-rows-open-on-press.test.tsx \
  test/hover-cards-glass.test.ts \
  src/agent-session/acpmux/newtab/NewTabScreen.test.tsx \
  src/agent-session/acpmux/newtab/screenActions.test.ts \
  src/agent-session/acpmux/newtab/chatOpenFeedback.test.tsx \
  src/agent-session/acpmux/layoutWatch.test.ts \
  2>&1 | grep -E "^\(fail\)|^ *[0-9]+ (pass|fail)|error:|Error:|✗" | head -80
echo "== all unit tests"
bun test 2>&1 | grep -E "^\(fail\)|^ *[0-9]+ (pass|fail)|^Ran " | head -80
