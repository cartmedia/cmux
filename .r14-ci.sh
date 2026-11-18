set -u
OUT="${FAST_CHECKS_OUT:-/tmp/r14out}"; mkdir -p "$OUT"
cd cmux-tui
echo "== head: tests"
cargo test -p cmux-tui-core --lib -- page_relay names_tests untrusted_mint event_bus 2>&1 | grep -E "^test |test result|panicked|error|FAILED|assert" | tail -60
head_tests=${PIPESTATUS[0]}
cargo test -p cmux-tui-cloud-conversations --lib 2>&1 | grep -E "test result|error" | tail -5
echo "== head: bench"
for i in 1 2 3; do
  cargo test --release -p cmux-tui-core --lib page_relay_fanout_bench -- --ignored --nocapture 2>&1 | grep -E "PAGE_RELAY_BENCH|panicked|error\[" | sed 's/^/head /'
done
echo "== base: same harness (base files from .r14-base, the new test file kept)"
cd ..
cp -R .r14-base/. .
rm -f cmux-tui/crates/cmux-tui-core/src/server/cmd_subscribe_names_tests.rs
printf '\n#[cfg(all(test, unix))]\n#[path = "page_relay_events_tests.rs"]\nmod page_relay_events_tests;\n' >> cmux-tui/crates/cmux-tui-core/src/server/client_hello.rs
grep -c subscribe_where cmux-tui/crates/cmux-tui-core/src/server/cmd_subscribe.rs
cd cmux-tui
cargo test -p cmux-tui-core --lib -- a_page_relay_mailbox_takes_only_its_page_events 2>&1 | grep -E "^test |test result|panicked|error|left|right|must take" | tail -25
base_detector=${PIPESTATUS[0]}
for i in 1 2 3; do
  cargo test --release -p cmux-tui-core --lib page_relay_fanout_bench -- --ignored --nocapture 2>&1 | grep -E "PAGE_RELAY_BENCH|panicked|error\[" | sed 's/^/base /'
done
echo "head_tests_exit=$head_tests base_detector_exit=$base_detector"
[ "$head_tests" = 0 ] && [ "$base_detector" != 0 ]
