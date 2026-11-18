// l10n-allow-file: test fixture prompts and the host line's stand-in text, not shipped UI.
// A chat followed at its bottom holds still while nothing new arrives, whatever ticks (Leo, 2026-11-18,
// with a recording of his NIGHTLY: "did we fix this scroll bounce at the bottom yet?"). His chat sat at
// the bottom with a running command ("cmux-ci submit … 8m 3s") and "Waiting for the agent service
// (1:48)… Restart agent service" over the composer, both counting up each second, and the transcript
// jumped 30–70 px up and down every few hundred ms while he did not scroll.
// Here: the stand-in acpmux's chat ends in a running turn with two commands (their clocks tick), the
// host's waiting line sits in the stack over the composer as App.tsx draws it, and its clock ticks
// each second. For 10 s every row in view must hold its place to the pixel, and the view must keep
// following (no Jump to latest). Real Chromium against the pane's dev entry (support/papercutChat.ts);
// CI fails the file when it cannot launch it (requireBrowserLane). Locally:
//   CMUX_BROWSER_TESTS=1 bun test test/agent-pane-ticking-bottom-still.test.ts
import { afterAll, beforeAll, expect, setDefaultTimeout, test } from "bun:test";
import { chromium, type Browser } from "playwright";
import { requireBrowserLane } from "./support/requireBrowserLane";
import { papercutPane } from "./support/papercutChat";
import { TRANSCRIPT_ROW } from "../src/agent-session/acpmux/transcriptSelectors";

await requireBrowserLane("agent-pane-ticking-bottom-still.test.ts", async () => {
  setDefaultTimeout(300_000);
  let browser: Browser;
  let pane: Awaited<ReturnType<typeof papercutPane>>;
  beforeAll(async () => {
    browser = await chromium.launch({ headless: true });
    pane = await papercutPane(() => browser, { turns: 40, running: true, ready: ".acpmux-row" });
  });
  afterAll(async () => {
    await pane?.close();
    await browser?.close();
  });

  test("a followed chat with a ticking host line and running commands holds still for 10 s", async () => {
    const page = await pane.open({ width: 900, height: 700 });
    try {
      // Let the open settle first: the first draw's own motion is not this test's.
      await page.waitForTimeout(1500);
      const result = await page.evaluate(
        async ({ rowSelector }) => {
          const scroller = document.querySelector<HTMLElement>(".acpmux-scroll")!;
          const dock = document.querySelector<HTMLElement>(".acpmux-dock")!;
          const fromEnd = () => scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop;
          const startGap = fromEnd();
          // HostUnavailableLine's waiting markup: the group, its line, the message with its clock and
          // the quiet Restart agent service after it.
          const group = document.createElement("div");
          group.className = "acpmux-host-line-group";
          group.dataset.testHostLine = "";
          group.innerHTML =
            '<div class="acpmux-host-line"><p class="acpmux-host-line-message">Waiting for the agent service (1:48)</p><button type="button" class="acpmux-host-line-quiet">Restart agent service</button></div>';
          dock.insertBefore(group, dock.querySelector(":scope > .acpmux-composer"));
          const message = group.querySelector("p")!;
          let seconds = 108;
          const tick = setInterval(() => {
            seconds += 1;
            message.textContent = `Waiting for the agent service (${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")})`;
          }, 1000);
          const painted = () =>
            new Promise<void>((resolve) =>
              requestAnimationFrame(() => {
                const channel = new MessageChannel();
                channel.port1.onmessage = () => resolve();
                channel.port2.postMessage(0);
              }),
            );
          // Two frames for the line's own arrival (it lies over the foot; host-line-still covers it).
          await painted();
          await painted();
          const view = scroller.getBoundingClientRect();
          const tops = () =>
            new Map(
              [...document.querySelectorAll<HTMLElement>(rowSelector)].map((row) => [
                row.dataset.rowId!,
                row.getBoundingClientRect().top,
              ]),
            );
          const before = new Map([...tops()].filter(([, top]) => top > view.top && top < view.bottom));
          const moves: { t: number; id: string; dy: number; scrollTop: number; fromEnd: number }[] = [];
          let most = 0;
          let jump = false;
          let worstGap = 0;
          const start = performance.now();
          while (performance.now() - start < 10_000) {
            await painted();
            const now = tops();
            for (const [id, top] of before) {
              const at = now.get(id);
              const dy = at === undefined ? Infinity : at - top;
              if (Math.abs(dy) > 0.5 && moves.length < 40)
                moves.push({ t: Math.round(performance.now() - start), id, dy, scrollTop: scroller.scrollTop, fromEnd: fromEnd() });
              most = Math.max(most, Math.abs(dy));
            }
            worstGap = Math.max(worstGap, fromEnd());
            if (document.querySelector(".acpmux-jump-latest")) jump = true;
          }
          clearInterval(tick);
          return { rows: before.size, most, moves, jump, startGap, worstGap, line: group.getBoundingClientRect().height };
        },
        { rowSelector: TRANSCRIPT_ROW },
      );
      console.log(JSON.stringify(result));
      expect(result.startGap).toBeLessThanOrEqual(2);
      expect(result.rows).toBeGreaterThan(3);
      expect(result.line).toBeGreaterThan(12);
      expect(result.jump).toBe(false);
      expect(result.most).toBeLessThanOrEqual(0.5);
    } finally {
      await page.close();
    }
  });
});
