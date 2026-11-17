// What the transcript painted between its rows, read after every painted frame (paintedFrames.ts).
//
// The transcript places each row at the sum of the placed heights above it plus the designed gap
// between each pair (VirtualTranscript.tsx; transcriptGrid.ts `transcriptGap` through the geometry's
// `gapAt`), and draws that gap under each row as its `.acpmux-row__gap` span. So between two adjacent
// mounted rows `a` and `b`, on screen (getBoundingClientRect, so a fold's riding translate counts):
//   b.top - a.bottom === a's gap span height (0 when it has none)
// More is a hole: the row above was placed at a height it is not drawn at (Leo, 2026-11-17: "why is
// there this big gap when i have shit expanded"). Less is an overlap. The reads:
// - HOLE: each adjacent pair's `b.top - a.bottom - gap`, with the rows' ids.
// - BLANK: the tallest band of the scroller's view, between the first and the last row in view, that
//   no row (nor its gap) covers.
// - SHIFT: how far tracked elements moved on screen since the sampler started.
// - LEFTOVER: a row still wearing an inline translate, a `.cv-fold` with an inline height, a
//   `[data-fold-clip]`, after the motion settled.
// transcript-row-holes-browser.test.ts gates on them; other frame tests may reuse them.
import type { Page } from "playwright";

export type RowBox = { id: string; pos: number; top: number; bottom: number; gap: number };
export type RowHole = { above: string; below: string; hole: number; inView: boolean };
export type RowFrame = {
  /// Adjacent pairs whose space is off the designed gap by more than 1px (hole > 0, overlap < 0).
  holes: RowHole[];
  /// The tallest uncovered band between the first and last row in view, px.
  blank: number;
  /// Leftovers of a motion (only meaningful once it settled).
  leftovers: string[];
  /// Tracked elements' on-screen top, by the name they were tracked under.
  tops: Record<string, number>;
};
export type RowSample = {
  frames: number;
  /// The worst hole (by size) seen in any frame, and the frame it was in.
  worstHole?: RowHole & { frame: number };
  /// The worst blank band seen in any frame.
  worstBlank: number;
  /// How far each tracked element moved at most from where it was at the start.
  moved: Record<string, number>;
};

declare global {
  interface Window {
    cmuxRowHoles: {
      /// One read of the page as it stands. `track`: name -> selector of an element whose top to report.
      read(track?: Record<string, string>): RowFrame;
      /// Reads after every painted frame until `stop`.
      start(track?: Record<string, string>): void;
      stop(): RowSample;
    };
  }
}

/** The page side; runs in the page, so it closes over nothing. */
function defineRowHoles() {
  if (window.cmuxRowHoles) return;
  const SLACK = 1;
  const read = (track: Record<string, string> = {}) => {
    const scroller = document.querySelector<HTMLElement>(".acpmux-scroll");
    const out = { holes: [] as RowHole[], blank: 0, leftovers: [] as string[], tops: {} as Record<string, number> };
    if (!scroller) return out;
    const view = scroller.getBoundingClientRect();
    const rows: RowBox[] = [];
    for (const node of scroller.querySelectorAll<HTMLElement>("article.acpmux-row")) {
      const box = node.getBoundingClientRect();
      const span = node.querySelector<HTMLElement>(":scope > .acpmux-row__gap");
      const gap = parseFloat(span?.style.height ?? "") || 0;
      rows.push({
        id: node.dataset.rowId ?? "?",
        pos: Number(node.getAttribute("aria-posinset")),
        top: box.top,
        bottom: box.bottom,
        gap,
      });
      if (node.style.translate !== "" && node.style.translate !== "none")
        out.leftovers.push(`row ${node.dataset.rowId} translate ${node.style.translate}`);
    }
    rows.sort((a, b) => a.pos - b.pos);
    for (let index = 1; index < rows.length; index += 1) {
      const above = rows[index - 1]!;
      const below = rows[index]!;
      if (below.pos !== above.pos + 1) continue;
      const hole = below.top - above.bottom - above.gap;
      if (Math.abs(hole) <= SLACK) continue;
      const inView = above.bottom < view.bottom && below.top > view.top;
      out.holes.push({ above: above.id, below: below.id, hole: Math.round(hole * 10) / 10, inView });
    }
    // The band of the view the rows and their gaps cover, from the first row in view to the last.
    const spans = rows
      .map((row) => [Math.max(view.top, row.top), Math.min(view.bottom, row.bottom + row.gap)] as const)
      .filter(([top, bottom]) => bottom > top)
      .sort((a, b) => a[0] - b[0]);
    let reach = spans[0]?.[1];
    for (const [top, bottom] of spans.slice(1)) {
      if (reach !== undefined && top - reach > out.blank) out.blank = Math.round((top - reach) * 10) / 10;
      reach = Math.max(reach ?? bottom, bottom);
    }
    for (const fold of scroller.querySelectorAll<HTMLElement>(".cv-fold"))
      if (fold.style.height !== "") out.leftovers.push(`.cv-fold height ${fold.style.height}`);
    for (const clip of scroller.querySelectorAll<HTMLElement>("[data-fold-clip]"))
      out.leftovers.push(`[data-fold-clip] on ${clip.className}`);
    for (const [name, selector] of Object.entries(track)) {
      const node = document.querySelector<HTMLElement>(selector);
      if (node) out.tops[name] = node.getBoundingClientRect().top;
    }
    return out;
  };
  let stopSampling: (() => void) | undefined;
  let sample: RowSample = { frames: 0, worstBlank: 0, moved: {} };
  window.cmuxRowHoles = {
    read,
    start(track = {}) {
      stopSampling?.();
      sample = { frames: 0, worstBlank: 0, moved: {} };
      const first = read(track).tops;
      stopSampling = window.cmuxAfterPaint(() => {
        const frame = read(track);
        sample.frames += 1;
        for (const hole of frame.holes)
          if (hole.inView && (!sample.worstHole || Math.abs(hole.hole) > Math.abs(sample.worstHole.hole)))
            sample.worstHole = { ...hole, frame: sample.frames };
        sample.worstBlank = Math.max(sample.worstBlank, frame.blank);
        for (const [name, top] of Object.entries(frame.tops))
          if (first[name] !== undefined)
            sample.moved[name] = Math.max(sample.moved[name] ?? 0, Math.round(Math.abs(top - first[name]!) * 10) / 10);
      });
    },
    stop() {
      stopSampling?.();
      stopSampling = undefined;
      return sample;
    },
  };
}

/** Defines `window.cmuxRowHoles` in `page`'s current document and every document it opens. Needs
 *  installPaintedFrames first. */
export async function installRowHoles(page: Page) {
  await page.addInitScript(defineRowHoles);
  await page.evaluate(defineRowHoles);
}
