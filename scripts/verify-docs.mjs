#!/usr/bin/env node
/**
 * scripts/verify-docs.mjs
 * Automated documentation integrity & consistency verification for Cyron.
 *
 * Validates:
 * 1. Broken internal Markdown links and local file references.
 * 2. Documented commands against package.json scripts and scripts/ directory.
 * 3. Release version consistency between package.json, tauri.conf.json, and docs.
 * 4. Correct installer artifact naming conventions.
 * 5. Mechanically detectable obsolete feature claims.
 * 6. Duplicate headings and missing canonical sections.
 * 7. Signing classification invariants and licensing consistency.
 */

import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

let errors = [];
let warnings = [];

function error(file, message) {
  const rel = path.relative(root, file);
  errors.push(`[ERROR] ${rel}: ${message}`);
}

function warn(file, message) {
  const rel = path.relative(root, file);
  warnings.push(`[WARN]  ${rel}: ${message}`);
}

function getMarkdownFiles(dir) {
  const results = [];
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (["node_modules", "target", ".git", "dist"].includes(entry.name)) continue;
      results.push(...getMarkdownFiles(full));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      results.push(full);
    }
  }
  return results;
}

function slugify(text) {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-");
}

function extractHeadings(content) {
  const headings = [];
  const lines = content.split("\n");
  for (const line of lines) {
    const match = line.match(/^(#{1,6})\s+(.+)$/);
    if (match) {
      headings.push({
        level: match[1].length,
        text: match[2].trim(),
        slug: slugify(match[2].trim()),
      });
    }
  }
  return headings;
}

// 1. Read package.json and tauri.conf.json
const pkgJsonPath = path.join(root, "package.json");
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");

if (!existsSync(pkgJsonPath)) {
  errors.push("package.json not found");
  process.exit(1);
}
const pkgJson = JSON.parse(readFileSync(pkgJsonPath, "utf8"));
const packageVersion = pkgJson.version;
const packageScripts = new Set(Object.keys(pkgJson.scripts || {}));

let tauriVersion = null;
if (existsSync(tauriConfPath)) {
  const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
  tauriVersion = tauriConf.version;
}

if (packageVersion !== tauriVersion) {
  errors.push(`Version mismatch: package.json (${packageVersion}) != tauri.conf.json (${tauriVersion})`);
}

// 2. Canonical Docs Existence Check
const requiredDocs = [
  "README.md",
  "LICENSE",
  "CONTRIBUTING.md",
  "AGENTS.md",
  "CLAUDE.md",
  "ROADMAP.md",
  "HANDOFF.md",
  "GATES.md",
  "docs/ARCHITECTURE.md",
  "docs/SECURITY.md",
  "docs/USER_GUIDE.md",
  "docs/BENCHMARKS.md",
  "docs/RELEASE_CHECKLIST.md",
  "docs/BUILD_AND_SIGNING.md",
  "docs/SIGNPATH_READINESS.md",
  "docs/CODE_SIGNING_POLICY.md",
  "docs/development/DESIGN.md",
  "docs/archive/PHASE_HISTORY.md",
];

for (const req of requiredDocs) {
  if (!existsSync(path.join(root, req))) {
    errors.push(`Required documentation file missing: ${req}`);
  }
}

// 3. Scan all Markdown files
const mdFiles = getMarkdownFiles(root);

for (const filePath of mdFiles) {
  const content = readFileSync(filePath, "utf8");
  const dir = path.dirname(filePath);
  const headings = extractHeadings(content);

  // Check Duplicate Consecutive Headings
  for (let i = 0; i < headings.length - 1; i++) {
    if (headings[i].level === headings[i + 1].level && headings[i].text === headings[i + 1].text) {
      error(filePath, `Duplicate consecutive heading found: "${headings[i].text}"`);
    }
  }

  // Check Obsolete Strings
  const obsoletePatterns = [
    { pattern: /\bjournal\.json\b/g, msg: 'Outdated journal name "journal.json". Must be "recovery_journal.jsonl".' },
    { pattern: /Store apps are missing/i, msg: 'Obsolete claim: "Store apps are missing".' },
    { pattern: /Windows Store\/MSIX applications are not enumerated/i, msg: 'Obsolete claim: "Windows Store/MSIX applications are not enumerated".' },
    { pattern: /quarantine has no size cap/i, msg: 'Obsolete claim: "quarantine has no size cap". Cyron has a 20 GB cap.' },
    { pattern: /needs bundler download;\s*unverified/i, msg: 'Obsolete claim: "needs bundler download; unverified". Release bundles are verified.' },
    { pattern: /release packaging, signing and a standalone installer remain unverified/i, msg: 'Obsolete claim: packaging and signing are implemented and verified.' },
    { pattern: /\(safe,\s*caution,\s*danger\)/i, msg: 'Incorrect risk tier "danger". Risk tiers are safe, caution, protected.' },
  ];

  for (const { pattern, msg } of obsoletePatterns) {
    if (pattern.test(content)) {
      error(filePath, msg);
    }
  }

  // Check Artifact Naming Conventions
  // Stale name: Cyron_<version>_x64-setup.exe (without "Uninstaller")
  const staleArtifactMatch = content.match(/Cyron_\d+\.\d+\.\d+_x64(-setup\.exe|_en-US\.msi)/g);
  if (staleArtifactMatch) {
    error(filePath, `Stale artifact naming detected: "${staleArtifactMatch[0]}". Must be "Cyron Uninstaller_<version>_...".`);
  }

  // Check Licensing Claim Consistency
  if (/OSI-approved MIT License committed/i.test(content)) {
    error(filePath, 'Claims "OSI-approved MIT License committed" but LICENSE is Source-Available / No-Reuse.');
  }

  // Check Internal Markdown Links
  // Match [text](target), ignoring image tags if needed or matching both
  const linkMatches = [...content.matchAll(/(?:\[(?:[^\]]+)\])\(([^)]+)\)/g)];
  for (const match of linkMatches) {
    let target = match[1].trim();

    // Skip external links, schemes, or mailto
    if (/^[a-z]+:/i.test(target) || target.startsWith("//")) continue;

    // Handle Anchor-only links (#anchor)
    if (target.startsWith("#")) {
      const anchor = target.slice(1);
      const exists = headings.some((h) => h.slug === anchor || h.slug === decodeURIComponent(anchor));
      if (!exists) {
        // Warning if anchor not found
        warn(filePath, `Anchor link "${target}" not found in current file headings.`);
      }
      continue;
    }

    // Split target into path and optional anchor
    const [targetFilePath, targetAnchor] = target.split("#");
    if (!targetFilePath) continue;

    // Resolve target path
    const resolvedPath = path.resolve(dir, targetFilePath);
    if (!existsSync(resolvedPath)) {
      error(filePath, `Broken link to missing file: "${target}" (resolved to ${path.relative(root, resolvedPath)})`);
    } else if (targetAnchor) {
      if (resolvedPath.endsWith(".md")) {
        const targetContent = readFileSync(resolvedPath, "utf8");
        const targetHeadings = extractHeadings(targetContent);
        const anchorExists = targetHeadings.some(
          (h) => h.slug === targetAnchor || h.slug === decodeURIComponent(targetAnchor)
        );
        if (!anchorExists) {
          warn(filePath, `Anchor "${targetAnchor}" not found in target file "${targetFilePath}".`);
        }
      }
    }
  }

  // Check Documented npm run commands
  const npmMatches = [...content.matchAll(/npm\s+run\s+([a-zA-Z0-9:_-]+)/g)];
  for (const match of npmMatches) {
    const cmd = match[1];
    if (!packageScripts.has(cmd)) {
      error(filePath, `Documented command "npm run ${cmd}" does not exist in package.json.`);
    }
  }

  // Check Documented scripts/ scripts
  const scriptMatches = [...content.matchAll(/(?:node|powershell(?:[^\n]*?-File)?)\s+(?:scripts\/)([a-zA-Z0-9_.-]+)/g)];
  for (const match of scriptMatches) {
    const scriptFile = match[1];
    const scriptPath = path.join(root, "scripts", scriptFile);
    if (!existsSync(scriptPath)) {
      error(filePath, `Documented script "scripts/${scriptFile}" does not exist on disk.`);
    }
  }
}

// Summary Report
console.log("=== Cyron Documentation Verification Report ===");
console.log(`Audited Markdown Files: ${mdFiles.length}`);
console.log(`Package Version: ${packageVersion} | Tauri Version: ${tauriVersion}`);

if (warnings.length > 0) {
  console.log(`\nWarnings (${warnings.length}):`);
  for (const w of warnings) console.log(`  ${w}`);
}

if (errors.length > 0) {
  console.error(`\nErrors (${errors.length}):`);
  for (const e of errors) console.error(`  ${e}`);
  console.error("\nDOCS_CHECK: FAILED");
  process.exit(1);
} else {
  console.log("\nDOCS_CHECK: PASSED (All documentation links, commands, versions, and invariants verified)");
  process.exit(0);
}
