#!/usr/bin/env python3
"""No org route reaches GitHub-hosted runners, or Linux over 8 vCPU, without a declared reason.

Leo, 2026-11-18: "why are some things ONLY linux anyways if we can simply just do the linux parts on our
machines". That night Linux CI starved: Blacksmith caps the organization's vCPUs at once (32 vCPU started 1 of
282 jobs from 00:00 to 00:48Z), and GitHub-hosted ubuntu-24.04 had about 4,300 jobs queued behind the GitHub Free
plan's 20 hosted jobs at once, shared by the whole organization, while 59 of our Macs stood idle. Jobs reached
both through routes nobody saw: a `github.run_attempt == 1 && 'ubuntu-24.04'` first attempt, an allowlist that
let a variable pick `ubuntu-24.04`, an overflow arm that upsized 4 vCPU work to 16 or 32 vCPU, and pools whose
stall fallback was a bigger size. The pattern page is cmuxterm-hq
docs/handbook/hosted-fallback-lands-on-a-capped-pool.md.

A job's org route is its `runs-on` once the fork-only arm (`github.repository_owner != 'manaflow-ai' && 'x' ||`,
which runs in a fork's own account) is set aside. Every quoted runner label in it counts (an allowlist's labels
included: the variable may pick any of them), and so does every member of each pool it reads through
`fromJSON(vars.RUNNER_POOLS).<pool>` (.github/runners.json `pools`), and every label of the job's
`strategy.matrix` when the route reads `matrix.*`. A fork pull request's literal (`head.repo.full_name !=
github.repository && 'x'`) runs in this organization, so it counts. Variable values (`vars.LINUX_RUNNER`) and
picker outputs (`needs.*`) are not labels written here and are not seen; their writers hold them to the pools
(scripts/ci/runner_pools.py, scripts/ci/cloud_overflow_switch.py, ci-repo-variables.yml).

The test fails:

(a) a route that can reach a GitHub-hosted label (ubuntu-*, windows-*, macos-<version>, macos-latest), unless
    that label comes only from a pool marked `"hosted": true` with a `hosted_why`, or the job declares
    `# github-hosted-required: <reason>` on its runs-on line or in the comment block right above it. The
    reason must be one GitHub-hosted runners alone can serve: GitHub OIDC for npm provenance, PyPI trusted
    publishing or artifact attestations, or a job that judges Blacksmith and so cannot run on it.
(b) a route that can reach a Linux label over 8 vCPU, unless every such label comes from
    - a pool whose `why` names a measurement ("measured") and says its jobs need "more than 8 vCPU", or
    - a job that declares `# size-reason: <measurement or need>` on its runs-on line or in the comment
      block right above it, or
    - on feat-cmux-next, the job's .github/runners.json `linux_sizes` entry with a `size` over 8 and a
      `measured` map (its measurement is the reason).
(c) a pool whose stall fallback is bigger than its first member while the first is 8 vCPU or less: Blacksmith
    caps vCPUs at once, so a small size that stalls is the cap, and asking for a bigger one asks for more of
    what is gone. A small pool falls back to the same size or smaller.

Kept-on-Linux reasons for jobs that could move to our Macs live in the pull request that moved the rest
(`# linux: <reason>` where a job states one); this test checks only the two routes above.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
RUNNERS = ROOT / ".github" / "runners.json"

# Workflows another open change owns. Each names who, and why it is not checked here yet.
SKIPPED_FILES = {
    # feat-cmux-next only: cmux#45853 moves the NEXT nightly's path through it and adds
    # tests/test_ci_nightly_path_keeps_its_size.py; the rest of its routes fold into that test.
    "cmux-next.yml": "owned by the nightly-path change (cmux#45853, tests/test_ci_nightly_path_keeps_its_size.py)",
}

FORK_OWNER = re.compile(r"github\.repository_owner\s*!=\s*'manaflow-ai'\s*&&\s*'[^']*'\s*\|\|\s*")
QUOTED = re.compile(r"""['"]([A-Za-z0-9_.\-]+)['"]""")
RUNNER_LIKE = re.compile(r"^(?:blacksmith-|ubuntu-|windows-|macos-)")
HOSTED = re.compile(r"^(?:ubuntu-(?:\d[\d.]*|latest)(?:-arm)?|windows-(?:\d+|latest)|macos-(?:\d+(?:-\w+)?|latest))$")
LINUX_SIZE = re.compile(r"^blacksmith-(\d+)vcpu-ubuntu-\d+(?:-arm)?$")
POOL_READ = re.compile(r"fromJSON\(\s*vars\.RUNNER_POOLS(?:\s*\|\|\s*'\{\}')?\s*\)\.([A-Za-z0-9_]+)")
JOB_HEADER = re.compile(r"^  ([A-Za-z0-9_-]+):\s*(?:#.*)?$")
RUNS_ON = re.compile(r"^    runs-on:\s*(.*)$")
HOSTED_REASON = re.compile(r"#\s*github-hosted-required:\s*\S")
SIZE_REASON = re.compile(r"#\s*size-reason:\s*\S")


