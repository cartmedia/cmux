#!/usr/bin/env bash
# A/B harness (not committed): the checks job's whole background pool, plain mode, as the job runs it
# (--jobs = the runner's width), A = base command list, B = patched list, interleaved A,B,... after one discarded
# warm-up. The fleet keeps only stdout, so each run prints its wall time and its executed-test-ID digest (ids.py),
# and the end prints every distinct ID list once ('ID <sha12> <id>').
set -uo pipefail
out="${FAST_CHECKS_OUT:-$PWD/ab-out}"; mkdir -p "$out"
pairs="${AB_PAIRS:-8}"
export GITHUB_WORKSPACE="${GITHUB_WORKSPACE:-$PWD}"
. scripts/ab-next-checks/deps.sh
command -v node >/dev/null || echo "harness: no node on this box"
jobs="${AB_JOBS:-16}"  # the 16 vCPU Linux runner most checks jobs take (mini_share 0.00); the job passes its nproc
echo "host $(hostname) nproc $(nproc) jobs $jobs budget=${CMUX_CI_CPU_BUDGET:-unset} $(python3 --version)"
git config --global --add safe.directory '*' 2>/dev/null || true
run() {  # arm label
  local arm="$1" label="$2" t0 t1 rc wall
  t0="$(date +%s.%N)"
  scripts/ci/run-checks-concurrently.sh --jobs "$jobs" < "scripts/ab-next-checks/$arm.txt" > "$out/$label.log" 2>&1; rc=$?
  t1="$(date +%s.%N)"
  wall="$(echo "$t1 - $t0" | bc)"
  printf '%s\t%s\t%s\t%s\n' "$label" "$arm" "$wall" "$rc" >> "$out/results.tsv"
  echo "RUN $label $arm $wall rc=$rc"
  python3 -I scripts/ab-next-checks/ids.py "$out/$label.log" "$label" "$out/$label.ids"
}
run a warmup
for i in $(seq 1 "$pairs"); do
  run a "a$i"
  run b "b$i"
done
cat "$out/results.tsv"
for f in "$out"/*.ids; do sha256sum "$f"; done | sort -u -k1,1 | while read -r sum f; do
  sed "s/^/ID ${sum:0:12} /" "$f"
done
