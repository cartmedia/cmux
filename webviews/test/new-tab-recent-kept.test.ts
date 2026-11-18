// A New Tab page keeps its recent chats and their children as the chat list changes
// (newTabModel.ts `RecentSessions`, screenModel.ts `KeptChatCards`) instead of reading every chat
// on each event: Leo, 2026-11-18, "typing in the composer still slow 😞 there must be a lot of open
// stuff?" (bench/session-scale-typing.bench.ts). Whatever the changes, it shows what a full read shows.
import { expect, test } from "bun:test";
import { RecentSessions, recentSessions } from "../src/agent-session/acpmux/newTabModel";
import { KeptChatCards, recentChatCards } from "../src/agent-session/acpmux/newtab/screenModel";
import { noteReplaced } from "../src/agent-session/acpmux/sessionIndex";
import { type AcpmuxSessionEntry, sessionEntry } from "../src/agent-session/acpmux/sessionList";

/// A small seeded generator, so a failure replays.
function random(seed: number) {
  let state = seed;
  return (below: number) => {
    state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
    return state % below;
  };
}

const INDICATORS = [undefined, "working", "input", "done", "failed"] as const;

function chat(index: number, fields: Record<string, unknown> = {}): AcpmuxSessionEntry {
  return sessionEntry({
    sessionId: `s${index}`,
    title: `Chat ${index}`,
    harness: "claude",
    status: "idle",
    cwd: `/Users/me/code/p${index % 7}`,
    updatedAt: 1_000 + index,
    ...(index % 4 === 1 ? { spawnedBy: `s${index - 1}` } : {}),
    ...fields,
  });
}

function replaced(list: readonly AcpmuxSessionEntry[], at: number, entry: AcpmuxSessionEntry) {
  const next = list.slice();
  next[at] = entry;
  noteReplaced(list, next, [at]);
  return next;
}

/// One chat changing as chats do: mostly working now, now and then asking, done, archived, unsent,
/// an older time, the same time as another, or started by another chat.
function changed(roll: (below: number) => number, list: readonly AcpmuxSessionEntry[], at: number, clock: number) {
  const index = Number(list[at]!.sessionId.slice(1));
  const old = list[at]!;
  const kind = roll(20);
  const base = {
    updatedAt: old.updatedAt,
    ...(old.spawnedBy ? { spawnedBy: old.spawnedBy } : {}),
    ...(old.indicator ? { indicator: old.indicator } : {}),
  };
  if (kind < 9) return chat(index, { ...base, updatedAt: clock, indicator: INDICATORS[roll(INDICATORS.length)] });
  if (kind < 11) return chat(index, { ...base, indicator: "input" });
  if (kind < 12) return chat(index, { ...base, archived: roll(2) === 0 });
  if (kind < 13) return chat(index, { ...base, unsent: roll(2) === 0 });
  if (kind < 14) return chat(index, { ...base, updatedAt: 500 + roll(400) });
  if (kind < 15) return chat(index, { ...base, updatedAt: list[roll(list.length)]!.updatedAt });
  if (kind < 16) return chat(index, { ...base, spawnedBy: roll(2) === 0 ? undefined : `s${roll(list.length)}` });
  if (kind < 17) return chat(index, { ...base, pendingPermissions: roll(2) });
  return chat(index, { ...base, title: `Renamed ${clock}` });
}

for (const [count, fold] of [
  [6, true],
  [6, false],
  [1, true],
  [40, true],
] as const)
  test(`recent chats (${count}, ${fold ? "folded" : "flat"}) are a full read's through any change`, () => {
    for (const seed of [1, 2, 3]) {
      const roll = random(seed);
      let clock = 5_000;
      let list: readonly AcpmuxSessionEntry[] = Array.from({ length: 120 }, (_, index) => chat(index));
      const kept = new RecentSessions();
      for (let step = 0; step < 1_500; step++) {
        expect(kept.of(list, count, fold).map((entry) => entry.sessionId)).toEqual(
          recentSessions(list, count, fold).map((entry) => entry.sessionId),
        );
        const at = roll(list.length);
        list =
          step % 97 === 96
            ? [...list, chat(list.length + 1_000, { updatedAt: (clock += 1) })]
            : replaced(list, at, changed(roll, list, at, (clock += 1 + roll(2))));
      }
    }
  });

test("the cards and their children are a full read's through any change", () => {
  const roll = random(7);
  let clock = 5_000;
  let list: readonly AcpmuxSessionEntry[] = Array.from({ length: 150 }, (_, index) => chat(index));
  const kept = new KeptChatCards();
  for (let step = 0; step < 1_500; step++) {
    expect(recentChatCards(list, 9_000, undefined, [], 8, true, kept)).toEqual(
      recentChatCards(list, 9_000, undefined, [], 8, true),
    );
    const at = roll(list.length);
    list = replaced(list, at, changed(roll, list, at, (clock += 1)));
  }
});

test("hands back the same chats while no shown one changes", () => {
  let list: readonly AcpmuxSessionEntry[] = Array.from({ length: 50 }, (_, index) => chat(index));
  const kept = new RecentSessions();
  const first = kept.of(list);
  // An old chat far below the shown ones renamed: the same array.
  list = replaced(list, 2, chat(2, { title: "Renamed" }));
  expect(kept.of(list)).toBe(first);
});
