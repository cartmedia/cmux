// Find in Chat's search operators (Leo, 2026-11-18: "I know that VS Code has like a decent kind of
// searching thing with arguments and whatnot, likewise with Obsidian"). The daemon owns the query
// language: `_acpmux/search_syntax` (acpmux-transcript-search syntax.rs) parses the text into terms,
// exclusions, filters and the chips the bar draws, and suggests operators for the token at the
// cursor; `_acpmux/transcript_search` answers the same `parsed` beside its hits. This module only
// reads those answers and edits the query text by the spans they give (UTF-16, a text field's own
// offsets), so the text is always the truth. It is not a parser: until the daemon has answered, a
// query is one literal needle, as Find matched before operators.
import { foldFind } from "../../../find/fold";

/** A chip's kind, as the daemon names it. */
export type SearchChipKind = "phrase" | "exclude" | "from" | "agent" | "in" | "folder" | "since" | "before";

/** One recognized piece of the query: it covers `query.slice(start, end)`. */
export type SearchChip = { kind: SearchChipKind; value: string; label: string; start: number; end: number };

/** An operator suggestion: `insert` replaces `query.slice(start, end)`. */
export type SearchCompletion = { insert: string; label: string; detail: string; start: number; end: number };

export type SearchRole = "user" | "assistant" | "tool";

/** What the daemon read in a query (syntax.rs `Parsed::to_json`). */
export type ParsedQuery = {
  terms: { text: string; phrase: boolean }[];
  exclude: string[];
  role?: SearchRole;
  agent?: string;
  in?: "this" | "all";
  folder?: string;
  sinceMs?: number;
  beforeMs?: number;
  chips: SearchChip[];
  problems: string[];
};

/** `_acpmux/search_syntax`'s answer. */
export type SyntaxAnswer = { parsed: ParsedQuery; completions: SearchCompletion[] };

/** What Find asks it: the query, the caret in it, and the local zone for `since:today`. */
export type SyntaxAsk = { query: string; cursor?: number; tzOffsetMinutes?: number };

const CHIP_KINDS = new Set<string>(["phrase", "exclude", "from", "agent", "in", "folder", "since", "before"]);
const ROLES = new Set<string>(["user", "assistant", "tool"]);

const num = (value: unknown) => (typeof value === "number" && Number.isFinite(value) ? value : undefined);
const text = (value: unknown) => (typeof value === "string" ? value : undefined);

/** Minutes this machine's local time is ahead of UTC, as the daemon reads `tzOffsetMinutes`. */
export const localTzOffsetMinutes = () => -new Date().getTimezoneOffset();

/** A span the daemon gave, kept only when it lies in a query of `length`. */
function span(raw: Record<string, unknown>, length: number): { start: number; end: number } | undefined {
  const start = num(raw.start);
  const end = num(raw.end);
  if (start === undefined || end === undefined || start < 0 || end < start || end > length) return undefined;
  return { start, end };
}

/** `parsed` as Find reads it, its spans checked against `query`; undefined for any other shape. */
export function readParsed(raw: unknown, query: string): ParsedQuery | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const value = raw as Record<string, unknown>;
  if (!Array.isArray(value.terms)) return undefined;
  const terms = value.terms.flatMap((term) => {
    const read = text((term as { text?: unknown })?.text);
    return read ? [{ text: read, phrase: (term as { phrase?: unknown }).phrase === true }] : [];
  });
  const exclude = Array.isArray(value.exclude)
    ? value.exclude.filter((each): each is string => typeof each === "string" && each !== "")
    : [];
  const chips: SearchChip[] = [];
  for (const each of Array.isArray(value.chips) ? value.chips : []) {
    const chip = each as Record<string, unknown>;
    const kind = text(chip.kind);
    const at = span(chip, query.length);
    if (!kind || !CHIP_KINDS.has(kind) || !at) continue;
    chips.push({ kind: kind as SearchChipKind, value: text(chip.value) ?? "", label: text(chip.label) ?? "", ...at });
  }
  const role = text(value.role);
  const scope = text(value.in);
  return {
    terms,
    exclude,
    ...(role && ROLES.has(role) && { role: role as SearchRole }),
    ...(text(value.agent) && { agent: text(value.agent) }),
    ...((scope === "this" || scope === "all") && { in: scope }),
    ...(text(value.folder) && { folder: text(value.folder) }),
    ...(num(value.sinceMs) !== undefined && { sinceMs: num(value.sinceMs) }),
    ...(num(value.beforeMs) !== undefined && { beforeMs: num(value.beforeMs) }),
    chips,
    problems: Array.isArray(value.problems)
      ? value.problems.filter((each): each is string => typeof each === "string")
      : [],
  };
}

