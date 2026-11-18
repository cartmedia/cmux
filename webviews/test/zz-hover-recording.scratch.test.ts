// Scratch (not committed): Before/After recordings of the user's 2026-11-17 hover motions on the real
// composer scope row (gallery agent-pane.composer-context), with a drawn pointer and per-frame row reads.
import { afterAll, beforeAll, expect, setDefaultTimeout, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { chromium, type Browser, type Page } from "playwright";
import { galleryFrame, warmGalleryFrame } from "./support/galleryFrame";

setDefaultTimeout(600_000);
const webviews = path.resolve(import.meta.dir, "..");
const out = process.env.HOVER_REC_OUT ?? path.join(webviews, "hover-rec-out");
fs.mkdirSync(out, { recursive: true });
const files = {
  desktop: path.join(webviews, "src/pages/shared/desktop.css"),
  location: path.join(webviews, "src/agent-session/acpmux/composerLocation.css"),
};
const use = (look: "before" | "after") => {
  fs.copyFileSync(path.join(webviews, `scratch-hover/desktop.${look}.css`), files.desktop);
  fs.copyFileSync(path.join(webviews, `scratch-hover/composerLocation.${look}.css`), files.location);
};

let vite: Awaited<ReturnType<typeof import("vite").createServer>>;
let browser: Browser;
beforeAll(async () => {
  const { createServer } = await import("vite");
  vite = await createServer({
    configFile: path.join(webviews, "vite.config.gallery-dev.ts"),
    server: { port: 0, host: "127.0.0.1", strictPort: false },
    logLevel: "error",
  });
  await vite.listen();
  browser = await chromium.launch({ headless: true });
  await warmGalleryFrame(vite, "agent-pane.composer-context", { browser, params: { variant: "environment-picker" } });
});
afterAll(async () => {
  use("after");
  await browser?.close();
  await vite?.close();
});

const W = 620;
const H = 520;
const url = () =>
  `http://127.0.0.1:${(vite.httpServer!.address() as { port: number }).port}${galleryFrame("agent-pane.composer-context", {
    variant: "environment-picker",
    frame: "component",
    width: W,
    theme: "Catppuccin Mocha",
  })}`;

// A drawn arrow that follows the real pointer (headless video draws none), and a per-frame log of
// which rows and chips are lit.
const POINTER = `
(() => {
  const dot = document.createElement('div');
  dot.style.cssText = 'position:fixed;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none;' +
    'border-left:7px solid transparent;border-right:7px solid transparent;border-bottom:16px solid #fff;' +
    'filter:drop-shadow(0 0 1px #000) drop-shadow(0 0 1px #000);transform-origin:7px 0;rotate:-25deg;';
  document.addEventListener('DOMContentLoaded', () => document.body.appendChild(dot));
  addEventListener('pointermove', (e) => { dot.style.left = (e.clientX - 7) + 'px'; dot.style.top = e.clientY + 'px'; }, true);
  window.__frames = [];
  const tick = (t) => {
    const lit = [...document.querySelectorAll('.ui-popup [role="option"], .ui-popup [role="menuitem"]')]
      .filter((r) => { const c = getComputedStyle(r).backgroundColor; return c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent'; })
      .map((r) => (r.textContent || '').trim().slice(0, 14));
    const chips = [...document.querySelectorAll('.acpmux-location-button')].map((b) =>
      (b.getAttribute('aria-expanded') === 'true' ? 'open:' : '') + getComputedStyle(b).backgroundColor);
    window.__frames.push({ t: Math.round(t), lit, chips });
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
})();`;

async function center(page: Page, name: RegExp) {
  const box = (await page.getByRole("button", { name }).first().boundingBox())!;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2, box };
}

async function glide(page: Page, x: number, y: number, ms: number) {
  await page.mouse.move(x, y, { steps: Math.max(2, Math.round(ms / 16)) });
}

