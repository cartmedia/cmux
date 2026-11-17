import { afterAll, beforeAll, setDefaultTimeout, test } from "bun:test";
import path from "node:path";
import { chromium, type Browser } from "playwright";
import { devHighlightWorker } from "./support/devHighlightWorker";
import { galleryFrame, warmGalleryFrame } from "./support/galleryFrame";

setDefaultTimeout(300_000);
const { createServer } = await import("vite");
let server: Awaited<ReturnType<typeof createServer>>;
let browser: Browser;
let base: string;
beforeAll(async () => {
  const root = path.resolve(import.meta.dir, "..");
  server = await createServer({
    root,
    configFile: path.join(root, "vite.config.ts"),
    server: { port: 0, host: "127.0.0.1" },
    plugins: [devHighlightWorker()],
    logLevel: "error",
  });
  await server.listen();
  base = `http://127.0.0.1:${(server.httpServer!.address() as { port: number }).port}`;
  browser = await chromium.launch({ headless: true });
  await warmGalleryFrame(server, "agent-pane.transcript", { browser, params: { variant: "edit-long-line-code" } });
});
afterAll(async () => {
  await browser?.close();
  await server?.close();
});
for (const variant of ["edit-long-line-code"])
  for (const run of [1, 2, 3, 4])
    test(`${variant} ${run}`, async () => {
      const page = await browser.newPage({ viewport: { width: 760, height: 560 } });
      let release = () => {};
      const held = new Promise<void>((resolve) => (release = resolve));
      await page.route(/EditDiffHunks/, async (route) => {
        await held;
        await route.continue();
      });
      await page.addInitScript(() => {
        const frames: string[][] = [];
        (window as unknown as { __frames: string[][] }).__frames = frames;
        const tick = () => {
          const out: string[] = [];
          for (const el of document.querySelectorAll<HTMLElement>(".acpmux-row, .acpmux-row *")) {
            const r = el.getBoundingClientRect();
            if (r.height === 0) continue;
            const fracH = Math.abs(r.height - Math.round(r.height)) > 0.01;
            const fracY = Math.abs(r.top * 2 - Math.round(r.top * 2)) > 0.01 || Math.abs(r.top - Math.round(r.top)) > 0.01;
            if (fracH || el.matches(".acpmux-row"))
              out.push(
                `${el.tagName.toLowerCase()}.${String(el.className).split(" ").slice(0, 2).join(".")}${el.dataset.rowId ? `[${el.dataset.rowId}]` : ""} y=${r.top.toFixed(2)} h=${r.height.toFixed(2)}${fracY ? "" : ""}`,
              );
          }
          frames.push(out);
          if (frames.length < 400) requestAnimationFrame(tick);
        };
        addEventListener("load", () => requestAnimationFrame(tick));
      });
      await page.goto(
        `${base}${galleryFrame("agent-pane.transcript", { variant, theme: "Apple System Colors", colorScheme: "dark", width: 760, height: 560 })}`,
      );
      await page.waitForFunction(() => document.documentElement.dataset.galleryReady !== undefined, null, {
        timeout: 60_000,
      });
      const report = await page.evaluate(
        () =>
          (window as unknown as { cmuxGalleryPlayReport?: { steps: { action: string; boxMoves: unknown[] }[] } })
            .cmuxGalleryPlayReport?.steps.map((s) => [s.action, s.boxMoves]) ?? null,
      );
      release();
      await page.locator("diffs-container [data-line]").first().waitFor();
      await page.waitForTimeout(500);
      const frames = await page.evaluate(() => (window as unknown as { __frames: string[][] }).__frames);
      // Print the distinct states, in order, with how many frames each held.
      const states: { s: string; n: number }[] = [];
      for (const f of frames) {
        const s = f.join("\n");
        if (states.length && states[states.length - 1]!.s === s) states[states.length - 1]!.n += 1;
        else states.push({ s, n: 1 });
      }
      console.log("FRACDIAG", variant, run, JSON.stringify(report));
      for (const st of states) console.log(`FRACDIAG-STATE x${st.n}\n${st.s}\n----`);
      await page.close();
    });
