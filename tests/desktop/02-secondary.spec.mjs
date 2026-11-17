import assert from "node:assert/strict";
import { browser } from "@wdio/globals";
import {
  fixture,
  waitText,
  waitHeading,
  nav,
  clickButton,
  searchApplications,
  screenshot,
  noHorizontalOverflow,
  setWindow,
  setTheme,
} from "./helpers.mjs";

describe("Cyron desktop secondary journeys and visual matrix", () => {
  afterEach(async () => {
    await browser.keys("Escape").catch(() => {});
    await browser.pause(150);
  });

  it("light and dark destinations fit the minimum and a large window", async function () {
    this.timeout(600_000);
    await setWindow(1000, 680);
    await nav("Applications");
    await searchApplications("");
    for (const label of [
      "Home",
      "Applications",
      "Storage & Cleanup",
      "Performance",
      "History & Recovery",
      "Settings",
    ]) {
      await nav(label);
      await noHorizontalOverflow(`${label} at 1000x680`);
    }
    await nav("Home");
    await screenshot("20-home-minimum.png");

    const light = await setTheme("Light");
    assert.equal(light, "light");
    await setWindow(1440, 960);
    for (const [label, name] of [
      ["Home", "21-home-light.png"],
      ["Applications", "22-applications-light.png"],
      ["Storage & Cleanup", "23-storage-light.png"],
      ["Performance", "24-performance-light.png"],
      ["History & Recovery", "25-history-light.png"],
      ["Settings", "26-settings-light.png"],
    ]) {
      await nav(label);
      await noHorizontalOverflow(`${label} at 1440x960 light`);
      await screenshot(name);
    }

    const dark = await setTheme("Dark");
    assert.equal(dark, "dark");
    await nav("Home");
    await noHorizontalOverflow("Home dark");
    await screenshot("27-home-dark.png");
    await setTheme("System");
    await setWindow(1320, 860);
  });

  it("Settings persists the chosen theme across a real reload", async () => {
    await nav("Settings");
    await setTheme("Light");
    assert.equal(await browser.execute(() => localStorage.getItem("cyron.settings.v1") !== null), true);
    assert.equal(await browser.execute(() => document.documentElement.dataset.theme), "light");

    await browser.refresh();
    await waitHeading("Home", 60_000);
    await nav("Settings");
    const pressed = await browser.execute(() => {
      const group = document.querySelector('div[aria-label="Theme"]');
      return [...(group?.querySelectorAll("button") ?? [])].map((button) => ({
        label: button.textContent?.trim(),
        pressed: button.getAttribute("aria-pressed") === "true",
      }));
    });
    assert.deepEqual(
      pressed.find((entry) => entry.pressed)?.label,
      "Light",
    );
    await setTheme("System");
  });

  it("Storage disk-space, Performance evidence and History render measured content", async () => {
    await nav("Storage & Cleanup");
    await waitText("Caches & leftovers");
    await clickButton("Disk space");
    await waitText("Choose a drive to map everything on it.");
    await noHorizontalOverflow("Storage disk space");
    await screenshot("28-storage-disk-space.png");
    await clickButton("Cleanup");
    await waitText("Caches & leftovers");

    await nav("Performance");
    await waitText("Why is my PC slow?", 60_000);
    await waitText("Live sample", 60_000);
    await waitText("Processes", 60_000);
    await clickButton("Startup takes forever");
    await waitText("Separate what starts automatically from what Windows measured");
    const body = await browser.$("body").getText();
    assert.match(body, /CPU/);
    assert.match(body, /available/);
    assert.match(body, /startup entries/);
    await noHorizontalOverflow("Performance evidence");
    await screenshot("29-performance-evidence.png");

    await nav("History & Recovery");
    await waitText("Recorded operations");
    await waitText("Operation history");
    await noHorizontalOverflow("History and recovery");
    await screenshot("30-history-records.png");
  });

  it("spike capture and kernel/driver evidence render from live data", async () => {
    await nav("Performance");
    await waitText("Catch a short spike", 60_000);
    await waitText("Kernel, drivers & recent changes", 60_000);
    await noHorizontalOverflow("Performance diagnostics");

    const evidence = await browser.execute(() => {
      const overview = document.querySelector(".perf-kernel-overview")?.innerText ?? "";
      return {
        overview,
        drivers: document.querySelectorAll(".perf-driver-list li").length,
        eventBlock:
          document.querySelector(".perf-diagnostics-grid .perf-evidence-block:last-child")
            ?.innerText ?? "",
      };
    });
    const listed = Number(evidence.overview.match(/Drivers listed\s+(\d+)/)?.[1] ?? NaN);
    assert.ok(Number.isFinite(listed), `driver count not readable: ${evidence.overview}`);
    assert.equal(evidence.drivers, Math.min(listed, 8));
    assert.match(evidence.eventBlock, /Driver and device errors from the last 7 days/);

    await browser.execute(() =>
      document.querySelector(".perf-diagnostics-grid")?.scrollIntoView({ block: "center" }),
    );
    await browser.pause(250);
    await screenshot("34-kernel-driver-evidence.png");

    await clickButton("CPU spikes / kernel time");
    await waitText("Separate kernel and driver work from applications");
    await clickButton("Record 20 seconds");
    await waitText("of 20 readings", 30_000);
    await browser.waitUntil(
      async () =>
        browser.execute(() => document.querySelectorAll(".spike__bar").length > 0),
      { timeout: 15_000, timeoutMsg: "spike capture produced no readings" },
    );
    await clickButton("Stop capture");
    await waitText("Peak kernel", 15_000);

    const spike = await browser.execute(() => document.querySelector(".spike")?.innerText ?? "");
    assert.match(spike, /Peak CPU/);
    assert.match(spike, /Peak kernel/);
    assert.match(spike, /Largest process/);
    assert.match(
      spike,
      /No material spike|Kernel\/driver activity|accounted for most|physical disk was|no single dominant/,
    );
    await browser.execute(() =>
      document.querySelector(".spike")?.scrollIntoView({ block: "center" }),
    );
    await browser.pause(250);
    await screenshot("35-spike-capture.png");
  });

  it("startup inventory shows real provenance for the disposable entry", async () => {
    await nav("Performance");
    await waitText("Startup and scheduled tasks", 60_000);
    await waitText(fixture.startup.name, 60_000);
    const row = await browser.$(
      `//tr[contains(., ${JSON.stringify(fixture.startup.name)})]`,
    );
    await row.waitForExist({ timeout: 30_000 });
    const text = await row.getText();
    assert.match(text, /Enabled/);
    assert.match(text, /This user/);
    await row.scrollIntoView();
    await screenshot("33-startup-inventory.png");
  });

  it("Hunter asks for confirmation, enters targeting and exits with Escape", async () => {
    await nav("Applications");
    await waitText("Installed applications");
    await clickButton("Find by window");
    await waitText("Point at a desktop window to inspect the application behind it.");
    await screenshot("31-hunter-confirmation.png");
    await clickButton("Start targeting");
    await waitText("Move over any desktop window", 30_000);
    await screenshot("32-hunter-targeting.png");
    await browser.keys("Escape");
    await waitHeading("Applications", 30_000);
    await waitText("Installed applications");
  });
});
