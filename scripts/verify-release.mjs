#!/usr/bin/env node
/**
 * Verify a published GitHub release from this repository.
 *
 * Checks the tag, that it is a published (non-draft, non-prerelease) release,
 * and that the asset set is exactly the NSIS setup, the WiX MSI, the standalone
 * executable and SHA256SUMS.txt for that version.
 *
 *   node scripts/verify-release.mjs v0.2.1
 *   node scripts/verify-release.mjs            # uses package.json version
 *
 * Requires an authenticated `gh` CLI (it is only used to read release data).
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
const tag = process.argv[2] ?? `v${pkg.version}`;
const version = tag.replace(/^v/, "");

function fail(message) {
  console.error("RELEASE_VERIFY_FAILED");
  console.error(` - ${message}`);
  process.exit(1);
}

if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) {
  fail(`"${tag}" is not a release tag such as v0.2.1`);
}

const expectedAssets = [
  `Cyron.Uninstaller_${version}_x64-setup.exe`,
  `Cyron.Uninstaller_${version}_x64_en-US.msi`,
  "SHA256SUMS.txt",
  "cyron.exe",
].sort();

const result = spawnSync(
  "gh",
  ["release", "view", tag, "--json", "tagName,isDraft,isPrerelease,assets"],
  { cwd: root, encoding: "utf8" },
);

if (result.error) fail(`cannot run gh: ${result.error.message}`);
if (result.status !== 0) fail((result.stderr || result.stdout).trim() || `gh release view ${tag} failed`);

let release;
try {
  release = JSON.parse(result.stdout);
} catch (error) {
  fail(`gh returned unparsable JSON: ${error.message}`);
}

if (release.tagName !== tag) fail(`release tag is "${release.tagName}", expected "${tag}"`);
if (release.isDraft) fail("release is still a draft");
if (release.isPrerelease) fail("release is marked as a prerelease");

const assets = Array.isArray(release.assets) ? release.assets : [];
for (const asset of assets) {
  if (!asset.size) fail(`asset "${asset.name}" is empty`);
}
const names = assets.map((asset) => asset.name).sort();
if (names.join("|") !== expectedAssets.join("|")) {
  fail(`asset set is "${names.join(", ")}", expected "${expectedAssets.join(", ")}"`);
}

console.log(`RELEASE_VERIFIED ${tag} ${assets.length} assets`);
