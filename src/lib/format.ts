const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** Human readable file size using binary units, e.g. 1.4 GB. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;
  const fixed = value.toFixed(digits);
  const trimmed = fixed.includes(".")
    ? fixed.replace(/0+$/, "").replace(/\.$/, "")
    : fixed;
  return `${trimmed} ${BYTE_UNITS[unit]}`;
}

export function formatBytesFull(bytes: number): string {
  return `${formatBytes(bytes)} (${formatCount(bytes)} bytes)`;
}

export function formatCount(value: number): string {
  if (!Number.isFinite(value)) return "0";
  return new Intl.NumberFormat("en-US").format(Math.round(value));
}

/** Ratio from 0 to 1 rendered as a rounded percentage. */
export function formatPercent(ratio: number): string {
  if (!Number.isFinite(ratio)) return "0%";
  const clamped = Math.min(Math.max(ratio, 0), 1);
  const percent = clamped * 100;
  return `${percent >= 10 ? Math.round(percent) : percent.toFixed(1)}%`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "0s";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds >= 10 ? 0 : 1)}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  if (minutes < 60) return `${minutes}m ${rest}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

/**
 * Registry dates (yyyyMMdd) and date-only ISO strings are calendar dates, not
 * instants. Parsing them as UTC can show the previous day in negative offsets.
 */
function parseCalendarDate(value: string): Date | null {
  const match = /^(\d{4})-?(\d{2})-?(\d{2})$/.exec(value);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const date = new Date(year, month - 1, day);
  if (
    Number.isNaN(date.getTime()) ||
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null;
  }
  return date;
}

/** Unix seconds to a short date. Accepts ISO dates and yyyy-mm-dd for metadata. */
export function formatDate(value: number | string | null | undefined): string {
  if (value === null || value === undefined || value === "") return "—";
  let date: Date;
  if (typeof value === "number") {
    date = new Date(value * 1000);
  } else {
    date = parseCalendarDate(value) ?? new Date(value);
  }
  if (Number.isNaN(date.getTime())) return typeof value === "string" ? value : "—";
  return date.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
}

export function formatDateTime(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  const date = new Date(value * 1000);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function relativeTime(value: number | null | undefined): string {
  if (value === null || value === undefined) return "Never";
  const diff = Date.now() / 1000 - value;
  if (diff < 45) return "Just now";
  if (diff < 3600) return `${Math.round(diff / 60)} min ago`;
  if (diff < 86400) return `${Math.round(diff / 3600)} h ago`;
  if (diff < 86400 * 7) return `${Math.round(diff / 86400)} d ago`;
  return formatDate(value);
}

export function basename(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function parentPath(path: string): string | null {
  const cleaned = path.replace(/[\\/]+$/, "");
  if (!cleaned) return null;
  const index = Math.max(cleaned.lastIndexOf("\\"), cleaned.lastIndexOf("/"));
  if (index < 0) return null;
  // A child of a drive root has the root, not "C:", as its parent.
  if (index === 2 && /^[A-Za-z]:$/.test(cleaned.slice(0, 2))) {
    return `${cleaned.slice(0, 2)}${cleaned[2]}`;
  }
  if (index === 0) return cleaned.slice(0, 1);
  return cleaned.slice(0, index);
}

/** Shortens long paths from the middle so the tail stays readable. */
export function truncateMiddle(value: string, max = 56): string {
  if (max <= 0) return "";
  if (value.length <= max) return value;
  if (max === 1) return "…";
  const head = Math.max(1, Math.ceil((max - 1) * 0.42));
  const tail = Math.max(0, max - 1 - head);
  return `${value.slice(0, head)}…${tail > 0 ? value.slice(-tail) : ""}`;
}

export function plural(count: number, singular: string, pluralForm?: string): string {
  return `${formatCount(count)} ${count === 1 ? singular : (pluralForm ?? `${singular}s`)}`;
}

export function extensionOf(name: string): string {
  const base = basename(name).toLowerCase();
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) return "";
  return base.slice(dot + 1);
}
