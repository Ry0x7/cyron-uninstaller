import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";

const root = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const version = pkg.version;

const nsisDir = path.join(root, "src-tauri", "target", "release", "bundle", "nsis");
const msiDir = path.join(root, "src-tauri", "target", "release", "bundle", "msi");

function findBundle(dir, prefix, ext) {
  if (!fs.existsSync(dir)) return null;
  const files = fs.readdirSync(dir);
  const match = files.find(f => f.startsWith(prefix) && f.includes(version) && f.endsWith(ext));
  return match ? path.join(dir, match) : null;
}

const nsisPath = findBundle(nsisDir, "Cyron", ".exe") || path.join(nsisDir, `Cyron Uninstaller_${version}_x64-setup.exe`);
const msiPath = findBundle(msiDir, "Cyron", ".msi") || path.join(msiDir, `Cyron Uninstaller_${version}_x64_en-US.msi`);

function getSha256(filePath) {
  const buffer = fs.readFileSync(filePath);
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

let allOk = true;

console.log("--- Verifying Release Bundles ---");

for (const [name, targetPath] of [["NSIS Setup", nsisPath], ["WiX MSI Package", msiPath]]) {
  if (!fs.existsSync(targetPath)) {
    console.error(`FAIL: ${name} not found at ${targetPath}`);
    allOk = false;
    continue;
  }
  const stat = fs.statSync(targetPath);
  const sizeMb = (stat.size / (1024 * 1024)).toFixed(2);
  const sha = getSha256(targetPath);
  console.log(`OK: ${name}`);
  console.log(`    Path: ${targetPath}`);
  console.log(`    Size: ${sizeMb} MiB (${stat.size} bytes)`);
  console.log(`    SHA-256: ${sha}`);
}

if (!allOk) {
  process.exit(1);
}

console.log("RELEASE_BUNDLES_OK");
