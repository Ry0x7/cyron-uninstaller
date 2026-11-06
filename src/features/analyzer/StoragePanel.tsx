import { useMemo, useState } from "react";
import {
  Badge,
  Button,
  EmptyState,
  Eyebrow,
  Panel,
  ProgressBar,
  Stat,
} from "../../components/ui";
import { IconAlert, IconExternal, IconFolder, IconInfo } from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatCount, formatPercent } from "../../lib/format";
import type { ScanResult, StorageCategory } from "../../lib/types";
import "./storage.css";

const CATEGORY_ORDER = [
  "applications",
  "games",
  "downloads",
  "documents",
  "media",
  "system",
  "developer",
  "ai",
  "caches",
  "other",
];

const CATEGORY_CLASS: Record<string, string> = {
  applications: "apps",
  games: "games",
  downloads: "downloads",
  documents: "documents",
  media: "media",
  system: "system",
  developer: "developer",
  ai: "ai",
  caches: "caches",
  other: "other",
};

function categoryClass(key: string): string {
  return CATEGORY_CLASS[key] ?? "other";
}

function orderedCategories(categories: StorageCategory[]): StorageCategory[] {
  const rank = (key: string) => {
    const index = CATEGORY_ORDER.indexOf(key);
    return index === -1 ? CATEGORY_ORDER.length : index;
  };
  return [...categories].sort((left, right) => {
    if (right.bytes !== left.bytes) return right.bytes - left.bytes;
    return rank(left.key) - rank(right.key);
  });
}

function methodLabel(method: string): string {
  if (method === "mft") return "NTFS MFT";
  if (method === "directory") return "Directory walk";
  return method || "Unknown";
}

function methodDescription(method: string): string {
  if (method === "mft") {
    return "Sizes were read from the NTFS Master File Table. It requires administrator rights and reads metadata only — no file contents and no changes.";
  }
  if (method === "directory") {
    return "Folders were walked with the Windows directory API. This is the fallback used when the Master File Table cannot be read.";
  }
  return "The scanner did not report which method produced this scan.";
}

function share(bytes: number, total: number): number {
  if (!Number.isFinite(bytes) || bytes <= 0 || !Number.isFinite(total) || total <= 0) return 0;
  return Math.min(bytes / total, 1);
}

