import assert from "node:assert/strict";
import fs from "node:fs";
import { browser } from "@wdio/globals";
import {
  fixture,
  waitText,
  waitHeading,
  nav,
  clickButton,
  clickElement,
  clickRow,
  clickWithin,
  selectValue,
  searchApplications,
  waitRow,
  rowFor,
  rowNames,
  clickRowCheckbox,
  screenshot,
  enableCleaning,
  registryKeyExists,
  setWindow,
} from "./helpers.mjs";

describe("Cyron desktop core journeys", () => {
  before(async () => {
    await waitHeading("Home", 60_000);
    await setWindow(1320, 860);
  });

  afterEach(async () => {
    await browser.keys("Escape").catch(() => {});
    await browser.pause(150);
  });

  it("first launch and every destination render real content", async () => {
    assert.match(await browser.getTitle(), /Cyron/);
    assert.equal(
      await browser.execute(() => document.body.innerText.includes("Review before cleanup")),
      false,
    );
    await screenshot("01-home-light.png");

    await nav("Applications");
    await waitText("Installed applications");
    await screenshot("02-applications-light.png");

    await nav("Storage & Cleanup");
    await waitText("Caches & leftovers");
    await screenshot("03-storage-loading-light.png");

    await nav("Performance");
    await waitText("Why is my PC slow?");
    await screenshot("04-performance-loading-light.png");

    await nav("History & Recovery");
    await waitText("Recorded operations");
    await screenshot("05-history-empty-light.png");

    await nav("Settings");
    await waitText("Appearance, scan scope and safety");
    await screenshot("06-settings-light.png");

    await nav("Home");
    await screenshot("07-home-return-light.png");
  });

  it("Applications search, broken filter, sort and selection respond to real inventory", async () => {
    await nav("Applications");
    await waitRow(fixture.apps.removable.name, 60_000);
    await searchApplications("Cyron Desktop E2E");
    await waitRow(fixture.apps.remaining.name);
    assert.equal((await rowNames()).length, 4);

    await selectValue('select[aria-label="Filter by application source"]', "broken");
    assert.ok(await rowFor(fixture.apps.broken.name));
    assert.equal(await rowFor(fixture.apps.removable.name), false);
    await screenshot("08-applications-broken-filter.png");

    await selectValue('select[aria-label="Filter by application source"]', "all");
    await waitRow(fixture.apps.removable.name);
    const nameHeader = await browser.$('th[title="Sort by application name"]');
    await clickElement(nameHeader);
    await browser.pause(200);
    assert.equal((await rowNames())[0], fixture.apps.removable.name);
    await screenshot("09-applications-sorted.png");

    await clickRowCheckbox(fixture.apps.remaining.name);
    await waitText("1 application selected");
    await clickRowCheckbox(fixture.apps.cancelled.name);
    await waitText("2 applications selected");
    await clickButton("Clear");
    await waitText("Installed applications");
  });

  it("batch uninstall reports verified, remaining and cancelled outcomes", async () => {
    await nav("Applications");
    await waitRow(fixture.apps.remaining.name, 60_000);
    await searchApplications("Cyron Desktop E2E");
    await clickRowCheckbox(fixture.apps.remaining.name);
    await clickRowCheckbox(fixture.apps.cancelled.name);
    await waitText("2 applications selected");
    await clickButton("Batch Uninstall…");
    await waitText("Uninstall selected apps");
    await waitText("Queued applications");

    const confirmations = await browser.$$("label.un-confirm-box input");
    const visible = [];
    for (const input of confirmations) {
      if (await input.isDisplayed()) visible.push(input);
    }
    await clickElement(visible[visible.length - 1]);
    await clickButton("Start Batch Uninstall (2)");
    await waitText("Batch finished:", 120_000);
    const text = await browser.$("body").getText();
    assert.match(text, /1 still registered/);
    assert.match(text, /1 cancelled/);
    assert.match(text, /0 failed/);
    assert.ok(fs.existsSync(fixture.apps.remaining.marker));
    assert.ok(fs.existsSync(fixture.apps.cancelled.marker));
    await screenshot("10-batch-uninstall-result.png");
  });

  it("single uninstall launches the registered fixture uninstaller and verifies removal", async () => {
    await nav("Applications");
    await searchApplications(fixture.apps.removable.name);
    await waitRow(fixture.apps.removable.name, 60_000);
    await clickRow(fixture.apps.removable.name);
    const drawerNav = await browser.$(".un-drawer-nav");
    await drawerNav.waitForDisplayed();
    await clickWithin(drawerNav, "Uninstall");
    await waitText("Validated command line", 30_000);

    const confirm = await browser.$("label.un-confirm-box input");
    await clickElement(confirm);
    await clickButton("Launch uninstaller");
    await waitText("Scan for leftovers now", 120_000);
    assert.ok(fs.existsSync(fixture.apps.removable.marker));
    assert.equal(registryKeyExists(fixture.apps.removable.key), false);
    await screenshot("11-single-uninstall-result.png");

    await clickButton("Close");
    await waitText("Installed applications");
    await searchApplications(fixture.apps.removable.name);
    await browser.pause(400);
    assert.equal(await rowFor(fixture.apps.removable.name), false);
  });

  it("broken entry removal backs up the registry and restores it", async () => {
    await enableCleaning();
    await nav("Applications");
    await searchApplications(fixture.apps.broken.name);
    await waitRow(fixture.apps.broken.name, 60_000);
    await clickRow(fixture.apps.broken.name);
    await clickButton("Clean Broken Entry (Forced Removal)…");
    await waitText("Clean broken entry only");
    const cleanRadio = await browser.$('//label[contains(., "Clean broken entry only")]');
    await clickElement(cleanRadio);
    await clickButton("Review Ghost Entry →");
    await waitText("Verified Registry Entry Confirmation:", 30_000);
    assert.equal(registryKeyExists(fixture.apps.broken.key), true);
    await clickButton("Confirm & Clean Entry");
    await waitText("Restore Registry Now", 60_000);
    assert.equal(registryKeyExists(fixture.apps.broken.key), false);
    await screenshot("12-broken-entry-removed.png");

    await clickButton("Restore Registry Now");
    await waitText("Successfully restored", 30_000);
    assert.equal(registryKeyExists(fixture.apps.broken.key), true);
    await screenshot("13-broken-entry-restored.png");
    await clickButton("Done");
    await waitText("Installed applications");
    await searchApplications(fixture.apps.broken.name);
    await waitRow(fixture.apps.broken.name);
  });
});
