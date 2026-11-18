import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { transformSync } from "@babel/core";
import { REACT_COMPILER_TARGET } from "../reactCompiler.mjs";

/**
 * React Compiler bail ratchet for the agent chat (src/agent-session/acpmux). The compiler skips a
 * whole component or hook for one construct it cannot compile (a `try` with no `catch`, `??=`, a ref
 * written or read during render, a closure that reads a value declared below it), and says nothing at
 * build time: the component ships uncompiled, and every render of it re-creates every child's props.
 * The composer was such a component until this check came in: each key rendered its queue and re-wrote the `name`
 * and `type` of its hidden inputs (React re-writes them on every commit), 13 attribute writes a
 * dictated word.
 *
 * Each source file is compiled as the build compiles it (babel-plugin-react-compiler, React
 * `REACT_COMPILER_TARGET`), and each component or hook the compiler skips counts once. Files with
 * skips from before the rule have a count in compiler-bails-baseline.tsv, the known bails. A file
 * fails with more skips than its row (a file with no row may have none), and with fewer until the row
 * is lowered (`--update-baseline`), so the count only moves down. `--update-baseline` refuses to
 * raise a row. A row whose file is gone passes and is dropped on the next `--update-baseline`.
 *
 * React Doctor's `react-hooks-js/*` rules report the same diagnostics site by site against its own
 * baseline (scripts/ci/react-doctor-baseline.json); this counts what they cost, a function left
 * uncompiled, and runs with the other checks on every change.
 */

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const scope = path.join(root, "src/agent-session/acpmux");
const baselinePath = path.join(root, "scripts/compiler-bails-baseline.tsv");
const HEADER =
  "# Functions the React Compiler skips, per file, from before the rule (scripts/check-compiler-bails.mjs). Only moves down.\n";

/**
 * Each skipped function in `source` as `file:line reason`, one per function.
 * @param {string} file
 * @param {string} source
 * @returns {string[]}
 */
export function findBails(file, source) {
  /** @type {{ kind: string; fnLoc?: { start?: { line?: number } } | null; detail?: any }[]} */
  const events = [];
  transformSync(source, {
    filename: file,
    babelrc: false,
    configFile: false,
    parserOpts: { plugins: ["typescript", "jsx"] },
    plugins: [
      [
        "babel-plugin-react-compiler",
        { target: REACT_COMPILER_TARGET, ...(process.env.NO_SUPPRESSIONS ? { eslintSuppressionRules: [] } : {}), logger: { logEvent: (_, event) => events.push(event) } },
      ],
    ],
  });
  /** @type {Map<number, string>} */
  const skipped = new Map();
  for (const event of events) {
    if (event.kind !== "CompileError" && event.kind !== "PipelineError") continue;
    const at = event.fnLoc?.start?.line ?? 0;
    if (skipped.has(at)) continue;
    const reason = event.detail?.reason ?? event.detail?.options?.reason ?? "";
    skipped.set(at, String(reason).split("\n")[0].slice(0, 120));
  }
  return [...skipped].sort(([a], [b]) => a - b).map(([at, reason]) => `${path.relative(root, file)}:${at} ${reason}`);
}

/** @returns {Map<string, number>} */
function readBaseline() {
  if (!existsSync(baselinePath)) return new Map();
  return new Map(
    readFileSync(baselinePath, "utf8")
      .split("\n")
      .filter((line) => line && !line.startsWith("#"))
      .map((line) => {
        const [file, count] = line.split("\t");
        if (!file || !/^\d+$/.test(count ?? "")) throw new Error(`compiler-bails-baseline.tsv: bad row "${line}"`);
        return /** @type {[string, number]} */ ([file, Number(count)]);
      }),
  );
}

/** @param {Map<string, number>} counts */
function writeBaseline(counts) {
  const rows = [...counts].filter(([, count]) => count > 0).sort(([a], [b]) => a.localeCompare(b));
  writeFileSync(baselinePath, HEADER + rows.map(([file, count]) => `${file}\t${count}`).join("\n") + "\n");
}

/**
 * Files above their baseline row (with their skips) and below it; a row whose file is gone is neither.
 * @param {Map<string, string[]>} found
 * @param {Map<string, number>} baseline
 * @param {(file: string) => boolean} exists
 */
export function compareToBaseline(found, baseline, exists) {
  /** @type {string[]} */
  const grew = [];
  /** @type {string[]} */
  const shrank = [];
  for (const file of [...new Set([...found.keys(), ...baseline.keys()])].sort()) {
    const now = found.get(file)?.length ?? 0;
    const before = baseline.get(file) ?? 0;
    if (!found.has(file) && !exists(file)) continue;
    if (now > before)
      grew.push(`  ${file}: ${now} skipped, baseline ${before}\n    ${(found.get(file) ?? []).join("\n    ")}`);
    else if (now < before) shrank.push(`  ${file}: ${now} skipped, baseline ${before}`);
  }
  return { grew, shrank };
}
const files = process.argv.slice(2);
for (const f of files) for (const b of findBails(path.join(root, f), readFileSync(path.join(root, f), "utf8"))) console.log("BAIL " + b);
if (process.env.SCAN) {
  const tally = new Map();
  const walk = (dir) => { for (const e of readdirSync(dir, { withFileTypes: true })) { const f = path.join(dir, e.name); if (e.isDirectory()) walk(f); else if (/\.[jt]sx?$/.test(e.name) && !e.name.endsWith(".d.ts") && !/[._](?:test|spec|gallery|fixture)\.[jt]sx?$/.test(e.name)) for (const b of findBails(f, readFileSync(f, "utf8"))) { const reason = b.replace(/^\S+ /, "").replace(/\(BuildHIR[^)]*\) /, "").slice(0, 70); tally.set(reason, (tally.get(reason) ?? 0) + 1); } } };
  walk(path.join(root, process.env.SCAN));
  for (const [r, n] of [...tally].sort((a, b) => b[1] - a[1])) console.log(`TALLY ${n}\t${r}`);
}
