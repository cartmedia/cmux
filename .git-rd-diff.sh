#!/usr/bin/env bash
# Scratch for one fleet job: which source lines of VirtualTranscript.tsx React Doctor flags for refs, at
# head and at base, by text (line numbers move). Not committed.
set -uo pipefail
F=webviews/src/agent-session/acpmux/VirtualTranscript.tsx
flagged() {
  python3 scripts/ci/react_doctor_gate.py --changed-from 6c98c3bd0ebdf1dcfd9353ce4e12183b1be87300 --out-dir /tmp/rd-$1 >/tmp/rd-$1.log 2>&1
  grep -oE 'VirtualTranscript.tsx:[0-9]+ +react-hooks-js/refs' /tmp/rd-$1.log | grep -oE ':[0-9]+' | tr -d : |
    while read -r n; do sed -n "${n}p" "$F" | sed -E 's/^ +//'; done | sort | uniq -c | sort -k2 >/tmp/rd-$1.txt
  grep -E 'VirtualTranscript.tsx::react-hooks-js/refs' /tmp/rd-$1.log
}
flagged head
cp "$F" /tmp/vt-head.tsx
git checkout 6c98c3bd0ebdf1dcfd9353ce4e12183b1be87300 -- "$F" webviews/src/agent-session/acpmux/conversation/keptLayouts.ts 2>/dev/null || git checkout 6c98c3bd0ebdf1dcfd9353ce4e12183b1be87300 -- "$F"
flagged base
echo "== lines flagged at head, not (or less) at base"
diff /tmp/rd-base.txt /tmp/rd-head.txt | grep -E '^[<>]'
