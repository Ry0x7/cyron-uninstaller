#!/usr/bin/env node
/**
 * Verify the release updater artifacts before upload.
 *
 * Checks that the NSIS setup used as the updater package exists, its detached
 * signature is embedded unchanged in latest.json, and the manifest points at
 * this version's GitHub release asset.
 */
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
const version = pkg.version;
const tag = process.argv[2] ?? `v${version}`;

function fail(message) {
  console.error("UPDATER_ARTIFACTS_FAILED");
  console.error(` - ${message}`);
  process.exit(1);
}

const repository =
  typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url ?? "";
const match = repository.match(/github\.com[/:]([^/]+)\/([^/]+?)(?:\.git)?\/?$/);
if (!match) fail(`cannot derive a GitHub repository from "${repository}"`);
const slug = `${match[1]}/${match[2]}`;

const bundleDir = path.join(root, "src-tauri", "target", "release", "bundle");
const manifestPath = path.join(bundleDir, "latest.json");
const nsisDir = path.join(bundleDir, "nsis");

if (!existsSync(manifestPath)) fail(`missing updater manifest: ${manifestPath}`);
if (!existsSync(nsisDir)) fail(`missing NSIS bundle directory: ${nsisDir}`);

let manifest;
try {
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (error) {
  fail(`latest.json is not valid JSON: ${error.message}`);
}

if (manifest.version !== version) {
  fail(`latest.json version "${manifest.version}" != package.json "${version}"`);
}
const platform = manifest.platforms?.["windows-x86_64-nsis"];
if (!platform) fail("latest.json has no windows-x86_64-nsis platform");
if (manifest.platforms?.["windows-x86_64"]) {
  fail("latest.json has a generic windows-x86_64 entry; MSI installs would receive the NSIS package");
}
if (manifest.platforms?.["windows-x86_64-msi"]) {
  fail("latest.json has a windows-x86_64-msi entry, but MSI is not an in-app update package");
}
if (typeof platform.url !== "string" || typeof platform.signature !== "string") {
  fail("latest.json windows-x86_64-nsis entry needs url and signature strings");
}
if (!platform.url.startsWith(`https://github.com/${slug}/releases/download/${tag}/`)) {
  fail(`latest.json URL does not point at ${slug} ${tag}: ${platform.url}`);
}
if (!platform.signature.trim()) fail("latest.json signature is empty");

const installers = readdirSync(nsisDir)
  .filter((name) => name.includes(version) && name.endsWith("-setup.exe"))
  .sort();
if (installers.length !== 1) {
  fail(`expected one versioned NSIS setup in ${nsisDir}, found ${installers.length}`);
}
const installerName = installers[0];
const installerPath = path.join(nsisDir, installerName);
const installer = readFileSync(installerPath);
if (installer.length < 1024) fail(`updater installer is too small: ${installerPath}`);
if (installer[0] !== 0x4d || installer[1] !== 0x5a) fail("updater installer is not a PE file");

const signaturePath = `${installerPath}.sig`;
if (!existsSync(signaturePath)) fail(`missing detached signature: ${signaturePath}`);
const signature = readFileSync(signaturePath, "utf8").trim();
if (!signature) fail(`empty detached signature: ${signaturePath}`);
if (signature !== platform.signature.trim()) {
  fail("latest.json signature does not match the detached .sig file");
}

const expectedAsset = installerName.replace(/ /g, ".");
if (path.basename(platform.url) !== expectedAsset) {
  fail(`latest.json URL ends in "${path.basename(platform.url)}", expected "${expectedAsset}"`);
}

const releaseConfig = JSON.parse(
  readFileSync(path.join(root, "src-tauri", "tauri.release.conf.json"), "utf8"),
);
if (releaseConfig.bundle?.createUpdaterArtifacts !== true) {
  fail("tauri.release.conf.json does not set bundle.createUpdaterArtifacts=true");
}

console.log("UPDATER_ARTIFACTS_OK");
console.log(`  version: ${version}`);
console.log(`  installer: ${installerName} (${installer.length} bytes)`);
console.log(`  signature: ${signaturePath}`);
console.log(`  manifest: ${manifestPath}`);
