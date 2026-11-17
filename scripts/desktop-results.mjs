#!/usr/bin/env node
// Merges WebdriverIO's per-spec JSON reporter files into one results.json.
// The reporter emits wdio-<cid>-<spec>-json-reporter.json, not the configured
// outputFile, so the desktop harness calls this after every run.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function mergeDesktopResults(resultsDir) {
  const reports = fs
    .readdirSync(resultsDir)
    .filter((name) => /^wdio-\d+-\d+-json-reporter\.json$/.test(name))
    .sort();
  if (reports.length === 0) {
    throw new Error(`no WebdriverIO JSON reports found in ${resultsDir}`);
  }

  const suites = [];
  for (const name of reports) {
    const report = JSON.parse(fs.readFileSync(path.join(resultsDir, name), "utf8"));
    suites.push(...(report.suites ?? []));
  }

  const tests = suites.flatMap((suite) => suite.tests ?? []);
  const states = tests.reduce((counts, test) => {
    const state = `${test.state ?? "unknown"}`;
    counts[state] = (counts[state] ?? 0) + 1;
    return counts;
  }, {});
  const revisionPath = path.join(resultsDir, "revision.json");
  const revision = fs.existsSync(revisionPath)
    ? JSON.parse(fs.readFileSync(revisionPath, "utf8"))
    : null;

  const merged = {
    generatedAt: new Date().toISOString(),
    testedAt: revision?.testedAt ?? null,
    head: revision?.head ?? null,
    branch: revision?.branch ?? null,
    dirty: revision?.dirty ?? null,
    reports,
    summary: {
      specs: reports.length,
      suites: suites.length,
      tests: tests.length,
      passed: states.passed ?? 0,
      failed: states.failed ?? 0,
      skipped: states.skipped ?? 0,
      pending: states.pending ?? 0,
    },
    suites,
  };

  fs.writeFileSync(path.join(resultsDir, "results.json"), JSON.stringify(merged, null, 2));
  return merged;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const dir = path.resolve(process.argv[2] ?? path.join(process.cwd(), "test-results", "desktop"));
  const merged = mergeDesktopResults(dir);
  console.log(
    `DESKTOP_RESULTS ${merged.summary.passed}/${merged.summary.tests} passed across ${merged.summary.specs} specs`,
  );
  if (merged.summary.failed > 0 || merged.summary.tests === 0) process.exit(1);
}