def vcpu(label: str) -> int:
    match = LINUX_SIZE.match(label)
    return int(match.group(1)) if match else 0


def runners() -> dict:
    return json.loads(RUNNERS.read_text(encoding="utf-8"))


def pools(data: dict) -> dict[str, dict]:
    return {name: pool for name, pool in (data.get("pools") or {}).items() if isinstance(pool, dict)}


def declares_need(pool: dict) -> bool:
    why = str(pool.get("why", "")).lower()
    return "measured" in why and "more than 8 vcpu" in why


def route_labels(runs_on: object, job: dict, declared: dict[str, dict]) -> dict[str, set[str]]:
    """label -> where it came from: 'literal' or 'pool:<name>'."""
    text = runs_on if isinstance(runs_on, str) else json.dumps(runs_on)
    text = FORK_OWNER.sub("", text)
    if "matrix." in text:
        text += " " + json.dumps((job.get("strategy") or {}).get("matrix") or {})
    labels: dict[str, set[str]] = {}
    for pool in POOL_READ.findall(text):
        for member in (declared.get(pool) or {}).get("members", []):
            labels.setdefault(member, set()).add(f"pool:{pool}")
    for token in QUOTED.findall(POOL_READ.sub("", text)):
        if RUNNER_LIKE.match(token):
            labels.setdefault(token, set()).add("literal")
    return labels


def problems(workflows: dict[str, str], data: dict) -> list[str]:
    """workflows: file name -> text."""
    declared = pools(data)
    sizes = (data.get("linux_sizes") or {}).get("jobs") or {}
    out: list[str] = []
    for name, text in sorted(workflows.items()):
        if name in SKIPPED_FILES:
            continue
        document = yaml.safe_load(text) or {}
        comments = _comments_from_text(text)
        for job_name, job in ((document.get("jobs") or {}).items()):
            if not isinstance(job, dict) or "runs-on" not in job:
                continue
            key = f"{name}:{job_name}"
            note = comments.get(job_name, "")
            labels = route_labels(job["runs-on"], job, declared)
            for label, sources in sorted(labels.items()):
                if HOSTED.match(label):
                    hosted_ok = all(s.startswith("pool:") and declared[s[5:]].get("hosted")
                                    and str(declared[s[5:]].get("hosted_why", "")).strip() for s in sources)
                    if not hosted_ok and not HOSTED_REASON.search(note):
                        out.append(f"{key} can run on GitHub-hosted {label} ({', '.join(sorted(sources))}) with no "
                                   "hosted pool and no `# github-hosted-required: <reason>`")
                size = vcpu(label)
                if size > 8:
                    entry = sizes.get(f"{name}#{job_name}") or {}
                    measured = isinstance(entry.get("measured"), dict) and int(entry.get("size") or 0) > 8
                    for source in sorted(sources):
                        if source.startswith("pool:") and declares_need(declared[source[5:]]):
                            continue
                        if SIZE_REASON.search(note) or measured:
                            continue
                        out.append(f"{key} can run on {label} ({source}) with no declared reason: a pool whose why "
                                   "names its measurement and says its jobs need more than 8 vCPU, a "
                                   "`# size-reason: ...` comment, or a linux_sizes size over 8")
    return out


def _comments_from_text(text: str) -> dict[str, str]:
    found: dict[str, str] = {}
    lines = text.split("\n")
    in_jobs, job = False, None
    for index, line in enumerate(lines):
        if line.startswith("jobs:"):
            in_jobs = True
            continue
        if in_jobs and line and not line.startswith((" ", "#")):
            in_jobs = False
        if not in_jobs:
            continue
        header = JOB_HEADER.match(line)
        if header:
            job = header.group(1)
            continue
        if job and RUNS_ON.match(line):
            block = [line]
            above = index - 1
            while above >= 0 and lines[above].strip().startswith("#"):
                block.append(lines[above])
                above -= 1
            found[job] = "\n".join(block)
    return found


def pool_problems(data: dict) -> list[str]:
    out = []
    for name, pool in sorted(pools(data).items()):
        members = [m for m in pool.get("members", []) if vcpu(m)]
        if len(members) < 2:
            continue
        first = vcpu(members[0])
        if first <= 8 and any(vcpu(m) > first for m in members[1:]):
            out.append(f"pool {name} falls back from {members[0]} to a bigger size {members[1:]}; a small pool "
                       "falls back to the same size or smaller")
    return out


def checkout_workflows() -> dict[str, str]:
    return {path.name: path.read_text(encoding="utf-8") for path in sorted(WORKFLOWS.glob("*.y*ml"))}


