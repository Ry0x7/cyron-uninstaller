#!/usr/bin/env node
/**
 * Build the static Tauri updater manifest for a GitHub release.
 *
 * `npm run build:release` signs the NSIS setup executable as
 * `*-setup.exe.sig` when TAURI_SIGNING_PRIVATE_KEY is set. This script embeds
 * that signature and the matching GitHub release download URL into
 * `latest.json`. The per-user NSIS setup is the preferred update package.
 *
 *   node scripts/generate-updater-manifest.mjs --tag v0.2.2
 *   node scripts/generate-updater-manifest.mjs --tag v0.2.2 --output path/to/latest.json
 */
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);

function argument(name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : null;
}

function fail(message) {
  console.error("UPDATER_MANIFEST_FAILED");
  console.error(` - ${message}`);
  process.exit(1);
}

const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
const version = pkg.version;
const tag = argument("--tag") ?? `v${version}`;
if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) {
  fail(`"${tag}" is not a release tag such as v0.2.2`);
}
if (tag.replace(/^v/, "") !== version) {
  fail(`tag "${tag}" does not match package.json version "${version}"`);
}

const repository =
  argument("--repo") ??
  (typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url ?? "");
const match = repository.match(/github\.com[/:]([^/]+)\/([^/]+?)(?:\.git)?\/?$/);
if (!match) fail(`cannot derive a GitHub repository from "${repository}"`);
const slug = `${match[1]}/${match[2]}`;

const bundleDir = path.join(root, "src-tauri", "target", "release", "bundle");
const nsisDir = path.join(bundleDir, "nsis");
const outputPath = path.resolve(
  root,
  argument("--output") ?? path.join(bundleDir, "latest.json"),
);

if (!existsSync(nsisDir)) fail(`NSIS bundle directory not found: ${nsisDir}`);
const installers = readdirSync(nsisDir)
  .filter((name) => name.includes(version) && name.endsWith("-setup.exe"))
  .sort();
if (installers.length !== 1) {
  fail(`expected one versioned NSIS setup in ${nsisDir}, found ${installers.length}`);
}

const installerName = installers[0];
const installerPath = path.join(nsisDir, installerName);
const signaturePath = `${installerPath}.sig`;
if (!existsSync(signaturePath)) {
  fail(`missing updater signature: ${signaturePath}. Build with TAURI_SIGNING_PRIVATE_KEY set.`);
}

const signature = readFileSync(signaturePath, "utf8").trim();
if (!signature) fail(`updater signature is empty: ${signaturePath}`);

const notesPath = path.join(root, "docs", `RELEASE_NOTES_${tag}.md`);
const notes = existsSync(notesPath) ? readFileSync(notesPath, "utf8").trim() : "";
const assetName = installerName.replace(/ /g, ".");
const url = `https://github.com/${slug}/releases/download/${tag}/${assetName}`;

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    // Scope the feed to NSIS. A generic windows-x86_64 entry would also be
    // selected by an MSI install, which must not be replaced by the per-user
    // NSIS package.
    "windows-x86_64-nsis": {
      signature,
      url,
    },
  },
};

writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`UPDATER_MANIFEST_OK ${outputPath}`);
console.log(`  version: ${version}`);
console.log(`  installer: ${assetName}`);
console.log(`  url: ${url}`);
