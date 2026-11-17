import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { browser } from "@wdio/globals";

export const fixture = JSON.parse(
  fs.readFileSync(process.env.CYRON_E2E_STATE, "utf8"),
);
export const resultsDir = process.env.CYRON_E2E_RESULTS;
export const evidenceDir = process.env.CYRON_E2E_EVIDENCE;
export const repoRoot = path.resolve(import.meta.dirname, "..", "..");
const uiaScript = path.join(repoRoot, "scripts", "desktop-uia.ps1");
const setWindowScript = path.join(repoRoot, "scripts", "set-window.ps1");

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function waitText(text, timeout = 30_000) {
  await browser.waitUntil(
    async () => browser.execute((value) => document.body.innerText.includes(value), text),
    { timeout, timeoutMsg: `timed out waiting for text: ${text}` },
  );
}

export async function waitNoText(text, timeout = 30_000) {
  await browser.waitUntil(
    async () => !(await browser.execute((value) => document.body.innerText.includes(value), text)),
    { timeout, timeoutMsg: `timed out waiting for text to disappear: ${text}` },
  );
}

export async function waitHeading(title, timeout = 30_000) {
  await browser.waitUntil(
    async () =>
      browser.execute((value) => {
        const active = document.querySelector('section.view-slot:not([hidden])');
        return active?.querySelector("h1")?.textContent?.trim() === value;
      }, title),
    { timeout, timeoutMsg: `heading did not become ${title}` },
  );
}

export async function clickElement(element) {
  // WebView2 pointer coordinates do not account for Windows display scaling,
  // so dispatch the click on the resolved element itself.
  await browser.execute((node) => node.click(), element);
}

export async function nav(label, title = label) {
  const button = await displayedButton(label, true);
  await clickElement(button);
  await waitHeading(title);
}

export async function displayedButton(text, exact = true) {
  return browser.waitUntil(
    async () => {
      const buttons = await browser.$$("button");
      for (const button of buttons) {
        if (!(await button.isDisplayed())) continue;
        const label = (await button.getText()).trim();
        if (exact ? label === text : label.includes(text)) return button;
      }
      return false;
    },
    { timeout: 15_000, timeoutMsg: `no displayed button ${exact ? "matching" : "containing"} ${text}` },
  );
}

export async function clickButton(text, exact = true) {
  const button = await displayedButton(text, exact);
  await clickElement(button);
}

export async function clickWithin(element, text) {
  const buttons = await element.$$("button");
  for (const button of buttons) {
    if ((await button.getText()).trim() === text) {
      await clickElement(button);
      return;
    }
  }
  throw new Error(`no button ${text} inside element`);
}

export async function selectValue(selector, value) {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (css, next) => {
          const node = document.querySelector(css);
          if (!node) return false;
          node.value = next;
          node.dispatchEvent(new Event("change", { bubbles: true }));
          return node.value === next;
        },
        selector,
        value,
      ),
    { timeout: 15_000, timeoutMsg: `could not select ${value} in ${selector}` },
  );
  await browser.pause(250);
}

export async function setInput(selector, value) {
  const input = await browser.$(selector);
  await input.waitForDisplayed({ timeout: 15_000 });
  await input.setValue(value);
}

export async function searchApplications(value) {
  await setInput('input[aria-label="Search installed applications"]', value);
  await browser.pause(450);
}

export async function rowFor(name) {
  return browser.execute(
    (label) => [...document.querySelectorAll("tr[aria-label]")].some((row) => row.getAttribute("aria-label") === label),
    `View details for ${name}`,
  );
}

export async function waitRow(name, timeout = 30_000) {
  await browser.waitUntil(
    async () =>
      browser.execute(
        (label) => [...document.querySelectorAll("tr[aria-label]")].some((row) => row.getAttribute("aria-label") === label),
        `View details for ${name}`,
      ),
    { timeout, timeoutMsg: `row did not appear: ${name}` },
  );
}

export async function rowNames() {
  return browser.execute(() =>
    [...document.querySelectorAll('tr[aria-label^="View details for "]')].map((row) =>
      row.getAttribute("aria-label").replace("View details for ", ""),
    ),
  );
}

export async function clickRow(name) {
  await waitRow(name);
  await browser.execute((label) => {
    const row = [...document.querySelectorAll("tr[aria-label]")].find(
      (node) => node.getAttribute("aria-label") === label,
    );
    row?.click();
  }, `View details for ${name}`);
}

export async function clickRowCheckbox(name) {
  await waitRow(name);
  await browser.execute((label) => {
    const row = [...document.querySelectorAll("tr[aria-label]")].find(
      (node) => node.getAttribute("aria-label") === label,
    );
    row?.querySelector('input[type="checkbox"]')?.click();
  }, `View details for ${name}`);
}