class RuleTests(unittest.TestCase):
    POOLS = {"pools": {
        "linux": {"why": "x", "members": ["blacksmith-4vcpu-ubuntu-2404", "blacksmith-2vcpu-ubuntu-2404"]},
        "linux_16": {"why": "measured at 16 vCPU: each needs more than 8 vCPU",
                     "members": ["blacksmith-16vcpu-ubuntu-2404", "blacksmith-8vcpu-ubuntu-2404"]},
        "big_unmeasured": {"why": "big", "members": ["blacksmith-32vcpu-ubuntu-2404"]},
        "provenance": {"why": "x", "hosted": True, "hosted_why": "OIDC", "members": ["ubuntu-24.04"]},
        "hosted_no_why": {"why": "x", "hosted": True, "members": ["ubuntu-24.04"]},
    }}
    FORK = "github.repository_owner != 'manaflow-ai' && 'ubuntu-24.04' || "

    def check(self, runs_on: str, above: str = "") -> list[str]:
        text = f"jobs:\n  j:\n{above}    runs-on: {runs_on}\n    steps: []\n"
        return problems({"w.yml": text}, self.POOLS)

    def test_hosted_routes_fail(self) -> None:
        for runs_on in (
            "ubuntu-24.04",
            "ubuntu-latest",
            "${{ " + self.FORK + "github.run_attempt == 1 && 'ubuntu-24.04' || fromJSON(vars.RUNNER_POOLS).linux }}",
            "${{ " + self.FORK + "contains(fromJSON('[\"ubuntu-24.04\",\"blacksmith-2vcpu-ubuntu-2404\"]'), "
            "vars.CI_TRUSTED_RUNNER) && vars.CI_TRUSTED_RUNNER || 'blacksmith-4vcpu-ubuntu-2404' }}",
            "${{ " + self.FORK + "fromJSON(vars.RUNNER_POOLS).hosted_no_why }}",
            "${{ " + self.FORK + "'macos-26' }}",
        ):
            with self.subTest(runs_on=runs_on):
                self.assertTrue(self.check(runs_on))

    def test_fork_arm_hosted_pool_and_declared_reason_pass(self) -> None:
        self.assertEqual(self.check("${{ " + self.FORK + "fromJSON(vars.RUNNER_POOLS).linux }}"), [])
        self.assertEqual(self.check("${{ " + self.FORK + "fromJSON(vars.RUNNER_POOLS).provenance }}"), [])
        self.assertEqual(self.check("ubuntu-latest # github-hosted-required: npm provenance needs GitHub OIDC"), [])
        self.assertEqual(self.check("ubuntu-latest", "    # github-hosted-required: PyPI trusted publishing\n"), [])
        self.assertTrue(self.check("ubuntu-latest # github-hosted-required:"))

    def test_big_linux_routes_need_a_reason(self) -> None:
        self.assertTrue(self.check("blacksmith-32vcpu-ubuntu-2404"))
        self.assertTrue(self.check("${{ vars.CI_CLOUD_OVERFLOW_SAVED && 'blacksmith-16vcpu-ubuntu-2404' || "
                                   "'blacksmith-4vcpu-ubuntu-2404' }}"))
        self.assertTrue(self.check("${{ fromJSON(vars.RUNNER_POOLS).big_unmeasured }}"))
        fork_pr = ("github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != "
                   "github.repository && 'blacksmith-32vcpu-ubuntu-2404' || ")
        self.assertTrue(self.check("${{ " + fork_pr + "fromJSON(vars.RUNNER_POOLS).linux }}"))
        self.assertEqual(self.check("${{ fromJSON(vars.RUNNER_POOLS).linux_16 }}"), [])
        self.assertEqual(self.check("blacksmith-32vcpu-ubuntu-2404",
                                    "    # size-reason: an interactive cargo box\n"), [])
        sized = dict(self.POOLS, linux_sizes={"jobs": {"w.yml#j": {"size": 16, "measured": {"16": [1, 2, 3]}}}})
        text = "jobs:\n  j:\n    runs-on: ${{ " + fork_pr + "fromJSON(vars.RUNNER_POOLS).linux }}\n"
        self.assertEqual(problems({"w.yml": text}, sized), [])

    def test_small_pools_never_fall_back_up(self) -> None:
        self.assertTrue(pool_problems({"pools": {"linux": {"members": ["blacksmith-4vcpu-ubuntu-2404",
                                                                       "blacksmith-16vcpu-ubuntu-2404"]}}}))
        self.assertEqual(pool_problems(self.POOLS), [])


class CheckoutTests(unittest.TestCase):
    def test_no_route_reaches_hosted_or_big_linux_without_a_reason(self) -> None:
        self.assertEqual(problems(checkout_workflows(), runners()), [])

    def test_no_small_pool_falls_back_to_a_bigger_size(self) -> None:
        self.assertEqual(pool_problems(runners()), [])

    def test_every_hosted_pool_says_why(self) -> None:
        for name, pool in pools(runners()).items():
            if any(HOSTED.match(m) for m in pool.get("members", [])):
                with self.subTest(pool=name):
                    self.assertTrue(pool.get("hosted") and str(pool.get("hosted_why", "")).strip())

    def test_skipped_files_exist(self) -> None:
        names = {path.name for path in WORKFLOWS.glob("*.y*ml")}
        for name in SKIPPED_FILES:
            with self.subTest(workflow=name):
                self.assertIn(name, names)


if __name__ == "__main__":
    unittest.main()
