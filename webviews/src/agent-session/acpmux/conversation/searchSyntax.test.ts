// l10n-allow-file: fixture prompts, not shipped UI.
// Find in Chat's search operators on the page side (searchSyntax.ts): reading the daemon's answers,
// editing the query by the spans it gives (a chip's removal, a suggestion, a quick pick), and
// matching the loaded rows with its reading (chatFind.ts findMatches with a pattern). The page
// parses nothing itself: a query the daemon has not read is one literal needle, as before.
import { describe, expect, test } from "bun:test";
import type { AcpmuxRow } from "../model";
import { findMatches, findPaintRows, patternFilters, patternHits } from "./chatFind";
import { readAnswer } from "./findIndex";
import {
  acceptCompletion,
  applyPick,
  findPattern,
  folderOperator,
  pickOn,
  QUICK_PICKS,
  readParsed,
  readSyntax,
  removeChip,
} from "./searchSyntax";
import { CHIPS_QUERY, PICKS_QUERY, cannedSyntax } from "./searchSyntaxFixture";

const row = (id: string, kind: AcpmuxRow["kind"], text: string, at = 1_000): AcpmuxRow => ({
  id,
  version: 1,
  at,
  kind,
  text,
});

describe("reading the daemon's answers", () => {
  test("search_syntax: parsed and completions, spans checked against the query", () => {
    const answer = readSyntax(
      {
        parsed: {
          terms: [{ text: "deploy", phrase: false }, { text: "" }],
          exclude: ["flaky", 3],
          role: "assistant",
          agent: "codex",
          in: "this",
          folder: "cmux",
          sinceMs: 10,
          beforeMs: 20,
          chips: [
            { kind: "from", value: "agent", label: "From agent", start: 7, end: 17 },
            { kind: "from", value: "agent", label: "Out of range", start: 7, end: 99 },
            { kind: "nonsense", value: "x", label: "x", start: 0, end: 1 },
          ],
          problems: ["since: needs a value"],
        },
        completions: [
          { insert: "from:", label: "from:", detail: "Who said it", start: 0, end: 2 },
          { insert: "", label: "empty", detail: "", start: 0, end: 2 },
        ],
      },
      "deploy from:agent",
    );
    expect(answer).toEqual({
      parsed: {
        terms: [{ text: "deploy", phrase: false }],
        exclude: ["flaky"],
        role: "assistant",
        agent: "codex",
        in: "this",
        folder: "cmux",
        sinceMs: 10,
        beforeMs: 20,
        chips: [{ kind: "from", value: "agent", label: "From agent", start: 7, end: 17 }],
        problems: ["since: needs a value"],
      },
      completions: [{ insert: "from:", label: "from:", detail: "Who said it", start: 0, end: 2 }],
    });
  });

  test("any other shape is no answer, and an older transcript_search answer has no parsed", () => {
    expect(readSyntax({ error: "x" }, "q")).toBeUndefined();
    expect(readSyntax(undefined, "q")).toBeUndefined();
    expect(readParsed({ chips: [] }, "q")).toBeUndefined();
    expect(readAnswer({ hits: [] }, "q")).toEqual({ hits: [] });
    expect(readAnswer({ hits: [], parsed: cannedSyntax(PICKS_QUERY).parsed }, PICKS_QUERY)?.parsed?.chips).toHaveLength(2);
  });
});

