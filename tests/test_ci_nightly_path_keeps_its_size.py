#!/usr/bin/env python3
"""The NEXT nightly's path through cmux-next.yml runs on our Macs first, and never on the outage upsize or hosted queue.

A NEXT nightly starts when nightly-next-tick.yml finds a feat-cmux-next commit whose cmux-next push run has a green
`Release compile` (scripts/cmux-next/request-nightly-next.sh), or when that run's request-nightly-next job asks. On
2026-11-17/18 no push run on feat-cmux-next got that far from about 19:17Z: each sat queued on two roots of that chain.
`portable job placement` and `cmux-next path routing` took GitHub-hosted ubuntu-24.04 on attempt 1, behind about
4,300 queued jobs. `cmux-next push head preflight`, `cmux-next macOS placement`, `cmux-next app compile placement` and
`cmux NEXT DEV daily tree wait` read `vars.CI_CLOUD_OVERFLOW_SAVED && RUNNER_POOLS.linux_16`. The overflow switch's
record stayed set from 19:48Z (cmux#45818), and linux_16 then named blacksmith-32vcpu-ubuntu-2404, which started 1 of
282 jobs from 00:00 to 00:48Z under Blacksmith's vCPU cap. The feed stayed at its 18:55Z build for over 7 hours
while 59 of our Macs stood idle.

Leo, 2026-11-18: "can we just push a nightly as higher priority? why are some things ONLY linux anyways if we can
simply just do the linux parts on our machines".

So, for every job on that path (PATH_ROOTS and every job they need, transitively), the trusted route (past the
fork literals):
1. reads nothing that moves it with the advisory crowd: no overflow upsize, no shared vars.LINUX_RUNNER, no
   `github.run_attempt == 1 && 'ubuntu-...'` hosted-first arm, no literal GitHub-hosted image, and no 16 or 32
   vCPU Linux pool;
2. starts on our Macs when it can reach a Linux runner at all: a feat-cmux-next push takes the nightly-only Macs first
   (OWNED_PUSH_ARM), or portable-placement places it on an idle mini first with small Blacksmith as the overflow
   (PLACED). Otherwise it names why it must stay on Linux in LINUX_ONLY.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "cmux-next.yml"

# The jobs a NEXT nightly waits on in a feat-cmux-next push run, besides every `Release compile` job.
PATH_ROOTS = ("request-nightly-next", "push-head-preflight", "portable-placement", "checks")
OWNED_PUSH_ARM = "github.event_name == 'push' && github.ref == 'refs/heads/feat-cmux-next' && 'glaeda-nightly-only-m4pro' || "
PLACED = re.compile(r"contains\(needs\.portable-placement\.outputs\.mini_jobs, ' [\w-]+ '\) && needs\.portable-placement\.outputs\.mini_runner")
# Jobs on the path that stay on (small) Linux Blacksmith. job -> why. A job leaves this list once it moves to a Mac.
LINUX_ONLY = {
    "release-compile-fast": "its result reads `timeout ... curl` (macOS has no timeout(1), and `|| true` hides the 127); "
                            "it already runs on the owned gateway Mac when macos-placement names one",
    "release-compile-revert": "ephemeral-required: holds a contents and pull-requests write App token",
    "compile-status": "ephemeral-required: holds a statuses write App token, and waits up to 58 min",
    "compile-status-handoff": "ephemeral-required: holds a statuses write App token, and waits up to 58 min",
    "macos-placement": "ephemeral-required: mints the CMUX_BOT and GLAEDA_ROUTE App tokens",
    "app-compile-placement": "ephemeral-required: mints the CMUX_BOT and GLAEDA_ROUTE App tokens",
    "dev-daily-place": "ephemeral-required: mints the GLAEDA_ROUTE App token",
    "dev-landing-place": "ephemeral-required: mints the GLAEDA_ROUTE App token",
}
SHARED_ROUTES = {
    "the overflow upsize": re.compile(r"vars\.CI_CLOUD_OVERFLOW_SAVED"),
    "the shared LINUX_RUNNER": re.compile(r"vars\.LINUX_RUNNER(?!\w)"),
    "the hosted-first arm": re.compile(r"github\.run_attempt\s*==\s*1\s*&&\s*'ubuntu-"),
    "a GitHub-hosted image": re.compile(r"'ubuntu-[\d.]+'|'ubuntu-latest'"),
    "a 16 or 32 vCPU Linux pool": re.compile(r"fromJSON\(vars\.RUNNER_POOLS\)\.linux_(16|32)\b|'blacksmith-(16|32)vcpu-ubuntu"),
}
REACHES_LINUX = re.compile(r"fromJSON\(vars\.RUNNER_POOLS\)\.linux|'blacksmith-\d+vcpu-ubuntu|'ubuntu-")
# The fork branches name a literal (a fork's run gets no variables); only the trusted tail is a route.
FORK_BRANCHES = (
    re.compile(r"github\.repository_owner != 'manaflow-ai' && '[^']+' \|\| "),
    re.compile(r"github\.event_name == 'pull_request' && github\.event\.pull_request\.head\.repo\.full_name"
               r" != github\.repository && '[^']+' \|\| "),
)


def trusted_route(runs_on: object) -> str:
    text = json.dumps(runs_on) if not isinstance(runs_on, str) else runs_on
    for branch in FORK_BRANCHES:
        text = branch.sub("", text)
    return re.sub(r"^\$\{\{\s*\(?", "", text)


def path_jobs(jobs: dict) -> list[str]:
    roots = [name for name, job in jobs.items()
             if name in PATH_ROOTS or "Release compile" in str((job or {}).get("name", ""))]
    seen: list[str] = []
    todo = list(roots)
    while todo:
        name = todo.pop(0)
        if name in seen or name not in jobs:
            continue
        seen.append(name)
        needs = jobs[name].get("needs") or []
        todo.extend([needs] if isinstance(needs, str) else needs)
    return seen


def problems(jobs: dict) -> list[str]:
    out = []
    names = path_jobs(jobs)
    missing = [root for root in PATH_ROOTS if root not in jobs]
    out += [f"{root}: a PATH_ROOTS job is gone from cmux-next.yml" for root in missing]
    for name in names:
        route = trusted_route(jobs[name].get("runs-on", ""))
        for why, pattern in SHARED_ROUTES.items():
            if pattern.search(route):
                out.append(f"cmux-next.yml#{name}: runs-on reads {why}; the nightly path takes its measured small pool")
        if not REACHES_LINUX.search(route):
            continue
        owned_first = route.startswith(OWNED_PUSH_ARM) or bool(PLACED.search(route))
        if owned_first and name in LINUX_ONLY:
            out.append(f"cmux-next.yml#{name}: starts on our Macs now; drop it from LINUX_ONLY")
        if not owned_first and name not in LINUX_ONLY:
            out.append(f"cmux-next.yml#{name}: can run on Linux Blacksmith or hosted without our Macs first; add "
                       f"the feat-cmux-next push arm or portable placement, or name its Linux-only reason in LINUX_ONLY")
    out += [f"LINUX_ONLY names {name}, which is not on the nightly path" for name in LINUX_ONLY if name not in names]
    return out


def load_jobs() -> dict:
    return yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))["jobs"]


class NightlyPathKeepsItsSize(unittest.TestCase):
    def test_the_nightly_path_starts_on_our_macs_and_never_on_the_upsize_or_hosted(self) -> None:
        self.assertEqual(problems(load_jobs()), [])

    def test_the_path_reaches_the_jobs_that_stalled_on_2026_11_18(self) -> None:
        names = set(path_jobs(load_jobs()))
        for name in ("portable-placement", "path_route", "push-head-preflight", "macos-placement",
                     "app-compile-placement", "checks", "release-compile", "request-nightly-next"):
            self.assertIn(name, names)

    def test_the_checker_flags_each_shared_route_and_an_unexplained_linux_job(self) -> None:
        tail = "fromJSON(vars.RUNNER_POOLS).linux_4 }}"
        cases = {
            "the overflow upsize": "${{ vars.CI_CLOUD_OVERFLOW_SAVED && fromJSON(vars.RUNNER_POOLS).linux_8 || " + tail,
            "the shared LINUX_RUNNER": "${{ vars.LINUX_RUNNER || " + tail,
            "the hosted-first arm": "${{ github.run_attempt == 1 && 'ubuntu-24.04' || " + tail,
            "a GitHub-hosted image": "${{ 'ubuntu-24.04' }}",
            "a 16 or 32 vCPU Linux pool": "${{ fromJSON(vars.RUNNER_POOLS).linux_16 }}",
            "without our Macs first": "${{ " + tail,
        }
        for why, runs_on in cases.items():
            with self.subTest(why=why):
                jobs = {root: {"runs-on": "${{ " + OWNED_PUSH_ARM + tail} for root in PATH_ROOTS}
                jobs["router"] = {"runs-on": runs_on}
                jobs["request-nightly-next"]["needs"] = "router"
                found = problems(jobs)
                self.assertTrue(any("#router" in line and why in line for line in found), found)

    def test_owned_first_routes_pass(self) -> None:
        tail = "fromJSON(vars.RUNNER_POOLS).linux_4 }}"
        jobs = {root: {"runs-on": "${{ github.repository_owner != 'manaflow-ai' && 'ubuntu-24.04' || " + OWNED_PUSH_ARM + tail}
                for root in PATH_ROOTS}
        jobs["checks"] = {"runs-on": "${{ contains(needs.portable-placement.outputs.mini_jobs, ' checks ') && "
                                     "needs.portable-placement.outputs.mini_runner || " + tail}
        self.assertEqual([line for line in problems(jobs) if "LINUX_ONLY names" not in line], [])


if __name__ == "__main__":
    unittest.main()