async function record(look: "before" | "after") {
  use(look);
  const context = await browser.newContext({
    viewport: { width: W, height: H },
    deviceScaleFactor: 2,
    recordVideo: { dir: path.join(out, `${look}-raw`), size: { width: W * 2, height: H * 2 } },
  });
  await context.addInitScript(POINTER);
  const page = await context.newPage();
  await page.goto(url());
  await page.waitForFunction(() => document.documentElement.dataset.galleryReady === "1", undefined, { timeout: 120_000 });
  await page.evaluate(() => document.fonts.ready);
  // The variant's play opens the menu; start closed, as the person did.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(400);
  const env = await center(page, /^Environment/);
  const folder = await center(page, /^Folder/);
  await page.mouse.move(env.x + 60, env.y + 60);
  await page.waitForTimeout(300);
  await page.evaluate(() => ((window as unknown as { __frames: unknown[] }).__frames = []));
  // 1. Onto the location chip, click it open, wander on the chip.
  await glide(page, env.x, env.y, 250);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForTimeout(250);
  for (const dx of [-12, 10, -6, 4]) await glide(page, env.x + dx, env.y + 2, 120);
  // 2. Up through the rows: Other devices, Cloud, This Mac, then back down quickly (the two-lit-rows moments).
  const rows = await page.locator('.acpmux-env-popover [role="option"]').all();
  const centers = [];
  for (const row of rows) {
    const b = (await row.boundingBox())!;
    centers.push({ x: b.x + b.width * 0.35, y: b.y + b.height / 2 });
  }
  const order = [...centers].reverse(); // bottom row first: the menu opens above the chip
  for (const c of order) await glide(page, c.x, c.y, 140);
  await page.waitForTimeout(200);
  for (const c of [...order].reverse()) await glide(page, c.x, c.y, 120);
  await page.waitForTimeout(150);
  for (const c of order) await glide(page, c.x, c.y, 110);
  // 3. Out of the menu, back over the open chip and across it (the flicker).
  await glide(page, env.x + 70, env.y - 6, 200);
  for (const dx of [0, -20, 18, -8, 30, 0]) await glide(page, env.x + dx, env.y + (dx % 3), 150);
  await page.waitForTimeout(300);
  // 4. Close, open the folder chip, sweep its rows to the last one, then leave over the chip.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(250);
  await glide(page, folder.x, folder.y, 250);
  await page.mouse.down();
  await page.mouse.up();
  await page.waitForTimeout(300);
  const frows = await page.locator('.acpmux-folder-popover [role="option"]').all();
  const fc = [];
  for (const row of frows) {
    const b = await row.boundingBox();
    if (b && b.y > 0) fc.push({ x: b.x + b.width * 0.35, y: b.y + b.height / 2 });
  }
  for (const c of fc.slice(-4)) await glide(page, c.x, c.y, 130);
  await glide(page, folder.x, folder.y, 140);
  for (const dx of [-8, 10, -4, 12, 0]) await glide(page, folder.x + dx, folder.y + 1, 150);
  await page.waitForTimeout(400);
  const frames = await page.evaluate(() => (window as unknown as { __frames: unknown[] }).__frames);
  fs.writeFileSync(path.join(out, `${look}-frames.json`), JSON.stringify(frames));
  const video = page.video();
  await context.close();
  fs.copyFileSync(await video!.path(), path.join(out, `${look}.webm`));
  return frames as { t: number; lit: string[]; chips: string[] }[];
}

test("record before and after", async () => {
  const before = await record("before");
  const after = await record("after");
  const twoLit = (frames: { lit: string[] }[]) => frames.filter((f) => f.lit.length > 1).length;
  const summary = { before: { frames: before.length, twoLit: twoLit(before) }, after: { frames: after.length, twoLit: twoLit(after) } };
  fs.writeFileSync(path.join(out, "summary.json"), JSON.stringify(summary, null, 2));
  console.log("SUMMARY", JSON.stringify(summary));
  expect(after.length).toBeGreaterThan(0);
});
