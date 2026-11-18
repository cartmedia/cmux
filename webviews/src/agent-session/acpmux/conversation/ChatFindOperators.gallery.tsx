// l10n-allow-file: gallery fixtures (sample prompts and replies), not shipped UI.
// Find in Chat's search operators (searchSyntax.ts; Leo, 2026-11-18: "I know that VS Code has like a
// decent kind of searching thing with arguments and whatnot, likewise with Obsidian"): the glass
// find pill as it is, with its match-case (Aa) and filters toggles, and under it the operator
// suggestions, the options row and the chips the daemon read, over the real find state
// (useChatFind) and a stand-in daemon that answers `_acpmux/search_syntax` from canned shapes
// (searchSyntaxFixture.ts). The variants: the bar open and empty, a query with chips, the
// suggestions for `fr`, the options row with two picks on, chips with no results, and an acpmux
// without operators (the bar as it was).
import { useLayoutEffect, useMemo, useRef, type CSSProperties } from "react";
import { componentEntry } from "../../../gallery/format";
import type { PlayContext } from "../../../gallery/play";
import { assistant, user } from "../../../gallery/fixtures/acpmux";
import { ChatFindBar } from "./ChatFindBar";
import { clearFindHighlights, findPaintRows, paintFindHighlights, patternFilters, useChatFind } from "./chatFind";
import type { FindIndexSource } from "./findIndex";
import { CHIPS_QUERY, EMPTY_QUERY, PICKS_QUERY, cannedSyntax } from "./searchSyntaxFixture";

type Mode = "operators" | "absent";

const rows = [
  user("Can you deploy the exact phrase fix to staging? The retry banner still shows.", 40),
  assistant("I will deploy it now. The exact phrase check passes on every run.", 38),
  assistant("The deploy failed once: the flaky exact phrase test timed out on CI.", 30),
  user("Why does the retry loop give up after one try?", 12),
  assistant("Deployed to staging. The exact phrase banner shows on the settings page.", 8),
];

function indexFor(mode: Mode): FindIndexSource {
  return {
    key: "gallery-chat",
    search: () => Promise.resolve({ hits: [], total: 0 }),
    holdsSeq: () => true,
    load: () => Promise.resolve(true),
    ...(mode === "operators" && {
      syntax: (ask) => Promise.resolve(cannedSyntax(ask.query, ask.cursor)),
    }),
  };
}

function Board({ mode, options }: { mode: Mode; options?: boolean }) {
  const index = useMemo(() => indexFor(mode), [mode]);
  const find = useChatFind(rows, index);
  const board = useRef<HTMLDivElement>(null);
  const target = find.open ? find.matches[find.active] : undefined;
  // The highlights over the rows as the transcript paints them (VirtualTranscript.tsx): the pattern's
  // terms, only in the rows it matched when it filters rows.
  useLayoutEffect(() => {
    const root = board.current;
    if (!root) return;
    if (!find.open || !find.pattern.terms.length) return clearFindHighlights();
    paintFindHighlights(
      root,
      find.pattern,
      target ? { rowId: target.rowId, card: false, occurrence: target.occurrence } : undefined,
      patternFilters(find.pattern) ? findPaintRows(rows, find.matches) : undefined,
    );
  });
  useLayoutEffect(() => clearFindHighlights, []);
  return (
    <div
      ref={board}
      className="acpmux-shell"
      style={
        {
          position: "relative",
          height: "100%",
          overflow: "auto",
          display: "block",
          "--acpmux-session-info-clear": "0px",
        } as CSSProperties
      }
      onFocusCapture={() => {
        if (!find.open) find.show();
      }}
    >
      <ChatFindBar find={find} optionsOpen={options} />
      <div
        className="selectable"
        style={{
          display: "grid",
          gap: "var(--ds-space-3)",
          padding: "var(--ds-space-56) var(--ds-space-5) var(--ds-space-5)",
          fontSize: "var(--text-content)",
          lineHeight: 1.5,
        }}
      >
        {rows.map((row) => (
          <div
            key={row.id}
            data-row-id={row.id}
            style={
              row.kind === "user"
                ? {
                    justifySelf: "end",
                    maxWidth: "80%",
                    padding: "var(--ds-space-2) var(--ds-space-3)",
                    borderRadius: 14,
                    background: "var(--agent-bubble, color-mix(in srgb, currentColor 8%, transparent))",
                  }
                : undefined
            }
          >
            {row.text}
          </div>
        ))}
      </div>
    </div>
  );
}