describe("editing the query by the daemon's spans", () => {
  test("a chip comes out in one click and the spaces it leaves are tidied", () => {
    const chips = cannedSyntax(CHIPS_QUERY).parsed.chips;
    const [phrase, exclude, from, since] = chips;
    expect(removeChip(CHIPS_QUERY, phrase!)).toBe("deploy -flaky from:agent since:7d");
    expect(removeChip(CHIPS_QUERY, exclude!)).toBe('deploy "exact phrase" from:agent since:7d');
    expect(removeChip(CHIPS_QUERY, from!)).toBe('deploy "exact phrase" -flaky since:7d');
    expect(removeChip(CHIPS_QUERY, since!)).toBe('deploy "exact phrase" -flaky from:agent');
    expect(removeChip("from:me", { start: 0, end: 7 })).toBe("");
    expect(removeChip("  from:me   retry ", { start: 2, end: 9 })).toBe("retry ");
  });

  test("UTF-16 spans: a chip after an emoji comes out whole", () => {
    const query = "🚀 launch from:me";
    expect(query.slice(10, 17)).toBe("from:me");
    expect(removeChip(query, { start: 10, end: 17 })).toBe("🚀 launch");
  });

  test("a suggestion replaces its token and puts the caret after it", () => {
    expect(acceptCompletion("fr", { insert: "from:", label: "from:", detail: "", start: 0, end: 2 })).toEqual({
      query: "from:",
      cursor: 5,
    });
    expect(
      acceptCompletion("retry since:t tail", { insert: "since:today ", label: "", detail: "", start: 6, end: 13 }),
    ).toEqual({ query: "retry since:today  tail", cursor: 18 });
  });

  test("a quick pick appends its operator, replaces the chip of its kind, and comes out when on again", () => {
    const chips = cannedSyntax(PICKS_QUERY).parsed.chips;
    expect(pickOn(chips, PICKS_QUERY, QUICK_PICKS.fromMe.operator)).toBe(true);
    expect(pickOn(chips, PICKS_QUERY, QUICK_PICKS.fromAgent.operator)).toBe(false);
    // On again: out.
    expect(applyPick(PICKS_QUERY, chips, QUICK_PICKS.fromMe.kinds, QUICK_PICKS.fromMe.operator)).toBe("retry since:today");
    // Another of its kind: in its place.
    expect(applyPick(PICKS_QUERY, chips, QUICK_PICKS.fromAgent.kinds, QUICK_PICKS.fromAgent.operator)).toBe(
      "retry from:agent since:today",
    );
    expect(applyPick(PICKS_QUERY, chips, QUICK_PICKS.week.kinds, QUICK_PICKS.week.operator)).toBe("retry from:me since:7d");
    // None of its kind: at the end.
    expect(applyPick(PICKS_QUERY, chips, QUICK_PICKS.codex.kinds, QUICK_PICKS.codex.operator)).toBe(
      "retry from:me since:today agent:codex",
    );
    expect(applyPick("", [], ["from"], "from:me")).toBe("from:me");
  });

  test("a folder name becomes folder:NAME, quoted when it holds a space", () => {
    expect(folderOperator(" cmux ")).toBe("folder:cmux");
    expect(folderOperator("my app")).toBe('folder:"my app"');
    expect(folderOperator('"')).toBeUndefined();
    expect(folderOperator("  ")).toBeUndefined();
  });
});

describe("matching the loaded rows with the daemon's reading", () => {
  const rows = [
    row("u1", "user", "Can you deploy the exact phrase fix?", 1_000),
    row("a1", "assistant", "I will deploy it. The exact phrase check passes.", 2_000),
    row("a2", "assistant", "The deploy failed: the flaky exact phrase test.", 3_000),
    row("a3", "assistant", "Deploy only, nothing else.", 4_000),
  ];

  test("every term in one message, none excluded, of the role and time asked", () => {
    const parsed = { ...cannedSyntax(CHIPS_QUERY, CHIPS_QUERY.length, 10_000).parsed, sinceMs: 1_500 };
    const matches = findMatches(rows, findPattern(CHIPS_QUERY, parsed, false));
    // Only a1: u1 is a prompt, a2 holds flaky, a3 lacks the phrase.
    expect(matches.map((match) => [match.rowId, match.occurrence, match.term])).toEqual([
      ["a1", 0, "deploy"],
      ["a1", 1, "exact phrase"],
    ]);
    // Before since: nothing.
    expect(findMatches(rows, findPattern(CHIPS_QUERY, { ...parsed, sinceMs: 2_500 }, false))).toEqual([]);
    expect(findMatches(rows, findPattern(CHIPS_QUERY, { ...parsed, beforeMs: 2_000 }, false))).toEqual([]);
  });

  test("case-sensitive matches the text as written", () => {
    const parsed = { terms: [{ text: "Deploy", phrase: false }], exclude: [], chips: [], problems: [] };
    expect(findMatches(rows, findPattern("Deploy", parsed, false)).map((match) => match.rowId)).toEqual([
      "u1",
      "a1",
      "a2",
      "a3",
    ]);
    expect(findMatches(rows, findPattern("Deploy", parsed, true)).map((match) => match.rowId)).toEqual(["a3"]);
  });

  test("until the daemon answers, the raw text is one literal needle, as before", () => {
    expect(findMatches(rows, findPattern("deploy the", undefined, false)).map((match) => match.rowId)).toEqual(["u1"]);
    expect(findMatches(rows, "deploy the")).toEqual(findMatches(rows, findPattern("deploy the", undefined, false)));
    expect(patternFilters(findPattern("deploy the", undefined, false))).toBe(false);
  });

  test("several terms' places in order, an overlap kept once", () => {
    expect(patternHits("abcabc", ["bc", "abc"])).toEqual([
      { at: 0, length: 3, term: 1 },
      { at: 3, length: 3, term: 1 },
    ]);
    expect(patternHits("x deploy y", ["y", "deploy"])).toEqual([
      { at: 2, length: 6, term: 1 },
      { at: 9, length: 1, term: 0 },
    ]);
  });

  test("a filtering pattern paints only the drawn rows its matches are in", () => {
    const fold: AcpmuxRow = { ...row("a1:fold", "assistant", ""), id: "a1:fold" };
    const run: AcpmuxRow = { ...row("run", "assistant", ""), joined: ["a1"] };
    const painted = findPaintRows([...rows, fold, run], [{ rowIndex: 1, rowId: "a1", occurrence: 0 }]);
    expect([...painted].sort()).toEqual(["a1", "a1:fold", "run"]);
  });
});