export function StoragePanel({ result }: { result: ScanResult }) {
  const [revealError, setRevealError] = useState<string | null>(null);

  const categories = useMemo(() => orderedCategories(result.categories ?? []), [result.categories]);
  const total = result.totalSize;
  const categoriesWithData = categories.filter((category) => category.bytes > 0 || category.fileCount > 0);
  const insights = (result.insights ?? []).filter((insight) => insight.bytes > 0 || insight.fileCount > 0);
  const ages = result.ageBuckets ?? [];
  const ageTotal = ages.reduce((sum, bucket) => sum + bucket.bytes, 0);
  const partial = result.cancelled || result.errors.length > 0;
  const categoryChartDescription = categoriesWithData
    .map((category) => {
      const confidence = category.known ? "known path rule" : "inferred";
      return `${category.label}: ${formatBytes(category.bytes)}, ${formatPercent(
        share(category.bytes, total),
      )}, ${confidence}`;
    })
    .join("; ");

  const reveal = async (path: string) => {
    setRevealError(null);
    try {
      await api.revealPath(path);
    } catch (error) {
      setRevealError(errorMessage(error));
    }
  };

  return (
    <div className="sto-panel">
      <div className="sto-summary">
        <div className="sto-summary__stats">
          <Stat label="Logical size" value={formatBytes(result.totalSize)} size="lg" />
          <Stat label="Allocated" value={formatBytes(result.totalAllocated)} />
          <Stat label="Files" value={formatCount(result.fileCount)} />
          <Stat
            label="Scan method"
            value={<span className="sto-method">{methodLabel(result.method)}</span>}
            meta={result.elapsedMs > 0 ? `${formatCount(result.elapsedMs)} ms` : undefined}
          />
        </div>
        <p className="sto-summary__line">
          Categories and ages are measured from this scan of{" "}
          <span className="sto-mono" title={result.rootPath}>
            {result.rootPath}
          </span>
          . A <Badge tone="accent">Known rule</Badge> label means a recognised path rule matched;{" "}
          <Badge>Inferred</Badge> means the category was inferred from file extensions. This is
          storage intelligence, not a cleaning recommendation.
        </p>
      </div>

      {(result.methodNote || partial) && (
        <div className={`sto-callout${partial ? " sto-callout--error" : ""}`} role={partial ? "alert" : "status"}>
          <IconInfo size={14} aria-hidden="true" />
          <div>
            <strong>{partial ? (result.cancelled ? "Partial scan results" : "Some locations could not be scanned") : "Scan method note"}</strong>
            <p>
              {result.cancelled
                ? "Scanning was cancelled. The figures below cover only the files reached before cancellation."
                : result.errors.length > 0
                  ? `${formatCount(result.errors.length)} scan warning${result.errors.length === 1 ? "" : "s"} were reported. These figures cover only accessible locations; review scan diagnostics before treating them as complete.`
                  : result.methodNote}
            </p>
          </div>
        </div>
      )}

      {revealError && (
        <div className="sto-callout sto-callout--error" role="alert">
          <IconAlert size={14} aria-hidden="true" />
          <p>{revealError}</p>
        </div>
      )}

      <Panel
        title="What uses the space"
        subtitle="Storage is grouped by path and file-type clues. Known path rules take precedence over extension inference; these categories do not indicate whether anything is safe to remove."
      >
        {categoriesWithData.length === 0 ? (
          <EmptyState
            compact
            icon={IconFolder}
            title="No category data"
            description="The scan returned no categorised files, which usually means the folder was empty or unreadable."
          />
        ) : (
          <>
            <div
              className="sto-composition"
              role="img"
              aria-label={`Category composition of ${formatBytes(total)}: ${categoryChartDescription || "no categorized files"}`} 
            >
              {categoriesWithData.map((category) => (
                <span
                  key={category.key}
                  className={`sto-composition__slice sto-fill--${categoryClass(category.key)}`}
                  style={{ width: `${share(category.bytes, total) * 100}%` }}
                  title={`${category.label}: ${formatBytes(category.bytes)} (${formatPercent(
                    share(category.bytes, total),
                  )})`}
                />
              ))}
            </div>

            <ul className="sto-category-list">
              {categoriesWithData.map((category) => {
                const ratio = share(category.bytes, total);
                return (
                  <li className="sto-category" key={category.key}>
                    <div className="sto-category__head">
                      <span className={`sto-swatch sto-fill--${categoryClass(category.key)}`} aria-hidden="true" />
                      <span className="sto-category__name">{category.label}</span>
                      <Badge tone={category.known ? "accent" : "neutral"}>
                        {category.known ? "Known rule" : "Inferred"}
                      </Badge>
                      <span className="sto-category__size">{formatBytes(category.bytes)}</span>
                      <span className="sto-category__share">{formatPercent(ratio)}</span>
                    </div>
                    <div className="sto-category__meter">
                      <ProgressBar value={ratio} label={`${category.label} share of scan`} />
                    </div>
                    <div className="sto-category__meta">
                      <span>{formatCount(category.fileCount)} files</span>
                      <span>Allocated {formatBytes(category.allocated)}</span>
                    </div>
                    {category.note && <p className="sto-category__note">{category.note}</p>}
                  </li>
                );
              })}
            </ul>

            {categoriesWithData.length < categories.length && (
              <p className="sto-footnote">
                {formatCount(categories.length - categoriesWithData.length)} categories had no files in this
                scan and are omitted.
              </p>
            )}
          </>
        )}
      </Panel>

      <Panel
        title="Age of the data"
        subtitle="Based on file modification time. A recent date does not mean a file is unimportant, and an old file is not automatically disposable."
      >
        {ages.length === 0 || ageTotal === 0 ? (
          <p className="sto-footnote">No modification times were available for this scan.</p>
        ) : (
          <div className="sto-age">
            <div className="sto-age__bar" role="img" aria-label="Storage by file age">
              {ages.map((bucket) => (
                <span
                  key={bucket.key}
                  className={`sto-age__slice sto-age--${bucket.key}`}
                  style={{ width: `${share(bucket.bytes, ageTotal) * 100}%` }}
                  title={`${bucket.label}: ${formatBytes(bucket.bytes)}`}
                />
              ))}
            </div>
            <ul className="sto-age__legend">
              {ages.map((bucket) => (
                <li key={bucket.key}>
                  <span className={`sto-swatch sto-age--${bucket.key}`} aria-hidden="true" />
                  <span className="sto-age__label">{bucket.label}</span>
                  <span className="sto-age__value">{formatBytes(bucket.bytes)}</span>
                  <span className="sto-age__count">{formatCount(bucket.fileCount)} files</span>
                  <span className="sto-age__share">{formatPercent(share(bucket.bytes, ageTotal))}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </Panel>

      <Panel
        title="Notable storage"
        subtitle="Measured landmarks in this scan. These are observations, not cleanup targets."
      >
        {insights.length === 0 ? (
          <p className="sto-footnote">
            No notable patterns met the reporting thresholds in this scan. This does not mean that the
            scanned files are unimportant or safe to remove.
          </p>
        ) : (
          <ul className="sto-insights">
            {insights.map((insight) => (
              <li className="sto-insight" key={insight.key}>
                <div className="sto-insight__head">
                  <Eyebrow>{insight.title}</Eyebrow>
                  <span className="sto-insight__size">{formatBytes(insight.bytes)}</span>
                  <span className="sto-insight__count">{formatCount(insight.fileCount)} files</span>
                </div>
                <p className="sto-insight__detail">{insight.detail}</p>
                {insight.samplePath && (
                  <div className="sto-insight__path">
                    <span className="sto-mono" title={insight.samplePath}>
                      {insight.samplePath}
                    </span>
                    <Button
                      size="sm"
                      variant="ghost"
                      icon={<IconExternal size={12} />}
                      onClick={() => reveal(insight.samplePath as string)}
                      aria-label={`Open sample location for ${insight.title}`}
                    >
                      Show
                    </Button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title="Scan diagnostics" className="sto-diagnostics">
        <div className="sto-diagnostics__method">
          <span className="sto-diagnostics__label">Method</span>
          <Badge tone={result.method === "mft" ? "safe" : "neutral"}>{methodLabel(result.method)}</Badge>
          <span className="sto-diagnostics__text">{methodDescription(result.method)}</span>
        </div>
        <ul className="sto-diagnostics__limits">
          <li>
            <strong>Pruned tree.</strong> The visual tree keeps at most 200 children per folder and 8 levels.
            Aggregated totals cover the scan scope, but cancellation and inaccessible locations can make that
            scope incomplete.
          </li>
          <li>
            <strong>Inferred categories.</strong> Extension rules cannot know intent. A large <code>.bin</code>{" "}
            file may be a model, a VM disk or a build artifact.
          </li>
          <li>
            <strong>Logical vs allocated.</strong> Logical size is what the files report; allocated is what the
            volume actually reserved. They differ with compression, sparse files and hard links.
          </li>
          <li>
            <strong>Read-only.</strong> No file was opened for writing, moved or deleted to produce this report.
          </li>
        </ul>
        {result.truncated && (
          <p className="sto-footnote">
            Some detail was capped to keep the result bounded. Totals cover the scan scope; they may be partial
            if the scan was cancelled or reported inaccessible locations.
          </p>
        )}
      </Panel>
    </div>
  );
}
