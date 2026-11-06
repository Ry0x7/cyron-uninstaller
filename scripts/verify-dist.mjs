#!/usr/bin/env node
// Builds the frontend, then proves the emitted bundle exists and is wired up.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(root, "dist");

function fail(message) {
  console.error(`DIST_FAIL: ${message}`);
  process.exit(1);
}

const build = spawnSync("npm run build", {
  cwd: root,
  stdio: "inherit",
  shell: true,
});
if (build.error) fail(`could not start the build: ${build.error.message}`);
if (build.status !== 0) fail(`npm run build exited with status ${build.status}`);

const indexPath = path.join(dist, "index.html");
if (!existsSync(indexPath)) fail("dist/index.html was not emitted");

const html = readFileSync(indexPath, "utf8");
const scripts = [...html.matchAll(/<script\b[^>]*\bsrc="([^"]+)"/g)].map((match) => match[1]);
const stylesheets = [...html.matchAll(/<link\b[^>]*>/g)]
  .filter((match) => /\brel="stylesheet"/.test(match[0]))
  .map((match) => /\bhref="([^"]+)"/.exec(match[0])?.[1] ?? "")
  .filter(Boolean);

if (scripts.length === 0) fail("dist/index.html references no script");
if (stylesheets.length === 0) fail("dist/index.html references no stylesheet");

let assetCount = 0;
let totalBytes = 0;

for (const [kind, sources] of /** @type {const} */ ([
  ["script", scripts],
  ["stylesheet", stylesheets],
])) {
  for (const source of sources) {
    if (/^[a-z]+:/i.test(source)) fail(`${kind} points outside the bundle: ${source}`);
    const relative = source.replace(/^\/+/, "").split("?")[0];
    if (!/^assets[\\/]/.test(relative)) fail(`${kind} is not a hashed assets/ path: ${source}`);
    const file = path.resolve(dist, relative);
    if (!file.startsWith(dist + path.sep)) fail(`${kind} escapes the bundle: ${source}`);
    if (!existsSync(file)) fail(`${kind} does not exist on disk: ${source}`);
    const size = statSync(file).size;
    if (size === 0) fail(`${kind} is empty: ${source}`);
    if (!/\.(js|css)$/i.test(file)) fail(`${kind} has an unexpected extension: ${source}`);
    assetCount += 1;
    totalBytes += size;
  }
}

if (assetCount < 2) fail(`expected at least a JS and a CSS asset, found ${assetCount}`);

console.log(
  `bundle ok: ${assetCount} assets, ${(totalBytes / 1024).toFixed(1)} KiB, index.html ${statSync(indexPath).size} bytes`,
);
console.log("DIST_OK");
