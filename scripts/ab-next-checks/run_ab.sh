#!/usr/bin/env bash
# A/B harness (not committed): the checks job's whole background pool, plain mode, as the job runs it
# (--jobs = online CPUs), A = base command list, B = patched list, interleaved A,B,... after one discarded warm-up.
set -uo pipefail
out="${FAST_CHECKS_OUT:-$PWD/ab-out}"; mkdir -p "$out"
pairs="${AB_PAIRS:-8}"
export GITHUB_WORKSPACE="${GITHUB_WORKSPACE:-$PWD}"
. scripts/ab-next-checks/deps.sh
command -v node >/dev/null || echo "harness: no node on this box"
jobs="${AB_JOBS:-16}"  # the 16 vCPU Linux runner most checks jobs take (mini_share 0.00); the job passes its nproc
{ echo "host $(hostname) cpus $jobs budget=${CMUX_CI_CPU_BUDGET:-unset}"; uname -a; python3 --version; git --version; } > "$out/env.txt"
git config --global --add safe.directory '*' 2>/dev/null || true
run() {  # arm label
  local arm="$1" label="$2" t0 t1 rc
  t0="$(date +%s.%N)"
  scripts/ci/run-checks-concurrently.sh --jobs "$jobs" < "scripts/ab-next-checks/$arm.txt" > "$out/$label.log" 2>&1; rc=$?
  t1="$(date +%s.%N)"
  printf '%s\t%s\t%s\t%s\n' "$label" "$arm" "$(echo "$t1 - $t0" | bc)" "$rc" >> "$out/results.tsv"
  tail -1 "$out/$label.log" | head -c 300; echo " [$label $(echo "$t1 - $t0" | bc)s rc=$rc]"
}
run a warmup
for i in $(seq 1 "$pairs"); do
  run a "a$i"
  run b "b$i"
done
cat "$out/results.tsv"
