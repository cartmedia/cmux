// PROBE ONLY (not committed): replays a sanitized window of the chat in Leo's recording into the
// papercut stand-in, then holds 10 s at the bottom with the ticking waiting line.
import { afterAll, beforeAll, expect, setDefaultTimeout, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { chromium, type Browser, type Page } from "playwright";
import { papercutPane, PAPERCUT_CHAT, type PapercutStep } from "./support/papercutChat";
import { TRANSCRIPT_ROW } from "../src/agent-session/acpmux/transcriptSelectors";

type Held = any;
const hold = (page: Page, ms: number): Promise<Held> =>
  page.evaluate(
    async ({ rowSelector, ms }) => {
      const scroller = document.querySelector<HTMLElement>(".acpmux-scroll")!;
      const dock = document.querySelector<HTMLElement>(".acpmux-dock")!;
      const fromEnd = () => scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop;
      const startGap = fromEnd();
      // HostWaitingLine's markup: the group, its line, the message with its clock and the quiet
      // Restart agent service after it.
      const group = document.createElement("output");
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
      const moves: Held["moves"] = [];
      let most = 0;
      let jump = false;
      let worstGap = 0;
      const start = performance.now();
      while (performance.now() - start < ms) {
        await painted();
        const now = tops();
        const shown = !!document.querySelector(".acpmux-jump-latest[data-shown]");
        jump ||= shown;
        let frameMost = 0;
        let frameId = "";
        for (const [id, top] of before) {
          const at = now.get(id);
          const dy = at === undefined ? 9999 : at - top;
          if (Math.abs(dy) > Math.abs(frameMost)) {
            frameMost = dy;
            frameId = id;
          }
          most = Math.max(most, Math.abs(dy));
        }
        if (Math.abs(frameMost) > 0.5 && moves.length < 400)
          moves.push({
            t: Math.round(performance.now() - start),
            id: frameId,
            dy: Math.round(frameMost * 10) / 10,
            scrollTop: Math.round(scroller.scrollTop),
            fromEnd: Math.round(fromEnd()),
            jump: shown,
          });
        worstGap = Math.max(worstGap, fromEnd());
      }
      clearInterval(tick);
      return { rows: before.size, most, moves, jump, startGap, worstGap, line: group.getBoundingClientRect().height };
    },
    { rowSelector: TRANSCRIPT_ROW, ms },
  );

const SANITIZE = (s: string) => (s.length > 20 ? s.replace(/[a-zA-Z]/g, "x") : s);
const RAW_SESSION = SANITIZE("01a18a35-b1e8-7951-ba2f-703bf916b9a6");
const RECORDED_AT = 1795042840000;
const lines = readFileSync(path.join(import.meta.dir, ".probe/leo-window-big.jsonl"), "utf8").trim().split("\n");
const shift = Date.now() - RECORDED_AT;
const steps: PapercutStep[] = lines.map((line) => {
  const e = JSON.parse(line.split(RAW_SESSION).join(PAPERCUT_CHAT));
  return [e.dir, e.kind, e.msg, e.at + shift];
});
const turn = {
  turnId: SANITIZE("01a1f0dc-b0f3-7097-a020-f485a5430bb4"),
  promptId: SANITIZE("bg-done-bg-4a80e47f5325"),
  startedAt: 1795042357491 + shift,
};

setDefaultTimeout(300_000);
let browser: Browser;
beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
});
afterAll(async () => browser?.close());

for (const [width, busy] of [[900, false], [900, true]] as const) {
  test(`leo replay holds still at ${width} busy=${busy}`, async () => {
    const pane = await papercutPane(() => browser, {
      turns: 0,
      running: false,
      extra: steps,
      session: { status: "running", turn, harness: "claude", family: "claude" },
      ready: ".acpmux-row",
      ...(busy
        ? {
            otherSessions: Array.from({ length: 8 }, (_, n) => ({
              sessionId: `0190a000-0000-7000-8000-0000000fc1${String(n).padStart(2, "0")}`,
              cwd: `/home/dev/other-${n}`,
            })),
          }
        : {}),
    });
    try {
      if (busy) pane.busyMethods.add("_acpmux/events");
      const page = await pane.open({ width, height: 900 });
      const cdp = await page.context().newCDPSession(page);
      await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
      await page.waitForTimeout(3000);
      const sentBefore = pane.methods.length;
      if (busy) for (const m of ["_acpmux/events", "_acpmux/peers", "_acpmux/status", "_acpmux/watch", "_acpmux/attach"]) pane.busyMethods.add(m);
      const box = (await page.locator(".acpmux-scroll").boundingBox())!;
      await page.mouse.move(box.x + box.width * 0.8, box.y + box.height * 0.7);
      const held = hold(page, 12_000);
      await page.waitForTimeout(200);
      // A trackpad flick down at the bottom: a burst of wheel events, decaying, ~60 Hz, every 1.3 s.
      for (let burst = 0; burst < 4; burst++) {
        for (let step = 0; step < 18; step++) {
          await page.mouse.wheel(0, -Math.max(2, Math.round(20 * Math.pow(0.82, step))));
          await page.waitForTimeout(16);
        }
        await page.waitForTimeout(1000);
      }
      const result = await held;
      console.log(`up-${width}-${busy}`, JSON.stringify(result), "methods", JSON.stringify(pane.methods.slice(sentBefore).slice(0, 80)));
      const tail = await page.evaluate(
        (sel) =>
          [...document.querySelectorAll<HTMLElement>(sel)]
            .slice(-6)
            .map((r) => `${r.dataset.rowId}|${r.className}|${Math.round(r.getBoundingClientRect().height)}`),
        TRANSCRIPT_ROW,
      );
      console.log(`up-${width}-tail`, JSON.stringify(tail));
      await page.screenshot({ path: `${process.env.FAST_CHECKS_OUT ?? "/tmp"}/replay-${width}.png` });
      expect(true).toBe(true);
    } finally {
      await pane.close();
    }
  });
}
