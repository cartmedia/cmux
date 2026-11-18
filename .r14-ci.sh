set -u
BASE=38a83f2729ba2ccccc13dc152feae893366b7bd2
OUT="${FAST_CHECKS_OUT:-/tmp/r14out}"; mkdir -p "$OUT"
cd cmux-tui
echo "== head: tests"
cargo test -p cmux-tui-core --lib -- page_relay names_tests untrusted_mint event_bus 2>&1 | tail -40 | tee "$OUT/head-tests.txt"
head_tests=${PIPESTATUS[0]}
cargo test -p cmux-tui-cloud-conversations --lib 2>&1 | tail -5
echo "== head: bench"
for i in 1 2 3; do
  cargo test --release -p cmux-tui-core --lib page_relay_fanout_bench -- --ignored --nocapture 2>&1 | grep PAGE_RELAY_BENCH | tee -a "$OUT/head-bench.txt"
done
echo "== base: same harness"
cd ..
git diff --name-only "$BASE" -- cmux-tui Packages | grep -v 'page_relay_events_tests.rs$' | grep -v 'cmd_subscribe_names_tests.rs$' | xargs git checkout "$BASE" --
rm -f cmux-tui/crates/cmux-tui-core/src/server/cmd_subscribe_names_tests.rs
printf '\n#[cfg(all(test, unix))]\n#[path = "page_relay_events_tests.rs"]\nmod page_relay_events_tests;\n' >> cmux-tui/crates/cmux-tui-core/src/server/client_hello.rs
git status --short
cd cmux-tui
cargo test -p cmux-tui-core --lib -- a_page_relay_mailbox_takes_only_its_page_events 2>&1 | tail -25 | tee "$OUT/base-detector.txt"
base_detector=${PIPESTATUS[0]}
for i in 1 2 3; do
  cargo test --release -p cmux-tui-core --lib page_relay_fanout_bench -- --ignored --nocapture 2>&1 | grep PAGE_RELAY_BENCH | tee -a "$OUT/base-bench.txt"
done
echo "head_tests_exit=$head_tests base_detector_exit=$base_detector"
[ "$head_tests" = 0 ] && [ "$base_detector" != 0 ]
