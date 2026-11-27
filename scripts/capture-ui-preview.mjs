#!/usr/bin/env node
/**
 * Capture documentation screenshots of the React frontend in headless Chromium.
 *
 * The Tauri IPC is replaced by scripts/preview/mock-tauri.js, so every number,
 * path and process in these images is sample data. They show layout and
 * styling only and must be labelled as previews, never as native captures.
 *
 *   npm run preview:screenshots            # writes docs/screenshots/*.png
 *   npm run preview:screenshots -- --scale 2 --themes light,dark --out <dir>
 *
 * --themes captures every view once per theme as <name>-<theme>.png, which is
 * how the landing page gets matching light and dark images.
 *
 * Needs Playwright with Chromium (`npx playwright install chromium`).
 */
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : fallback;
};
const outDir = path.resolve(root, option("out", "docs/screenshots"));
const scale = Number(option("scale", "1"));
const only = option("only", "");
const themes = option("themes", "").split(",").filter(Boolean);
const port = Number(option("port", "1427"));
const mock = path.join(root, "scripts", "preview", "mock-tauri.js");

/**
 * Each shot: a theme, a viewport height and the steps that reach the view.
 * Kinds: nav (sidebar item), tab (segmented control), text, button, fill, key,
 * into (scroll an element into view) and live (seconds of monitor sampling).
 */
const SHOTS = [
  { name: "home", steps: [] },
  { name: "applications", steps: [["nav", "Applications"]] },
  {
    name: "search",
    steps: [
      ["nav", "Search"],
      ["fill", 'input[type="search"], input', "report"],
      ["key", "Enter"],
    ],
  },
  { name: "cleanup", steps: [["nav", "Storage & Cleanup"]] },
  {
    name: "disk-map",
    height: 1000,
    steps: [
      ["nav", "Storage & Cleanup"],
      ["tab", "Disk space"],
      ["button", "Scan"],
      ["into", "canvas"],
    ],
  },
  {
    name: "disk-map-dark",
    theme: "dark",
    height: 1000,
    steps: [
      ["nav", "Storage & Cleanup"],
      ["tab", "Disk space"],
      ["button", "Scan"],
      ["into", "canvas"],
    ],
  },
  {
    name: "performance-cpu",
    steps: [
      ["nav", "Performance"],
      ["live", 70],
    ],
  },
  {
    name: "performance-cores-dark",
    theme: "dark",
    steps: [
      ["nav", "Performance"],
      ["live", 70],
      ["text", "Logical processors (16)"],
    ],
  },
  {
    name: "performance-memory",
    steps: [
      ["nav", "Performance"],
      ["live", 70],
      ["text", "Memory"],
    ],
  },
  {
    name: "performance-gpu-dark",
    theme: "dark",
    steps: [
      ["nav", "Performance"],
      ["live", 70],
      ["text", "GPU 0"],
    ],
  },
  {
    name: "performance-processes",
    steps: [
      ["nav", "Performance"],
      ["live", 4],
      ["tab", "Processes"],
      ["live", 3],
    ],
  },
  {
    name: "recovery",
    steps: [
      ["nav", "History & Recovery"],
      ["tab", "Recovery"],
    ],
  },
];

function startVite() {
  const vite = spawn(
    process.execPath,
    [
      path.join(root, "node_modules", "vite", "bin", "vite.js"),
      "--port",
      String(port),
      "--strictPort",
    ],
    {
      cwd: root,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("Vite did not start within 30 s")),
      30000,
    );
    vite.stdout.on("data", (chunk) => {
      if (String(chunk).includes("Local")) {
        clearTimeout(timer);
        resolve(vite);
      }
    });
    vite.on("exit", (code) => reject(new Error(`Vite exited with ${code}`)));
  });
}

async function capture(browser, shot) {
  const page = await browser.newPage({
    viewport: { width: 1440, height: shot.height ?? 920 },
    deviceScaleFactor: scale,
    colorScheme: shot.theme ?? "light",
  });
  page.on("pageerror", (error) =>
    console.error(`[${shot.name}] ${error.message}`),
  );
  await page.clock.install();
  await page.addInitScript({ path: mock });
  await page.goto(`http://localhost:${port}/`);
  await page.clock.runFor(1500);

  for (const [kind, value, extra] of shot.steps) {
    if (kind === "nav")
      await page
        .locator("nav, aside")
        .getByText(value, { exact: true })
        .first()
        .click();
    if (kind === "text")
      await page.getByText(value, { exact: true }).last().click();
    if (kind === "tab")
      await page.getByRole("tab", { name: value, exact: true }).first().click();
    if (kind === "button")
      await page
        .getByRole("button", { name: value, exact: true })
        .first()
        .click();
    if (kind === "fill") await page.locator(value).first().fill(extra);
    if (kind === "key") await page.keyboard.press(value);
    if (kind === "into")
      await page
        .locator(value)
        .first()
        .evaluate((node) => node.scrollIntoView({ block: "end" }));
    // The live monitor polls once a second; fast-forward to fill its 60-second history.
    if (kind === "live")
      for (let i = 0; i < value; i += 1) await page.clock.runFor(1000);
    await page.clock.runFor(600);
  }
  await page.waitForTimeout(250);
  const file = path.join(outDir, `${shot.name}.png`);
  await page.screenshot({ path: file });
  await page.close();
  console.log(`wrote ${path.relative(root, file)}`);
}

mkdirSync(outDir, { recursive: true });
const vite = await startVite();
const browser = await chromium.launch();
try {
  for (const shot of SHOTS) {
    if (only && !only.split(",").includes(shot.name)) continue;
    if (!themes.length) {
      await capture(browser, shot);
      continue;
    }
    // Per-theme mode: one capture per theme for each distinct view.
    if (
      shot.name.endsWith("-dark") &&
      SHOTS.some((other) => other.name === shot.name.slice(0, -5))
    )
      continue;
    const base = shot.name.replace(/-dark$/, "");
    for (const theme of themes)
      await capture(browser, { ...shot, name: `${base}-${theme}`, theme });
  }
} finally {
  await browser.close();
  vite.kill();
}
