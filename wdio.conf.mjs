import fs from "node:fs";
import path from "node:path";

const appBinaryPath = path.resolve("src-tauri/target/debug/cyron.exe");
let originalSettings = null;
let originalCollapsed = null;

export const config = {
  runner: "local",
  specs: ["./tests/desktop/**/*.spec.mjs"],
  maxInstances: 1,
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": {
        application: appBinaryPath,
      },
    },
  ],
  services: [
    [
      "@wdio/tauri-service",
      {
        appBinaryPath,
        driverProvider: "external",
        autoInstallTauriDriver: true,
        autoDownloadEdgeDriver: true,
        captureBackendLogs: true,
        captureFrontendLogs: true,
      },
    ],
  ],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 600_000,
    grep: process.env.CYRON_E2E_GREP || undefined,
  },
  reporters: [
    "spec",
    [
      "json",
      {
        outputDir: "./test-results/desktop",
        outputFile: "results.json",
      },
    ],
  ],
  afterTest: async (test, _context, { error }) => {
    if (!error) return;
    try {
      fs.mkdirSync("test-results/desktop/failures", { recursive: true });
      const name = test.title.replace(/[^a-z0-9]+/gi, "-").slice(0, 80);
      await browser.saveScreenshot(`test-results/desktop/failures/${name}.png`);
    } catch {
      // Session may already be gone; the failure is reported by the runner.
    }
  },
  logLevel: "info",
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  before: async () => {
    originalSettings = await browser.execute(() => localStorage.getItem("cyron.settings.v1"));
    originalCollapsed = await browser.execute(() => localStorage.getItem("cyron.sidebar.collapsed"));
    fs.mkdirSync("test-results/desktop", { recursive: true });
    fs.writeFileSync(
      "test-results/desktop/original-settings.json",
      JSON.stringify({ settings: originalSettings, collapsed: originalCollapsed }, null, 2),
    );
    await browser.execute(() => {
      localStorage.setItem(
        "cyron.settings.v1",
        JSON.stringify({
          theme: "system",
          includeHidden: true,
          exclusions: [],
          projectRoots: [],
          readOnly: true,
        }),
      );
      localStorage.setItem("cyron.sidebar.collapsed", "0");
    });
    await browser.refresh();
    await browser.pause(600);
  },
  after: async () => {
    if (!browser.sessionId) return;
    await browser.execute(
      (settings, collapsed) => {
        if (settings === null) localStorage.removeItem("cyron.settings.v1");
        else localStorage.setItem("cyron.settings.v1", settings);
        if (collapsed === null) localStorage.removeItem("cyron.sidebar.collapsed");
        else localStorage.setItem("cyron.sidebar.collapsed", collapsed);
      },
      originalSettings,
      originalCollapsed,
    );
    await browser.refresh();
  },
};
