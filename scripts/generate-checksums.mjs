import fs from "node:fs";
import crypto from "node:crypto";
import path from "node:path";

const files = [
  "src-tauri/target/release/bundle/nsis/Cyron Uninstaller_0.1.0_x64-setup.exe",
  "src-tauri/target/release/bundle/msi/Cyron Uninstaller_0.1.0_x64_en-US.msi",
  "src-tauri/target/release/cyron.exe"
];

const lines = files.map(file => {
  const buf = fs.readFileSync(file);
  const hash = crypto.createHash("sha256").update(buf).digest("hex");
  return `${hash}  ${path.basename(file)}`;
});

const content = lines.join("\n") + "\n";
fs.writeFileSync("SHA256SUMS.txt", content, "utf8");
console.log(content);