/** `_acpmux/search_syntax`'s answer for `query`; undefined for any other shape. */
export function readSyntax(raw: unknown, query: string): SyntaxAnswer | undefined {
  const parsed = readParsed((raw as { parsed?: unknown } | undefined)?.parsed, query);
  if (!parsed) return undefined;
  const list = (raw as { completions?: unknown }).completions;
  const completions: SearchCompletion[] = [];
  for (const each of Array.isArray(list) ? list : []) {
    const item = each as Record<string, unknown>;
    const insert = text(item.insert);
    const at = span(item, query.length);
    if (!insert || !at) continue;
    completions.push({ insert, label: text(item.label) || insert, detail: text(item.detail) ?? "", ...at });
  }
  return { parsed, completions };
}

/** `query` with `query.slice(start, end)` replaced by `insert`, and the caret after it. */
export function replaceSpan(
  query: string,
  start: number,
  end: number,
  insert: string,
): { query: string; cursor: number } {
  return { query: query.slice(0, start) + insert + query.slice(end), cursor: start + insert.length };
}

/** `query` without `chip`'s text: the spaces it leaves are one, and none at the ends. */
export function removeChip(query: string, chip: Pick<SearchChip, "start" | "end">): string {
  const before = query.slice(0, chip.start).replace(/\s+$/, "");
  const after = query.slice(chip.end).replace(/^\s+/, "");
  return before && after ? `${before} ${after}` : before || after;
}

/** A completion accepted: its text in place of the token it was for. */
export const acceptCompletion = (query: string, completion: SearchCompletion) =>
  replaceSpan(query, completion.start, completion.end, completion.insert);

/** A quick pick of the options row: the operator text it puts in the query. */
export type SearchPick = { kinds: readonly SearchChipKind[]; operator: string };

/** The options row's picks, as the operator each writes (the query text stays the truth). */
export const QUICK_PICKS = {
  fromMe: { kinds: ["from"], operator: "from:me" },
  fromAgent: { kinds: ["from"], operator: "from:agent" },
  claude: { kinds: ["agent"], operator: "agent:claude" },
  codex: { kinds: ["agent"], operator: "agent:codex" },
  today: { kinds: ["since"], operator: "since:today" },
  week: { kinds: ["since"], operator: "since:7d" },
} as const satisfies Record<string, SearchPick>;

/** `folder:NAME`, quoted when the name holds a space or a quote would end it. */
export function folderOperator(name: string): string | undefined {
  const trimmed = name.trim().replace(/"/g, "");
  if (!trimmed) return undefined;
  return /\s/.test(trimmed) ? `folder:"${trimmed}"` : `folder:${trimmed}`;
}

/** Whether `pick`'s operator is the one the query's chips already hold. */
export const pickOn = (chips: readonly SearchChip[] | undefined, query: string, operator: string) =>
  Boolean(chips?.some((chip) => foldFind(query.slice(chip.start, chip.end)) === foldFind(operator)));

/**
 * The query after a quick pick: on again it comes out; else it replaces the chip of its kind the
 * daemon found (one `from:` at a time), else it goes at the end. The spans are the daemon's, so this
 * reads no syntax of its own.
 */
export function applyPick(
  query: string,
  chips: readonly SearchChip[],
  kinds: readonly SearchChipKind[],
  operator: string,
): string {
  const same = chips.find((chip) => foldFind(query.slice(chip.start, chip.end)) === foldFind(operator));
  if (same) return removeChip(query, same);
  const other = chips.find((chip) => kinds.includes(chip.kind));
  if (other) return replaceSpan(query, other.start, other.end, operator).query;
  const trimmed = query.replace(/\s+$/, "");
  return trimmed ? `${trimmed} ${operator}` : operator;
}

/** What Find matches in the loaded rows: the terms each row must hold (all of them), the ones it
 * must not, the role it must be, and its time span. `key` names it, for caches. */
export type FindPattern = {
  terms: readonly string[];
  exclude: readonly string[];
  role?: SearchRole;
  sinceMs?: number;
  beforeMs?: number;
  caseSensitive: boolean;
  key: string;
};

/** The pattern of `query`: the daemon's reading when it has one, else the whole text as one
 * literal needle (as Find matched before operators, and with an acpmux that has no syntax). */
export function findPattern(query: string, parsed: ParsedQuery | undefined, caseSensitive: boolean): FindPattern {
  if (!parsed) {
    const terms = query ? [query] : [];
    return { terms, exclude: [], caseSensitive, key: `${caseSensitive ? "C" : "c"}\u0000${query}` };
  }
  const terms = parsed.terms.map((term) => term.text).filter(Boolean);
  const key = [
    caseSensitive ? "C" : "c",
    terms.join("\u0001"),
    parsed.exclude.join("\u0001"),
    parsed.role ?? "",
    parsed.sinceMs ?? "",
    parsed.beforeMs ?? "",
  ].join("\u0000");
  return {
    terms,
    exclude: parsed.exclude,
    ...(parsed.role && { role: parsed.role }),
    ...(parsed.sinceMs !== undefined && { sinceMs: parsed.sinceMs }),
    ...(parsed.beforeMs !== undefined && { beforeMs: parsed.beforeMs }),
    caseSensitive,
    key,
  };
}

/** No pattern: nothing matches. */
export const NO_PATTERN: FindPattern = { terms: [], exclude: [], caseSensitive: false, key: "" };