const field = { selector: ".acpmux-find .find-pill__field" };
const filters = { selector: ".acpmux-find .acpmux-find__filters" };
const count = (ctx: PlayContext) => ctx.document.querySelector(".acpmux-find .find-pill__count");
const chips = (ctx: PlayContext) => ctx.document.querySelectorAll(".acpmux-find .acpmux-find__chip").length;
// The bar is anchored at its right edge and grows to the left when the count shows; the rows under
// the pill appear below it and move nothing.
const shifts = {
  layoutShiftMax: {
    value: 0.05,
    reason: "The bar is anchored at its right edge and grows to the left when the count text first shows.",
  },
};

export default componentEntry<{ mode: Mode; options?: boolean }>({
  id: "agent-pane.chat-find-operators",
  title: "Find in Chat: search operators, options and chips",
  area: "Agent pane",
  height: 320,
  covers: [
    "agent-session/acpmux/conversation/ChatFindBar.tsx#ChatFindBar",
    "agent-session/acpmux/conversation/chatFind.ts#useChatFind",
    "agent-session/acpmux/conversation/searchSyntax.ts",
  ],
  load: async () => Board,
  styles: () => Promise.all([import("../styles.css"), import("./conversation.css")]),
  widths: { narrow: 360, normal: 640 },
  variants: {
    empty: {
      note: "The bar open with no query: the pill as it is, with Aa and the filters toggle beside the steps.",
      props: { mode: "operators" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.focus(field);
        await ctx.waitFor(() => ctx.document.querySelector(".acpmux-find .acpmux-find__filters"));
      },
    },
    chips: {
      note: "A word, a phrase, an exclusion and two filters: the daemon's chips under the pill, each removable; the highlights are the terms in the agent's replies without flaky.",
      props: { mode: "operators" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.type(CHIPS_QUERY, field);
        await ctx.waitFor(() => chips(ctx) === 4 && count(ctx)?.textContent === "1 of 4");
      },
    },
    autocomplete: {
      note: "fr typed: the operator suggestion from: under the field; Tab or Return takes it, Escape closes the list first.",
      props: { mode: "operators" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.type("fr", field);
        await ctx.waitFor(() => ctx.document.querySelector(".acpmux-find .acpmux-find__suggestion[aria-selected=true]"));
      },
    },
    options: {
      note: "The filters toggle on: the options row under the pill, Me and Today lit by the query's from:me since:today.",
      props: { mode: "operators" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.type(PICKS_QUERY, field);
        await ctx.click(filters);
        await ctx.waitFor(
          () =>
            ctx.document.querySelectorAll(".acpmux-find .acpmux-find__pick[aria-pressed=true]").length === 2 &&
            chips(ctx) === 2,
        );
      },
    },
    "no-results": {
      note: "Filters that leave nothing: No results, with the chips to take a filter out.",
      props: { mode: "operators" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.type(EMPTY_QUERY, field);
        await ctx.waitFor(() => chips(ctx) === 2 && count(ctx)?.hasAttribute("data-none") === true);
      },
    },
    "older-acpmux": {
      note: "An acpmux without search operators: no Aa, no filters, no chips; the text matches as one needle, as before.",
      props: { mode: "absent" },
      checks: shifts,
      play: async (ctx) => {
        await ctx.type("exact phrase", field);
        await ctx.waitFor(() => count(ctx)?.textContent === "1 of 4");
      },
    },
  },
});