export async function screenshot(name) {
  fs.mkdirSync(evidenceDir, { recursive: true });
  fs.mkdirSync(path.join(resultsDir, "screens"), { recursive: true });
  await browser.saveScreenshot(path.join(evidenceDir, name));
  await browser.saveScreenshot(path.join(resultsDir, "screens", name));
}

export async function noHorizontalOverflow(label) {
  const values = await browser.execute(() => ({
    innerWidth: window.innerWidth,
    scrollWidth: document.documentElement.scrollWidth,
    bodyWidth: document.body.scrollWidth,
  }));
  if (values.scrollWidth > values.innerWidth + 1 || values.bodyWidth > values.innerWidth + 1) {
    throw new Error(
      `${label}: horizontal overflow at ${values.innerWidth}px (document ${values.scrollWidth}, body ${values.bodyWidth})`,
    );
  }
  return values;
}

export async function setWindow(width, height) {
  // WebView2's setWindowSize hook waits for a Tauri window-state plugin that
  // Cyron does not ship, so it can block indefinitely at the minimum size.
  // Resize the real native window instead, then let the webview settle.
  execFileSync(
    "powershell",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      setWindowScript,
      "-Width",
      String(width),
      "-Height",
      String(height),
    ],
    { stdio: "ignore" },
  );
  await browser.pause(500);
  return browser.execute(() => [window.innerWidth, window.innerHeight]);
}

export async function setTheme(label) {
  await nav("Settings");
  const group = await browser.$('div[aria-label="Theme"]');
  await group.waitForDisplayed({ timeout: 15_000 });
  await clickWithin(group, label);
  await browser.pause(250);
  return browser.execute(() => document.documentElement.dataset.theme);
}

export async function enableCleaning() {
  await nav("Settings");
  await clickButton("Safety");
  const changed = await browser.execute(() => {
    const row = [...document.querySelectorAll(".toggle-row")].find((node) =>
      node.textContent?.includes("Cleaning enabled"),
    );
    const toggle = row?.querySelector('button[role="switch"]');
    if (!toggle) return false;
    if (toggle.getAttribute("aria-checked") === "true") return true;
    toggle.click();
    return true;
  });
  if (!changed) throw new Error("cleaning toggle not found in Settings");
  await browser.waitUntil(
    async () =>
      browser.execute(() => {
        const toggle = [...document.querySelectorAll('button[role="switch"]')].find((node) =>
          node.closest(".toggle-row")?.textContent?.includes("Cleaning enabled"),
        );
        return toggle?.getAttribute("aria-checked") === "true";
      }),
    { timeout: 10_000, timeoutMsg: "cleaning did not become enabled" },
  );
}

export function registryKeyExists(subkey) {
  const output = execFileSync(
    "powershell",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `if (Test-Path 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${subkey}') { 'yes' } else { 'no' }`,
    ],
    { encoding: "utf8", timeout: 15_000 },
  );
  return output.trim() === "yes";
}

export function startupApprovedByte(name) {
  const output = execFileSync(
    "powershell",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `$v=(Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run' -Name '${name}' -ErrorAction SilentlyContinue).'${name}'; if ($v) { $v[0] } else { 'missing' }`,
    ],
    { encoding: "utf8", timeout: 15_000 },
  );
  return output.trim();
}

export function uia(action, options = {}) {
  const args = [
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    uiaScript,
    "-Action",
    action,
  ];
  for (const [key, value] of Object.entries(options)) {
    if (value !== undefined) args.push(`-${key}`, String(value));
  }
  return execFileSync("powershell", args, { encoding: "utf8", timeout: 30_000 });
}

export async function waitNativeWindow(titleRegex, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const windows = uia("windows");
    if (windows.split("\n").some((line) => titleRegex.test(line))) return windows;
    await sleep(250);
  }
  throw new Error(`native window did not appear: ${titleRegex}`);
}

export async function nativeAsk(titleRegex, buttonName) {
  await waitNativeWindow(titleRegex);
  return uia("invoke", { TitleRegex: titleRegex.source, NameRegex: `^${buttonName}$` });
}

export function getLocalStorageValue(key) {
  return browser.execute((storageKey) => localStorage.getItem(storageKey), key);
}

export function setLocalStorageValue(key, value) {
  return browser.execute(
    (storageKey, storageValue) => {
      if (storageValue === null) localStorage.removeItem(storageKey);
      else localStorage.setItem(storageKey, storageValue);
    },
    key,
    value,
  );
}
