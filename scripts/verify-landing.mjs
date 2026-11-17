#!/usr/bin/env node
/**
 * Static acceptance checks for the standalone landing page. This does not replace
 * visual inspection; it catches broken local references, invented benchmark values,
 * external asset requests and missing accessibility hooks.
 */
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const landing = path.join(root, "landing");
const errors = [];

function requireFile(relative) {
  const full = path.join(root, relative);
  if (!existsSync(full)) errors.push(`missing file: ${relative}`);
  return existsSync(full) ? readFileSync(full, "utf8") : "";
}

const html = requireFile("landing/index.html");
const css = requireFile("landing/styles.css");
const js = requireFile("landing/main.js");
const dataPath = path.join(landing, "data.js");
const releasePath = path.join(landing, "release.js");
requireFile("landing/data.js");
requireFile("landing/release.js");
requireFile("landing/benchmarks.js");

if (!errors.length) {
  // 1. Local asset references must resolve.
  for (const match of html.matchAll(/src="([^"]+)"/g)) {
    const ref = match[1];
    if (/^https?:/i.test(ref)) {
      errors.push(`external image request: ${ref}`);
      continue;
    }
    if (!existsSync(path.join(landing, ref))) errors.push(`missing local asset: ${ref}`);
  }

  // 2. Anchor targets must exist.
  for (const match of html.matchAll(/href="#([^"]+)"/g)) {
    if (!new RegExp(`id="${match[1]}"`).test(html)) errors.push(`missing anchor target: #${match[1]}`);
  }

  // 3. No remote fonts or CSS imports; zero external asset dependencies.
  if (/@import\s+url\(\s*['"]?https?:/i.test(css) || /url\(\s*['"]?https?:/i.test(css)) {
    errors.push("CSS requests a remote asset");
  }
  if (/fonts\.googleapis|fonts\.gstatic/i.test(html + css)) errors.push("remote font reference found");

  // 4. Benchmark values must match docs/BENCHMARKS.md.
  const benchDoc = requireFile("docs/BENCHMARKS.md");
  for (const token of ["20,557", "17,511", "0.21s", "0.36s", "8.4 MiB", "11.1 MiB"]) {
    if (!benchDoc.includes(token)) errors.push(`docs/BENCHMARKS.md no longer contains ${token}`);
  }

  // 5. Data honesty: only Cyron has a measured timing; competitors are Not run.
  const { BENCHMARKS, COMPARISON, TOOLS, LEGEND } = await import(pathToFileURL(dataPath));
  if (BENCHMARKS.length !== 3) errors.push("expected three Cyron benchmark rows");
  const expected = [
    [20557, 0.21],
    [17511, 0.36],
    [72, 0.001],
  ];
  BENCHMARKS.forEach((row, index) => {
    if (row.files !== expected[index][0]) errors.push(`unexpected file count in benchmark ${index}`);
    if (row.median !== expected[index][1]) errors.push(`unexpected median in benchmark ${index}`);
  });
  if (TOOLS.join("|") !== "Cyron|Revo|BCUninstaller|WizTree|BleachBit") {
    errors.push("comparison tools changed unexpectedly");
  }
  const timingRow = COMPARISON[COMPARISON.length - 1];
  if (timingRow.values[0] !== "measured") errors.push("Cyron timing row is not marked measured");
  if (timingRow.values.slice(1).some((value) => value !== "notrun")) {
    errors.push("a competitor timing is claimed without a run");
  }
  for (const row of COMPARISON) {
    if (row.values.length !== TOOLS.length) errors.push(`comparison row width mismatch: ${row.label}`);
    for (const value of row.values) {
      if (!(value in LEGEND)) errors.push(`unmapped comparison value: ${value}`);
    }
  }

  // 5b. Release metadata is generated from the tag and must match its own URLs.
  const { RELEASE } = await import(pathToFileURL(releasePath));
  if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(RELEASE.tag)) {
    errors.push(`invalid release tag: ${RELEASE.tag}`);
  }
  if (RELEASE.version !== RELEASE.tag.replace(/^v/, "")) {
    errors.push("release version does not match its tag");
  }
  if (RELEASE.releaseUrl !== `https://github.com/${RELEASE.repo}/releases/tag/${RELEASE.tag}`) {
    errors.push("release URL does not point at the generated tag");
  }
  if (RELEASE.notesUrl !== RELEASE.releaseUrl && !RELEASE.notesUrl.includes(`/${RELEASE.tag}/`)) {
    errors.push("release notes URL is not pinned to the generated tag");
  }
  if (!RELEASE.benchmarkUrl.includes(`/${RELEASE.tag}/docs/BENCHMARKS.md`)) {
    errors.push("benchmark source URL is not pinned to the generated tag");
  }

  // 5c. Generated benchmark and release data must be current with the repository.
  const sync = spawnSync(
    process.execPath,
    [path.join(root, "scripts", "sync-landing.mjs"), "--check"],
    { cwd: root, encoding: "utf8" },
  );
  if (sync.status !== 0) {
    errors.push(`landing data is stale: ${(sync.stderr || sync.stdout).trim()}`);
  }

  // 6. Prohibited claims: the page must not invent a speed or quality ranking.
  const copy = html + "\n" + js + "\n" + css;
  const banned = [
    /faster than (revo|bcuninstaller|wiztree|treesize|bleachbit|pc manager|sysinternals)/i,
    /beats (revo|bcuninstaller|wiztree|treesize|bleachbit)/i,
    /world'?s fastest/i,
    /guaranteed speed/i,
    /#1 (uninstaller|cleaner|scanner)/i,
  ];
  for (const pattern of banned) {
    if (pattern.test(copy)) errors.push(`prohibited claim matched ${pattern}`);
  }
  if (!html.includes("No competitor benchmark exists.")) {
    errors.push("missing the no-competitor-benchmark disclosure");
  }

  // 7. Accessibility and progressive enhancement hooks.
  for (const token of [
    'class="skip-link"',
    '<main id="main"',
    "<h1",
    'aria-label="Capability comparison table"',
    'data-mode="stress"',
    "prefers-reduced-motion: reduce",
    "prefers-reduced-transparency: reduce",
    'data-pref="theme"',
    'data-pref="motion"',
    'data-pref="system"',
    'id="vibecode"',
    "prefers-color-scheme: dark",
    "data-release-version",
    "data-release-download",
    "data-release-notes",
    "data-benchmark-source",
    "data-bench-chip",
    "data-bench-peaks",
  ]) {
    if (!(html + css + js).includes(token)) errors.push(`missing required hook: ${token}`);
  }

  // 8. Appearance must be decided before the stylesheet paints, or dark mode flashes white.
  const head = html.slice(0, html.indexOf("<body"));
  const stylesheetIndex = head.indexOf('rel="stylesheet"');
  const themeScriptIndex = head.indexOf("cyron-theme");
  if (stylesheetIndex === -1) errors.push("stylesheet link not found in head");
  if (themeScriptIndex === -1 || themeScriptIndex > stylesheetIndex) {
    errors.push("the theme script must run before the stylesheet to prevent a white flash");
  }
  if (!/<button[^>]*data-pref="theme"/.test(html)) errors.push("theme preference is not a button");
  if (!/<button[^>]*data-pref="motion"/.test(html)) errors.push("motion preference is not a button");
  if (!/<a[^>]*data-release-download/.test(html)) errors.push("download link is not wired to release data");
  if (!/<a[^>]*data-release-notes/.test(html)) errors.push("release notes link is not wired to release data");
  if (!/isDev[\s\S]*localhost/.test(js)) errors.push("stress toggle is not gated to local development");
  if (!/["']data["'][\s\S]*["']worst["']/.test(js)) errors.push("stress mode is not addressable by URL");
  if (/<img(?![^>]*\balt=)/.test(html)) errors.push("an image is missing alt text");

  // 9. JavaScript syntax.
  for (const file of ["landing/main.js", "landing/data.js", "scripts/verify-landing.mjs"]) {
    const result = spawnSync(process.execPath, ["--check", path.join(root, file)], { encoding: "utf8" });
    if (result.status !== 0) errors.push(`syntax error in ${file}: ${result.stderr.trim()}`);
  }
}

if (errors.length) {
  console.error("LANDING_VERIFY_FAILED");
  for (const error of errors) console.error(` - ${error}`);
  process.exit(1);
}

console.log(`LANDING_VERIFY_OK (${html.length} html, ${css.length} css, ${js.length} js)`);
