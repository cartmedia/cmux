// l10n-allow-file: fixture prompts, not shipped UI.
// Find in Chat's bar with a daemon that reads search operators (ChatFindBar.tsx over useChatFind,
// searchSyntax.ts): the Aa and filters toggles, the chips the daemon read and their one-click
// removal, the operator suggestions (arrows, Tab, Return, Escape first), the options row's picks
// writing operators into the text, the parse asks debounced with a stale answer dropped, the index
// asked with the raw text, caseSensitive and the local zone, and with an older acpmux the bar as it
// was: no toggles, no chips, the text one literal needle.
import { act } from "react";
import { afterAll, afterEach, describe, expect, test } from "bun:test";
import { installDom } from "../../../pages/settings/testDom";
import type { AcpmuxRow } from "../model";
import type { ChatFind } from "./chatFind";
import type { FindIndexSource, IndexSearch } from "./findIndex";
import type { SyntaxAnswer, SyntaxAsk } from "./searchSyntax";
import { CHIPS_QUERY, PICKS_QUERY, cannedSyntax } from "./searchSyntaxFixture";

const restore = installDom();
afterAll(() => restore());
const { createRoot } = await import("react-dom/client");
const { ChatFindBar } = await import("./ChatFindBar");
const { useChatFind } = await import("./chatFind");

const row = (id: string, kind: AcpmuxRow["kind"], text: string): AcpmuxRow => ({
  id,
  version: 1,
  at: Date.now() - 60_000,
  kind,
  text,
});
const rows = [
  row("u1", "user", "Can you deploy the exact phrase fix? Why does the retry stop?"),
  row("a1", "assistant", "I will deploy it now. The exact phrase check passes."),
  row("a2", "assistant", "The deploy failed: the flaky exact phrase test."),
];

type Source = { source: FindIndexSource; asks: SyntaxAsk[]; searches: IndexSearch[] };

/// A daemon index with the canned syntax (or none: an older acpmux); `hold` keeps syntax answers
/// until released, for the stale-answer case.
function daemon(options: { syntax?: boolean; hold?: boolean } = {}): Source & { release: () => void } {
  const asks: SyntaxAsk[] = [];
  const searches: IndexSearch[] = [];
  const held: (() => void)[] = [];
  const source: FindIndexSource = {
    key: "chat-1",
    search: (search) => {
      searches.push(search);
      return Promise.resolve({ hits: [], total: 0 });
    },
    holdsSeq: () => true,
    load: () => Promise.resolve(true),
    ...(options.syntax !== false && {
      syntax: (ask: SyntaxAsk) => {
        asks.push(ask);
        const answer = cannedSyntax(ask.query, ask.cursor);
        if (!options.hold) return Promise.resolve(answer);
        return new Promise<SyntaxAnswer>((resolve) => held.push(() => resolve(answer)));
      },
    }),
  };
  return { source, asks, searches, release: () => held.splice(0).forEach((go) => go()) };
}

const mounted: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const done of mounted.splice(0)) await done();
});

async function render(source: FindIndexSource) {
  const host = document.createElement("div");
  host.className = "acpmux-shell";
  document.body.append(host);
  const root = createRoot(host);
  const state: { find?: ChatFind } = {};
  function Pane() {
    const find = useChatFind(rows, source);
    state.find = find;
    return find.open ? <ChatFindBar find={find} /> : null;
  }
  await act(async () => root.render(<Pane />));
  mounted.push(async () => {
    await act(async () => root.unmount());
    host.remove();
  });
  return {
    host,
    get find() {
      return state.find!;
    },
    field: () => host.querySelector<HTMLInputElement>(".find-pill__field")!,
  };
}

/// Past the parse and index debounce (INDEX_DEBOUNCE_MS), with their answers in.
const settle = () => act(async () => new Promise((resolve) => setTimeout(resolve, 220)));

async function typeInto(field: HTMLInputElement, text: string) {
  await act(async () => {
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    set.call(field, text);
    field.setSelectionRange(text.length, text.length);
    field.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

const key = (field: HTMLInputElement, init: KeyboardEventInit) =>
  act(async () => field.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init })));

const click = (element: Element | null) => act(async () => (element as HTMLElement).click());

