// One-off replay (not committed): each commit's change through the base's touched.ts and this branch's.
import fs from "node:fs";
import { manifestCases } from "./manifest";
import { loadEntries } from "./entries";
import { catalogTables } from "./catalogs";
import { loadEntryRoots, touchedEntries } from "./touched";
import { touchedEntries as oldTouched } from "./web-route-touched-old";

const [input, output] = process.argv.slice(2);
const commits = JSON.parse(fs.readFileSync(input!, "utf8")) as {
  sha: string;
  when: string;
  changed: string[];
  catalogKeys: Record<string, string[] | null>;
}[];
const memo = new Map<string, string | undefined>();
const read = (file: string) => {
  if (!memo.has(file)) {
    try {
      memo.set(file, fs.statSync(file).isFile() ? fs.readFileSync(file, "utf8") : undefined);
    } catch {
      memo.set(file, undefined);
    }
  }
  return memo.get(file);
};
const roots = await loadEntryRoots();
const entries = await loadEntries();
const full = manifestCases(entries, { frame: "component" }).length;
const cases = (ids: string[]) => (ids.length ? manifestCases(entries, { frame: "component", entries: ids }).length : 0);
const rows = [];
for (const commit of commits) {
  const fake = (keys: string[] | null | undefined, side: "head" | "base") =>
    keys === null ? "not json" : JSON.stringify({ strings: side === "head" ? Object.fromEntries((keys ?? []).map((k) => [k, 1])) : {} });
  const catalogs = catalogTables(
    commit.changed,
    (p) => fake(commit.catalogKeys[p], "head"),
    (p) => fake(commit.catalogKeys[p], "base"),
  );
  const t0 = performance.now();
  const before = oldTouched(roots, commit.changed, { read });
  const after = touchedEntries(roots, commit.changed, { catalogs, read });
  rows.push({
    sha: commit.sha,
    when: commit.when,
    changed: commit.changed,
    old: { all: before.all, entries: before.entries, reasons: before.reasons, cases: cases(before.entries) },
    new: {
      all: after.all,
      entries: after.entries,
      reasons: after.reasons,
      inert: after.inert,
      cases: cases(after.entries),
      hostFrame: after.ranked.filter((r) => r.why === "host-frame").length,
    },
    ms: Math.round(performance.now() - t0),
  });
}
fs.writeFileSync(output!, JSON.stringify({ full, rows }));
console.log(`replayed ${rows.length} commits; a full render is ${full} cases`);
