// A composer attachment's tooltip shows once and stays put while the pointer rests on the chip
// (Leo, 2026-11-18, of a video thumbnail and a pasted-text card in his composer: "the hover
// preview bounces for some reason"). The pointer rests on each tile (still, then with a hand's
// one-pixel tremor) while every animation frame samples the title tooltip (ui/titleTooltips.ts):
// it must show once, at one place, and never hide or move until the pointer leaves. Both engines:
// the app's WebKit and the gallery's Chromium. Frames go to $FAST_CHECKS_OUT when set.
import { expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { chromium, type Page, webkit } from "playwright";
import { galleryFrame, warmGalleryFrame } from "./support/galleryFrame";
import { requireBrowserLane } from "./support/requireBrowserLane";

await requireBrowserLane("composer-attachment-tooltip-steady.browser.test.ts", async () => {
  const ENTRY = "agent-pane.composer";
  type Sample = { t: number; hidden: boolean; left: number; top: number; text: string };
  type Run = { frames: number; shows: number; hides: number; places: string[]; texts: string[] };

  /// Samples the tooltip every frame for `ms`, with the pointer moving by `wiggle` px every 40 ms.
  async function rest(page: Page, x: number, y: number, ms: number, wiggle: number): Promise<Sample[]> {
    await page.evaluate(() => {
      const w = window as unknown as { __tipSamples: Sample[]; __tipStop: boolean };
      w.__tipSamples = [];
      w.__tipStop = false;
      const start = performance.now();
      const tick = () => {
        const tip = document.querySelector<HTMLElement>(".ui-title-tooltip");
        const box = tip?.getBoundingClientRect();
        w.__tipSamples.push({
          t: Math.round(performance.now() - start),
          hidden: !tip || tip.hidden,
          left: Math.round((box?.left ?? 0) * 2) / 2,
          top: Math.round((box?.top ?? 0) * 2) / 2,
          text: tip?.textContent ?? "",
        });
        if (!w.__tipStop) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    await page.mouse.move(x, y);
    const until = Date.now() + ms;
    let step = 0;
    while (Date.now() < until) {
      await page.waitForTimeout(40);
      if (wiggle) await page.mouse.move(x + (step++ % 2 ? wiggle : 0), y);
    }
    return page.evaluate(() => {
      const w = window as unknown as { __tipSamples: Sample[]; __tipStop: boolean };
      w.__tipStop = true;
      return w.__tipSamples;
    });
  }

  function summarize(samples: Sample[]): Run {
    let shows = 0;
    let hides = 0;
    const places = new Set<string>();
    const texts = new Set<string>();
    let was = true;
    for (const sample of samples) {
      if (was && !sample.hidden) shows++;
      if (!was && sample.hidden) hides++;
      was = sample.hidden;
      if (!sample.hidden) {
        places.add(`${sample.left},${sample.top}`);
        texts.add(sample.text);
      }
    }
    return { frames: samples.length, shows, hides, places: [...places], texts: [...texts] };
  }

  const SCENES = [
    { variant: "attachments-inline-mixed", chip: ".acpmux-md .acpmux-attachment-video", where: "inline video" },
    { variant: "attachments-top-mixed", chip: ".acpmux-attachments .acpmux-attachment-video", where: "strip video" },
    { variant: "attachments-inline-mixed", chip: ".acpmux-md .acpmux-attachment-image:not(.acpmux-attachment-video)", where: "inline image" },
    { variant: "text-pasted", chip: ".acpmux-attachment-pasted", where: "strip paste" },
    { variant: "text-pasted-inline", chip: ".acpmux-md .acpmux-paste-chip", where: "inline paste" },
  ];

  test("a resting pointer on an attachment chip shows its tooltip once, in one place", async () => {
    const { createServer } = await import("vite");
    const vite = await createServer({
      configFile: path.resolve(import.meta.dir, "../vite.config.gallery-dev.ts"),
      server: { host: "127.0.0.1", port: 0, strictPort: false, watch: { ignored: () => true } },
      logLevel: "error",
    });
    const report: Record<string, unknown> = {};
    const failures: string[] = [];
    try {
      await vite.listen();
      const port = (vite.httpServer!.address() as { port: number }).port;
      for (const engine of [chromium, webkit]) {
        const browser = await engine.launch({ headless: true });
        try {
          await warmGalleryFrame(vite, ENTRY, { browser });
          for (const scene of SCENES) {
            const page = await browser.newPage({ viewport: { width: 1000, height: 760 } });
            try {
              await page.goto(`http://127.0.0.1:${port}${galleryFrame(ENTRY, { variant: scene.variant, width: 1000 })}`);
              await page.waitForFunction(() => document.documentElement.dataset.galleryReady === "1", undefined, {
                timeout: 90_000,
              });
              await page.waitForSelector(scene.chip, { timeout: 30_000 });
              await page.mouse.move(5, 5);
              await page.waitForTimeout(400);
              const box = (await page.locator(scene.chip).first().boundingBox())!;
              // The tile's centre (its open button, over the poster) and its lower left (clear of
              // the play glyph and the ×).
              const spots = {
                centre: [box.x + box.width / 2, box.y + box.height / 2],
                corner: [box.x + 6, box.y + box.height - 6],
              } as const;
              for (const [spot, [x, y]] of Object.entries(spots))
                for (const wiggle of [0, 1]) {
                  await page.mouse.move(5, 5);
                  // Past the tooltip group's warm time, so each rest starts cold.
                  await page.waitForTimeout(1200);
                  const samples = await rest(page, x, y, 1500, wiggle);
                  const run = summarize(samples);
                  const key = `${engine.name()} ${scene.where} ${spot} ${wiggle ? "tremor" : "still"}`;
                  report[key] = run;
                  if (process.env.FAST_CHECKS_OUT)
                    writeFileSync(
                      path.join(process.env.FAST_CHECKS_OUT, `tooltip-frames-${key.replace(/\s+/g, "-")}.json`),
                      JSON.stringify(samples),
                    );
                  // A chip with no tooltip (a paste's card names itself) has nothing to bounce.
                  if (run.shows === 0) continue;
                  if (run.shows !== 1 || run.hides !== 0 || run.places.length !== 1)
                    failures.push(`${key}: ${JSON.stringify(run)}`);
                }
              if (process.env.FAST_CHECKS_OUT)
                await page.screenshot({
                  path: path.join(
                    process.env.FAST_CHECKS_OUT,
                    `tooltip-${engine.name()}-${scene.where.replace(/\s+/g, "-")}.png`,
                  ),
                });
            } finally {
              await page.close();
            }
          }
        } finally {
          await browser.close();
        }
      }
    } finally {
      await vite.close();
    }
    console.log(JSON.stringify(report, null, 1));
    if (process.env.FAST_CHECKS_OUT)
      writeFileSync(path.join(process.env.FAST_CHECKS_OUT, "tooltip-steady-report.json"), JSON.stringify(report, null, 1));
    expect(failures).toEqual([]);
  }, 600_000);
});