describe("Find in Chat with search operators", () => {
  test("the toggles show once the daemon reads operators; the pill keeps its parts", async () => {
    const { source, asks } = daemon();
    const view = await render(source);
    await act(async () => view.find.show());
    await settle();
    expect(asks[0]).toMatchObject({ query: "", cursor: 0 });
    expect(typeof asks[0]!.tzOffsetMinutes).toBe("number");
    const pill = view.host.querySelector(".find-pill")!;
    expect([...pill.children].map((part) => part.className.split(" ")[0])).toEqual([
      "find-pill__icon",
      "find-pill__field",
      "find-pill__count",
      "find-pill__separator",
      "find-pill__button",
      "find-pill__button",
      "find-pill__separator",
      "find-pill__button",
      "find-pill__button",
      "find-pill__button",
    ]);
    expect(pill.querySelector(".find-pill__button--text")?.textContent).toBe("Aa");
    expect(view.host.querySelector(".acpmux-find__filters")).not.toBeNull();
    expect(view.host.querySelector(".acpmux-find__chips")).toBeNull();
  });

  test("chips from the daemon; one click takes a chip's text out and tidies the spaces", async () => {
    const { source, searches } = daemon();
    const view = await render(source);
    await act(async () => view.find.show());
    await typeInto(view.field(), CHIPS_QUERY);
    await settle();
    const labels = [...view.host.querySelectorAll(".acpmux-find__chip-label")].map((chip) => chip.textContent);
    expect(labels).toEqual(['"exact phrase"', "Not flaky", "From agent", "Since 7d"]);
    // Local matching follows the reading: a1 only (u1 is a prompt, a2 holds flaky), at both terms.
    expect(view.find.matches.map((match) => [match.rowId, match.term])).toEqual([
      ["a1", "deploy"],
      ["a1", "exact phrase"],
    ]);
    // The index is asked with the raw text and the local zone.
    expect(searches.at(-1)).toMatchObject({ query: CHIPS_QUERY, tzOffsetMinutes: -new Date().getTimezoneOffset() });
    expect(searches.at(-1)?.caseSensitive).toBeUndefined();
    const remove = view.host.querySelectorAll(".acpmux-find__chip-remove")[2]!;
    await click(remove);
    expect(view.find.query).toBe('deploy "exact phrase" -flaky since:7d');
  });

  test("Aa: case counts in the loaded rows and in the index ask", async () => {
    const { source, searches } = daemon();
    const view = await render(source);
    await act(async () => view.find.show());
    await typeInto(view.field(), "The");
    await settle();
    expect(view.find.matches.length).toBe(5);
    await click(view.host.querySelector(".find-pill__button--text"));
    await settle();
    expect(view.host.querySelector(".find-pill__button--text")?.getAttribute("aria-pressed")).toBe("true");
    expect(view.find.matches.map((match) => match.rowId)).toEqual(["a1", "a2"]);
    expect(searches.at(-1)).toMatchObject({ query: "The", caseSensitive: true });
  });

  test("suggestions: arrows move, Escape closes the list before the bar, Tab takes one", async () => {
    const { source } = daemon();
    const view = await render(source);
    await act(async () => view.find.show());
    await typeInto(view.field(), "f");
    await settle();
    const options = () => [...view.host.querySelectorAll(".acpmux-find__suggestion")];
    expect(options().map((option) => option.querySelector(".acpmux-find__suggestion-label")?.textContent)).toEqual([
      "from:",
      "folder:",
    ]);
    expect(options()[0]!.getAttribute("aria-selected")).toBe("true");
    await key(view.field(), { key: "ArrowDown" });
    expect(options()[1]!.getAttribute("aria-selected")).toBe("true");
    await key(view.field(), { key: "Escape" });
    expect(options()).toHaveLength(0);
    expect(view.find.open).toBe(true);
    // A new text opens the list again; Tab takes the chosen one.
    await typeInto(view.field(), "fr");
    await settle();
    expect(options()).toHaveLength(1);
    await key(view.field(), { key: "Tab" });
    expect(view.find.query).toBe("from:");
    await settle();
    expect(options().map((option) => option.querySelector(".acpmux-find__suggestion-label")?.textContent)).toEqual([
      "from:me",
      "from:agent",
      "from:tool",
    ]);
    // Return takes one too, and does not step.
    await key(view.field(), { key: "Enter" });
    expect(view.find.query).toBe("from:me ");
    // With no list, Escape closes the bar.
    await settle();
    await key(view.field(), { key: "Escape" });
    expect(view.find.open).toBe(false);
  });

  test("the options row: picks write operators into the text and light up while it holds them", async () => {
    const { source } = daemon();
    const view = await render(source);
    await act(async () => view.find.show());
    await typeInto(view.field(), PICKS_QUERY);
    await settle();
    await click(view.host.querySelector(".acpmux-find__filters"));
    const picks = () => [...view.host.querySelectorAll<HTMLButtonElement>(".acpmux-find__pick")];
    expect(picks().map((pick) => pick.title)).toEqual([
      "from:me",
      "from:agent",
      "agent:claude",
      "agent:codex",
      "since:today",
      "since:7d",
    ]);
    expect(
      picks()
        .filter((pick) => pick.getAttribute("aria-pressed") === "true")
        .map((pick) => pick.title),
    ).toEqual(["from:me", "since:today"]);
    await click(picks()[1]!);
    expect(view.find.query).toBe("retry from:agent since:today");
    await click(picks()[3]!);
    expect(view.find.query).toBe("retry from:agent since:today agent:codex");
  });

  test("parse asks are debounced and a stale answer is dropped", async () => {
    const { source, asks, release } = daemon({ hold: true });
    const view = await render(source);
    await act(async () => view.find.show());
    await settle();
    release();
    await settle();
    await typeInto(view.field(), "d");
    await typeInto(view.field(), CHIPS_QUERY);
    await settle();
    // One ask for the settled text, none for the keystroke before it.
    expect(asks.map((ask) => ask.query)).toEqual(["", CHIPS_QUERY]);
    // An answer for an older text lands after a newer one was typed: dropped.
    await typeInto(view.field(), PICKS_QUERY);
    release();
    await settle();
    expect(view.find.syntax?.parsed).toBeUndefined();
    release();
    await settle();
    expect(view.find.syntax?.parsed?.chips.map((chip) => chip.label)).toEqual(["From me", "Since today"]);
  });

  test("an older acpmux: no toggles, no chips, the text one literal needle", async () => {
    const { source, searches } = daemon({ syntax: false });
    const view = await render(source);
    await act(async () => view.find.show());
    await typeInto(view.field(), "exact phrase");
    await settle();
    expect(view.host.querySelector(".find-pill__button--text")).toBeNull();
    expect(view.host.querySelector(".acpmux-find__filters")).toBeNull();
    expect(view.host.querySelector(".acpmux-find__chips")).toBeNull();
    expect(view.find.syntax).toBeUndefined();
    expect(view.find.matches.map((match) => match.rowId)).toEqual(["u1", "a1", "a2"]);
    expect(searches.at(-1)).toEqual({ query: "exact phrase", limit: 200 });
  });
});
