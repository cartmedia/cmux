set -u
cd webviews
V=src/agent-session/acpmux/VirtualTranscript.tsx
cp $V vt.fix.tmp
echo "== bails fix"; NO_SUPPRESSIONS= node scripts/list-bails.tmp.mjs $V src/agent-session/acpmux/mountEffect.ts
bun run typecheck 2>&1 | tail -5; echo "typecheck exit ${PIPESTATUS[0]}"
bunx vp lint src/agent-session/acpmux/VirtualTranscript.tsx src/agent-session/acpmux/mountEffect.ts 2>&1 | tail -5
for arm in base fix base fix; do
  cp vt.$arm.tmp $V
  echo "== ARM $arm"
  bun bench/render-counts.bench.ts 300 --only key,token,switch,show,resize 2>&1 | grep -E "^(RENDERS|WHY|\|)" | head -40
done
cp vt.fix.tmp $V
echo "== tests"; bun test $(grep -rlE "VirtualTranscript|RowFrame|sendMorph|entrance|loadFade|older" test | grep -E "\.test\.tsx?$" | head -60) 2>&1 | tail -15
