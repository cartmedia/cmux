"""A/B harness (not committed): one pool run's executed test IDs, from its log.

Every pool command other than the prepush gate lines (by command text; executed = the pool printed its verdict
group), plus every prepush gate unittest case from the -v output inside those commands' groups
('test_x (module.Class.test_x) ... ok|FAIL|ERROR', the verdict on that line or the next). Skipped cases count as
skipped, not executed. Writes the sorted IDs to OUT and prints 'IDS label count skipped sha256 red=[...]'.
usage: ids.py LOG LABEL OUT"""
import hashlib
import re
import sys
from pathlib import Path

log, label, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
group = re.compile(r"^::group::(.*): (ok|failed \(exit \d+\)|timed out at the pool deadline after \d+s), \d+s$")
case = re.compile(r"^(test_\w+) \(([\w.]+)\)")
verdict = re.compile(r"\.\.\. (ok|FAIL|ERROR|skipped|expected failure|unexpected success)")
gate = "tests/test_cmux_next_prepush_gate.py"
ids, skipped, red, cur = set(), 0, [], None
lines = log.read_text(errors="replace").splitlines()
for i, line in enumerate(lines):
    m = group.match(line)
    if m:
        cur = m.group(1)
        if gate not in cur:
            ids.add("cmd::" + cur)
        if m.group(2) != "ok":
            red.append(cur)
        continue
    if line == "::endgroup::":
        cur = None
        continue
    if cur is None or gate not in cur:
        continue
    c = case.match(line)
    if not c:
        continue
    v = verdict.search(line) or (verdict.search(lines[i + 1]) if i + 1 < len(lines) else None)
    if v and v.group(1) == "skipped":
        skipped += 1
    else:
        ids.add("prepush_gate::" + c.group(2).replace("__main__.", ""))
text = "".join(f"{t}\n" for t in sorted(ids))
out.write_text(text)
print(f"IDS {label} {len(ids)} {skipped} {hashlib.sha256(text.encode()).hexdigest()} red={red}", flush=True)
