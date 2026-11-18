// SCRATCH (not for commit): captures the Studio and Paper looks on the App window chat, light and dark.
import { afterAll, beforeAll, setDefaultTimeout, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { chromium, type Browser, type Page } from "playwright";
import { agentPaneTheme } from "../../src/gallery/theme/web";
import { deriveTokens, GHOSTTY_DEFAULT, rgbHex } from "../../src/gallery/theme/tokens";
import { galleryFrame, warmGalleryFrame } from "../support/galleryFrame";

setDefaultTimeout(240_000);
const webviews = path.resolve(import.meta.dir, "../..");
const repo = path.join(webviews, "..");
const ENTRY = "app.window";
const out = path.join(process.env.FAST_CHECKS_OUT ?? "/tmp/look-shots", "look-shots");
mkdirSync(out, { recursive: true });
const css = (look: string) => readFileSync(path.join(repo, "examples/looks", look, "agent-pane/theme.css"), "utf8");

// The look's own theme file colors (themes/NAME.json), through the gallery's port of the host's derivation.
const THEME_FILES: Record<string, { background: number; foreground: number; selection: number; accent: number }> = {
  studio: { background: 0x0b0b0c, foreground: 0xe4e4e7, selection: 0x27272a, accent: 0xa1a1aa },
  paper: { background: 0xfaf8f3, foreground: 0x2d2a26, selection: 0xe8e1d3, accent: 0x8a5a2b },
};
function themePayload(look: string) {
  const c = THEME_FILES[look]!;
  return agentPaneTheme(
    deriveTokens({
      ...GHOSTTY_DEFAULT,
      background: rgbHex(c.background),
      foreground: rgbHex(c.foreground),
      selectionBackground: rgbHex(c.selection),
      accentColor: rgbHex(c.accent),
    }),
    true,
  );
}

let vite: Awaited<ReturnType<typeof import("vite").createServer>>;
let browser: Browser;
beforeAll(async () => {
  const { createServer } = await import("vite");
  vite = await createServer({
    configFile: path.join(webviews, "vite.config.gallery-dev.ts"),
    server: { port: 0, host: "127.0.0.1" },
    logLevel: "error",
  });
  await vite.listen();
  browser = await chromium.launch({ headless: true });
  await warmGalleryFrame(vite, ENTRY, { browser, params: { variant: "look-studio" } });
});
afterAll(async () => {
  await browser?.close();
  await vite?.close();
});

const settle = (page: Page) =>
  page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));

const cases: { look: string; scheme: "light" | "dark"; themeFile: boolean; name: string }[] = [
  { look: "studio", scheme: "dark", themeFile: true, name: "studio-dark" },
  { look: "studio", scheme: "dark", themeFile: false, name: "studio-dark-apple" },
  { look: "studio", scheme: "light", themeFile: false, name: "studio-light" },
  { look: "paper", scheme: "light", themeFile: true, name: "paper-light" },
  { look: "paper", scheme: "light", themeFile: false, name: "paper-light-apple" },
  { look: "paper", scheme: "dark", themeFile: false, name: "paper-dark" },
  { look: "none", scheme: "dark", themeFile: false, name: "baseline-dark" },
  { look: "none", scheme: "light", themeFile: false, name: "baseline-light" },
];

for (const c of cases) {
  test(c.name, async () => {
    const page = await browser.newPage({ viewport: { width: 1200, height: 900 }, deviceScaleFactor: 1 });
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    try {
      const port = (vite.httpServer!.address() as { port: number }).port;
      await page.goto(
        `http://127.0.0.1:${port}${galleryFrame(ENTRY, {
          variant: c.look === "paper" ? "look-paper" : c.look === "studio" ? "look-studio" : "chat-session-info",
          theme: c.scheme === "light" ? "Apple System Colors Light" : "Apple System Colors",
          colorScheme: c.scheme,
        })}`,
      );
      await page.waitForFunction(() => document.documentElement.dataset.galleryReady, undefined, { timeout: 60_000 });
      await page.locator(".cv-user__bubble").first().waitFor({ timeout: 30_000 });
      if (c.themeFile) {
        await page.evaluate(
          (t) => (window as unknown as { cmuxAcpmuxBridge: { applyTheme(t: unknown): void } }).cmuxAcpmuxBridge.applyTheme(t),
          themePayload(c.look),
        );
      }
      if (c.look !== "none") {
        const look = css(c.look);
        await page.evaluate((look) => {
          const stage = document.getElementById("acpmux-user-theme")?.textContent ?? "";
          (window as unknown as { cmuxAcpmuxBridge: { applyCustomization(c: { themeCSS: string }): void } })
            .cmuxAcpmuxBridge.applyCustomization({ themeCSS: `${stage}\n${look}` });
        }, look);
      }
      await page.evaluate(() => document.fonts.ready);
      await settle(page);
      await page.waitForTimeout(600);
      // Scroll the transcript to the top so the first prompt, tool rows and the code block show.
      await page.evaluate(() => {
        for (const el of document.querySelectorAll<HTMLElement>("*")) {
          if (el.scrollHeight > el.clientHeight + 20 && getComputedStyle(el).overflowY.match(/auto|scroll/))
            el.scrollTop = 0;
        }
      });
      await settle(page);
      writeFileSync(path.join(out, `${c.name}-top.png`), await page.screenshot());
      await page.evaluate(() => {
        for (const el of document.querySelectorAll<HTMLElement>("*")) {
          if (el.scrollHeight > el.clientHeight + 20 && getComputedStyle(el).overflowY.match(/auto|scroll/))
            el.scrollTop = el.scrollHeight;
        }
      });
      await settle(page);
      await page.waitForTimeout(300);
      writeFileSync(path.join(out, `${c.name}.png`), await page.screenshot());
      const info = await page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        const names = ["--cv-user-bubble-tint", "--cv-card-fill", "--cv-font", "--cv-font-size", "--cv-column", "--cv-text"];
        const bubble = document.querySelector(".cv-user__bubble");
        return {
          vars: Object.fromEntries(names.map((n) => [n, root.getPropertyValue(n).trim()])),
          bubble: bubble
            ? { bg: getComputedStyle(bubble).backgroundColor, color: getComputedStyle(bubble).color, font: getComputedStyle(bubble).fontFamily }
            : null,
          body: getComputedStyle(document.body).backgroundColor,
        };
      });
      // Open every "Worked for" disclosure, then put the second prompt at the top: the prompt, its
      // tool rows (edits, bun test), the reply and its code block in one frame.
      const toggles = page.getByText(/^Worked for/);
      const count = await toggles.count();
      for (let i = 0; i < count; i++) {
        await toggles.nth(i).click({ force: true }).catch((e) => errors.push(`click ${i}: ${String(e)}`));
        await page.waitForTimeout(400);
      }
      await settle(page);
      await page.evaluate(() => {
        const bubbles = document.querySelectorAll(".cv-user__bubble");
        (bubbles[1] ?? bubbles[0])?.scrollIntoView({ block: "start" });
      });
      await settle(page);
      await page.waitForTimeout(500);
      writeFileSync(path.join(out, `${c.name}-tools.png`), await page.screenshot());
      await page.evaluate(() => document.querySelectorAll(".cv-user__bubble")[0]?.scrollIntoView({ block: "start" }));
      await settle(page);
      await page.waitForTimeout(300);
      writeFileSync(path.join(out, `${c.name}-tools-first.png`), await page.screenshot());
      writeFileSync(path.join(out, `${c.name}.json`), JSON.stringify({ info, errors, toggles: count }, null, 2));
    } finally {
      await page.close();
    }
  });
}
