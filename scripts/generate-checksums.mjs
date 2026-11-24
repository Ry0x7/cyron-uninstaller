#!/usr/bin/env node
/**
 * Generate SHA-256 checksums for the current release artifacts.
 *
 * GitHub replaces spaces in uploaded asset names with periods, so entries use
 * the name users actually download. Pass --output to place the file next to the
 * release bundles; the default is the repository root.
 *
 *   node scripts/generate-checksums.mjs
 *   node scripts/generate-checksums.mjs --output src-tauri/target/release/bundle/SHA256SUMS.txt
 */
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const outputIndex = args.indexOf("--output");
const outputPath = path.resolve(
  root,
  outputIndex >= 0 ? args[outputIndex + 1] : "SHA256SUMS.txt",
);
const requireUpdater = args.includes("--require-updater");

const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const version = pkg.version;
const bundleDir = path.join(root, "src-tauri", "target", "release", "bundle");
const nsisDir = path.join(bundleDir, "nsis");
const msiDir = path.join(bundleDir, "msi");

function fail(message) {
  console.error("CHECKSUMS_FAILED");
  console.error(` - ${message}`);
  process.exit(1);
}

function findOne(dir, predicate, label) {
  if (!fs.existsSync(dir)) fail(`${label} directory not found: ${dir}`);
  const matches = fs.readdirSync(dir).filter(predicate).sort();
  if (matches.length !== 1) fail(`expected one ${label} in ${dir}, found ${matches.length}`);
  return path.join(dir, matches[0]);
}

const files = [
  findOne(nsisDir, (name) => name.includes(version) && name.endsWith("-setup.exe"), "NSIS setup"),
  findOne(msiDir, (name) => name.includes(version) && name.endsWith(".msi"), "WiX MSI"),
];
const mainExe = path.join(root, "src-tauri", "target", "release", "cyron.exe");
if (!fs.existsSync(mainExe)) fail(`main executable not found: ${mainExe}`);
files.push(mainExe);

const updaterManifest = path.join(bundleDir, "latest.json");
if (fs.existsSync(updaterManifest)) files.push(updaterManifest);
const updaterSignature = path.join(nsisDir, `${path.basename(files[0])}.sig`);
if (requireUpdater && (!fs.existsSync(updaterManifest) || !fs.existsSync(updaterSignature))) {
  fail("latest.json or the NSIS updater signature is missing; build with npm run build:release");
}

const lines = files.map((file) => {
  const hash = crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
  return `${hash}  ${path.basename(file).replace(/ /g, ".")}`;
});

fs.writeFileSync(outputPath, `${lines.join("\n")}\n`, "utf8");
console.log(`CHECKSUMS_OK ${outputPath}`);
console.log(lines.join("\n"));
