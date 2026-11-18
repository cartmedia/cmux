// l10n-allow-file: canned daemon answers (its own English labels), test and gallery scaffolding.
// Canned `_acpmux/search_syntax` answers, in the daemon's shape (acpmux-transcript-search syntax.rs),
// for the mock daemon (mock.ts), the gallery and the tests. Not a parser: a query not listed here
// reads as one literal term, as Find matched before operators.
import type { SyntaxAnswer } from "./searchSyntax";

const DAY = 24 * 60 * 60_000;

/** The chips query of the gallery: a word, a phrase, an exclusion and two filters. */
export const CHIPS_QUERY = 'deploy "exact phrase" -flaky from:agent since:7d';
/** The options row's query: two picks on. */
export const PICKS_QUERY = "retry from:me since:today";
/** A query whose filters leave nothing. */
export const EMPTY_QUERY = "migration from:agent since:today";

const empty = { exclude: [], chips: [], problems: [] };

/** The daemon's answer for `query` at `cursor`, from the canned shapes; `now` and the local zone
 * place `since:`. */
export function cannedSyntax(query: string, cursor = query.length, now = Date.now()): SyntaxAnswer {
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const today = midnight.getTime();
  switch (query) {
    case CHIPS_QUERY:
      return {
        parsed: {
          terms: [
            { text: "deploy", phrase: false },
            { text: "exact phrase", phrase: true },
          ],
          exclude: ["flaky"],
          role: "assistant",
          sinceMs: now - 7 * DAY,
          chips: [
            { kind: "phrase", value: "exact phrase", label: '"exact phrase"', start: 7, end: 21 },
            { kind: "exclude", value: "flaky", label: "Not flaky", start: 22, end: 28 },
            { kind: "from", value: "agent", label: "From agent", start: 29, end: 39 },
            { kind: "since", value: "7d", label: "Since 7d", start: 40, end: 48 },
          ],
          problems: [],
        },
        completions: [],
      };
    case PICKS_QUERY:
      return {
        parsed: {
          ...empty,
          terms: [{ text: "retry", phrase: false }],
          role: "user",
          sinceMs: today,
          chips: [
            { kind: "from", value: "me", label: "From me", start: 6, end: 13 },
            { kind: "since", value: "today", label: "Since today", start: 14, end: 25 },
          ],
        },
        completions: [],
      };
    case EMPTY_QUERY:
      return {
        parsed: {
          ...empty,
          terms: [{ text: "migration", phrase: false }],
          role: "assistant",
          sinceMs: today,
          chips: [
            { kind: "from", value: "agent", label: "From agent", start: 10, end: 20 },
            { kind: "since", value: "today", label: "Since today", start: 21, end: 32 },
          ],
        },
        completions: [],
      };
    case "fr":
    case "f":
      return {
        parsed: { ...empty, terms: [{ text: query, phrase: false }] },
        completions:
          cursor === 0
            ? []
            : [
                { insert: "from:", label: "from:", detail: "Who said it: me or agent", start: 0, end: query.length },
                ...(query === "f"
                  ? [{ insert: "folder:", label: "folder:", detail: "The chat's folder holds this", start: 0, end: 1 }]
                  : []),
              ],
      };
    case "from:":
      return {
        parsed: { ...empty, terms: [{ text: "from:", phrase: false }], problems: ["from: needs a value (me, agent or tool)"] },
        completions: [
          { insert: "from:me ", label: "from:me", detail: "My prompts", start: 0, end: 5 },
          { insert: "from:agent ", label: "from:agent", detail: "The agent's replies", start: 0, end: 5 },
          { insert: "from:tool ", label: "from:tool", detail: "Tool calls and output", start: 0, end: 5 },
        ],
      };
    default:
      return { parsed: { ...empty, terms: query.trim() ? [{ text: query, phrase: false }] : [] }, completions: [] };
  }
}
