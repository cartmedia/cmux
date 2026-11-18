// Find in Chat's bar (chatFind.ts): the shared find pill (ui/FindPill.tsx, the web twin of the
// native one) with the query, where the reader is among the matches, and the steps. Enter and
// Cmd-G go to the next match, Shift-Enter and Shift-Cmd-G to the previous one, Escape closes.
// With a daemon that reads search operators (searchSyntax.ts; Leo, 2026-11-18: "I know that VS Code
// has like a decent kind of searching thing with arguments and whatnot, likewise with Obsidian"),
// the pill also has a match-case toggle (Aa) and a filters toggle, and under it, in the pill's own
// glass: operator suggestions for the token at the caret, the options row (quick picks that write
// operators into the text, so the text stays the truth), and the chips the daemon read, each
// removable in one click. With an older acpmux the bar is as it was.
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";
import { FindPill, type FindPillIconRole } from "../../../ui/FindPill";
import { setPopupInsetTop } from "../../../ui/popupInset";
import { Icon } from "../icons/Icon";
import { useT } from "../i18n";
import { FIND_HIDE_EVENT, FIND_OPEN_EVENT, type FindBarState, type FindSyntax } from "./chatFind";
import {
  acceptCompletion,
  applyPick,
  folderOperator,
  pickOn,
  QUICK_PICKS,
  removeChip,
  type SearchChip,
  type SearchPick,
} from "./searchSyntax";
import "./chatFindOptions.css";

/** The native pill's icons (FindPillIcons in the browser and terminal). */
const ICONS: Record<FindPillIconRole, string> = {
  search: "browser.omnibox.search",
  previous: "browser.find.previous",
  next: "browser.find.next",
  close: "action.close",
};

/** The harnesses' names on the options row's picks: product names, the same in every language. */
const HARNESS_NAMES = {
  // l10n-allow: a product name
  claude: "Claude",
  // l10n-allow: a product name
  codex: "Codex",
};

/** The room kept between the bar and a popup that flips below it. */
const POPUP_CLEARANCE = 4;

/** `label` names the field (its placeholder too); Find in Chat by default. */
export function ChatFindBar({
  find,
  label,
  optionsOpen: initialOptions = false,
}: {
  find: FindBarState;
  label?: string;
  /** The options row starts open (the gallery). */
  optionsOpen?: boolean;
}) {
  const t = useT();
  const field = useRef<HTMLInputElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const syntax = find.syntax;
  const [optionsOpen, setOptionsOpen] = useState(initialOptions);
  // The suggestion the arrows are on, and whether Escape closed the list for this text and caret.
  const [chosen, setChosen] = useState(0);
  const [dismissed, setDismissed] = useState<string>();
  const completions = syntax?.completions ?? [];
  const listKey = completions.map((each) => `${each.start}:${each.end}:${each.insert}`).join("|");
  const suggesting = completions.length > 0 && dismissed !== `${find.query}\u0000${listKey}`;
  useEffect(() => setChosen(0), [listKey]);
  // The bar is the top layer of its pane: while it shows, tooltips and hover cards keep below it,
  // flipping under their row instead of covering it (ui/popupInset.ts; Leo 2026-11-07: "we don't want
  // it to be coverable by other sorts of things. Like, for example, these hover previews").
  useLayoutEffect(() => {
    const node = bar.current;
    if (!node) return;
    const view = node.ownerDocument.defaultView;
    const place = () => setPopupInsetTop(node.getBoundingClientRect().bottom + POPUP_CLEARANCE, "findBar");
    place();
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(place) : undefined;
    observer?.observe(node);
    view?.addEventListener("resize", place);
    return () => {
      observer?.disconnect();
      view?.removeEventListener("resize", place);
      setPopupInsetTop(0, "findBar");
    };
  }, []);
  // Opening, it tells the pane, so a Session Info with no room for the bar beside it steps aside; the
  // pane closes it when the person opens Session Info over it there.
  const { hide } = find;
  useEffect(() => {
    const shell = field.current?.closest(".acpmux-shell");
    shell?.dispatchEvent(new (shell.ownerDocument.defaultView?.Event ?? Event)(FIND_OPEN_EVENT));
  }, []);
  useEffect(() => {
    const shell = field.current?.closest(".acpmux-shell");
    shell?.addEventListener(FIND_HIDE_EVENT, hide);
    return () => shell?.removeEventListener(FIND_HIDE_EVENT, hide);
  }, [hide]);
  // The transcript keeps its top fade while the bar is open, with full ink over the bar's box
  // (conversation.css: --acpmux-find-x, --acpmux-find-y, --acpmux-find-width and
  // --acpmux-find-height on the scroller: the pill and the rows under it), measured before paint and
  // again when the bar narrows or grows, the pane resizes, or Session Info moves it (its reach is a
  // style on the shell, SessionInfo.tsx SESSION_INFO_CLEAR).
  useLayoutEffect(() => {
    const node = bar.current;
    const scroller = node?.closest<HTMLElement>(".acpmux-scroll");
    if (!node || !scroller) return;
    const measure = () => {
      const box = node.getBoundingClientRect();
      const view = scroller.getBoundingClientRect();
      scroller.style.setProperty("--acpmux-find-x", `${box.left - view.left}px`);
      scroller.style.setProperty("--acpmux-find-y", `${box.top - view.top}px`);
      scroller.style.setProperty("--acpmux-find-width", `${box.width}px`);
      scroller.style.setProperty("--acpmux-find-height", `${box.height}px`);
    };
    measure();
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : undefined;
    observer?.observe(node);
    observer?.observe(scroller);
    const shell = scroller.closest(".acpmux-shell");
    const moved = typeof MutationObserver === "function" ? new MutationObserver(measure) : undefined;
    if (shell) moved?.observe(shell, { attributes: true, attributeFilter: ["style"] });
    return () => {
      observer?.disconnect();
      moved?.disconnect();
      scroller.style.removeProperty("--acpmux-find-x");
      scroller.style.removeProperty("--acpmux-find-y");
      scroller.style.removeProperty("--acpmux-find-width");
      scroller.style.removeProperty("--acpmux-find-height");
    };
  }, []);
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, [find.focusRequest]);
  // The caret, for the suggestions: they are for the token it is in.
  const setCursor = syntax?.setCursor;
  useEffect(() => {
    const input = field.current;
    if (!input || !setCursor) return;
    const report = () => setCursor(input.selectionStart ?? input.value.length);
    const events = ["select", "keyup", "click", "input", "focus"] as const;
    for (const event of events) input.addEventListener(event, report);
    return () => {
      for (const event of events) input.removeEventListener(event, report);
    };
  }, [setCursor]);
  // An edit made outside the field (a chip, a pick, a suggestion) puts the caret where it left it.
  const pendingCaret = useRef<number | undefined>(undefined);
  useLayoutEffect(() => {
    const input = field.current;
    const at = pendingCaret.current;
    if (!input || at === undefined) return;
    pendingCaret.current = undefined;
    input.focus();
    input.setSelectionRange(at, at);
  }, [find.query]);
  const edit = (next: string, at = next.length) => {
    pendingCaret.current = at;
    syntax?.edit(next, at);
  };
  const accept = (index: number) => {
    const completion = completions[index];
    if (!completion) return;
    const { query, cursor } = acceptCompletion(find.query, completion);
    edit(query, cursor);
  };
  const onFieldKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (!suggesting) return false;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const by = event.key === "ArrowDown" ? 1 : -1;
      setChosen((at) => (at + by + completions.length) % completions.length);
      return true;
    }
    if ((event.key === "Tab" && !event.shiftKey) || (event.key === "Enter" && !event.shiftKey && !event.metaKey)) {
      event.preventDefault();
      accept(Math.min(chosen, completions.length - 1));
      return true;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setDismissed(`${find.query}\u0000${listKey}`);
      return true;
    }
    return false;
  };
  // The transcript index's hits in turns not loaded count too (chatFind.ts `total`); while it is
  // still answering, a query with no loaded match says nothing yet rather than "No results".
  const count = find.total ?? find.matches.length;
  const position = find.total === undefined ? find.active : (find.position ?? 0);
  const searching = Boolean(find.busy) && !count;
  const status = !find.query
    ? ""
    : count
      ? t("find.count", { current: position + 1, total: count })
      : searching
        ? ""
        : t("find.noResults");
  const chips = syntax?.parsed?.chips ?? [];
  return (
    <div className="acpmux-find">
      <div className="acpmux-find__bar" ref={bar}>
        <FindPill
          query={find.query}
          onQueryChange={find.setQuery}
          status={status}
          noMatches={Boolean(find.query) && !count && !searching}
          busy={Boolean(find.query) && Boolean(find.busy)}
          canStep={count > 0}
          onNext={find.next}
          onPrevious={find.previous}
          onClose={find.hide}
          inputRef={field}
          labels={{
            placeholder: label ?? t("find.placeholder"),
            previous: t("find.previous"),
            next: t("find.next"),
            close: t("find.close"),
            ...(syntax && { matchCase: t("find.matchCase") }),
          }}
          icon={(role) => <Icon name={ICONS[role]} size={14} />}
          {...(syntax && {
            matchCase: syntax.caseSensitive,
            onMatchCaseChange: syntax.setCaseSensitive,
            onFieldKeyDown,
            toggles: (
              <button
                type="button"
                className="find-pill__button acpmux-find__filters"
                aria-label={t("find.filters")}
                title={t("find.filters")}
                aria-pressed={optionsOpen ? "true" : "false"}
                aria-expanded={optionsOpen}
                onClick={() => setOptionsOpen((open) => !open)}
              >
                <Icon name="filter" size={14} />
              </button>
            ),
          })}
        />
        {syntax && suggesting ? (
          <ul className="acpmux-find__panel acpmux-find__suggest" role="listbox" aria-label={t("find.suggestions")}>
            {completions.map((completion, index) => (
              <li
                key={`${completion.start}:${completion.insert}`}
                role="option"
                aria-selected={index === chosen}
                className="acpmux-find__suggestion"
                // Keeps the field's focus and caret.
                onPointerDown={(event) => event.preventDefault()}
                onPointerEnter={() => setChosen(index)}
                onClick={() => accept(index)}
              >
                <span className="acpmux-find__suggestion-label">{completion.label}</span>
                {completion.detail ? <span className="acpmux-find__suggestion-detail">{completion.detail}</span> : null}
              </li>
            ))}
          </ul>
        ) : null}
        {syntax && optionsOpen ? <FindOptions query={find.query} syntax={syntax} edit={edit} /> : null}
        {syntax && chips.length ? (
          <div className="acpmux-find__chips" role="group" aria-label={t("find.chips")}>
            {chips.map((chip) => (
              <FindChip key={`${chip.start}:${chip.end}`} chip={chip} onRemove={() => edit(removeChip(find.query, chip), chip.start)} />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** One chip the daemon read: its label, and a button that takes its text out of the query. */
function FindChip({ chip, onRemove }: { chip: SearchChip; onRemove: () => void }) {
  const t = useT();
  return (
    <span className="acpmux-find__chip" data-kind={chip.kind}>
      <span className="acpmux-find__chip-label">{chip.label}</span>
      <button
        type="button"
        className="acpmux-find__chip-remove"
        aria-label={t("find.chip.remove", { label: chip.label })}
        title={t("find.chip.remove", { label: chip.label })}
        onPointerDown={(event) => event.preventDefault()}
        onClick={onRemove}
      >
        <Icon name="action.close" size={10} />
      </button>
    </span>
  );
}

/** The options row (VS Code's "files to include" under its find): quick picks that write an
 * operator into the query, lit while the query holds it, and a folder field. */
function FindOptions({
  query,
  syntax,
  edit,
}: {
  query: string;
  syntax: FindSyntax;
  edit: (query: string, cursor?: number) => void;
}) {
  const t = useT();
  const chips = syntax.parsed?.chips ?? [];
  const [folder, setFolder] = useState("");
  const pick = (each: SearchPick, name: string) => (
    <button
      type="button"
      className="acpmux-find__pick"
      aria-pressed={pickOn(chips, query, each.operator) ? "true" : "false"}
      title={each.operator}
      onPointerDown={(event) => event.preventDefault()}
      onClick={() => edit(applyPick(query, chips, each.kinds, each.operator))}
    >
      {name}
    </button>
  );
  const addFolder = () => {
    const operator = folderOperator(folder);
    if (!operator) return;
    edit(applyPick(query, chips, ["folder"], operator));
    setFolder("");
  };
  return (
    <div className="acpmux-find__panel acpmux-find__options">
      <div className="acpmux-find__group" role="group" aria-label={t("find.options.from")}>
        <span className="acpmux-find__group-label">{t("find.options.from")}</span>
        {pick(QUICK_PICKS.fromMe, t("find.options.fromMe"))}
        {pick(QUICK_PICKS.fromAgent, t("find.options.fromAgent"))}
      </div>
      <div className="acpmux-find__group" role="group" aria-label={t("find.options.agent")}>
        <span className="acpmux-find__group-label">{t("find.options.agent")}</span>
        {pick(QUICK_PICKS.claude, HARNESS_NAMES.claude)}
        {pick(QUICK_PICKS.codex, HARNESS_NAMES.codex)}
      </div>
      <div className="acpmux-find__group" role="group" aria-label={t("find.options.since")}>
        <span className="acpmux-find__group-label">{t("find.options.since")}</span>
        {pick(QUICK_PICKS.today, t("find.options.today"))}
        {pick(QUICK_PICKS.week, t("find.options.week"))}
      </div>
      <input
        className="acpmux-find__folder"
        type="text"
        value={folder}
        placeholder={t("find.options.folder")}
        aria-label={t("find.options.folder")}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(event) => setFolder(event.target.value)}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing || event.keyCode === 229) return;
          if (event.key === "Enter") {
            event.preventDefault();
            addFolder();
          }
        }}
      />
    </div>
  );
}
