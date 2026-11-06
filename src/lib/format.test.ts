import { describe, it, expect } from "vitest";
import {
  formatBytes,
  formatCount,
  formatDate,
  formatDuration,
  formatPercent,
  basename,
  parentPath,
  truncateMiddle,
  extensionOf,
  plural,
} from "./format";

describe("formatBytes", () => {
  it("handles empty and invalid sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("0 B");
  });

  it("keeps bytes whole and scales by 1024", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(10 * 1024)).toBe("10 KB");
    expect(formatBytes(1024 * 1024)).toBe("1 MB");
    expect(formatBytes(1024 ** 3)).toBe("1 GB");
    expect(formatBytes(1024 ** 4)).toBe("1 TB");
  });

  it("uses sensible precision per magnitude", () => {
    expect(formatBytes(1023)).toBe("1023 B");
    expect(formatBytes(150 * 1024 ** 2)).toBe("150 MB");
    expect(formatBytes(10.5 * 1024 ** 2)).toBe("10.5 MB");
    expect(formatBytes(1.25 * 1024 ** 3)).toBe("1.25 GB");
  });

  it("trims trailing zeros consistently", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1280)).toBe("1.25 KB");
    expect(formatBytes(1.5 * 1024 ** 2)).toBe("1.5 MB");
    expect(formatBytes(2.5 * 1024 ** 3)).toBe("2.5 GB");
    expect(formatBytes(1.1 * 1024)).toBe("1.1 KB");
    expect(formatBytes(2 * 1024 ** 4)).toBe("2 TB");
    expect(formatBytes(100 * 1024 ** 2)).toBe("100 MB");
  });
});

describe("formatCount and formatPercent", () => {
  it("groups large counts", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(999)).toBe("999");
    expect(formatCount(1234)).toBe("1,234");
    expect(formatCount(1234567)).toBe("1,234,567");
    expect(formatCount(Number.NaN)).toBe("0");
  });

  it("clamps ratios to 0..1", () => {
    expect(formatPercent(0)).toBe("0.0%");
    expect(formatPercent(0.01)).toBe("1.0%");
    expect(formatPercent(0.5)).toBe("50%");
    expect(formatPercent(1)).toBe("100%");
    expect(formatPercent(1.4)).toBe("100%");
    expect(formatPercent(-0.2)).toBe("0.0%");
    expect(formatPercent(Number.NaN)).toBe("0%");
  });
});

describe("formatDuration", () => {
  it("formats sub-second, seconds, minutes and hours", () => {
    expect(formatDuration(0)).toBe("0ms");
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(1000)).toBe("1.0s");
    expect(formatDuration(1500)).toBe("1.5s");
    expect(formatDuration(10_000)).toBe("10s");
    expect(formatDuration(60_000)).toBe("1m 0s");
    expect(formatDuration(65_000)).toBe("1m 5s");
    expect(formatDuration(3_661_000)).toBe("1h 1m");
  });

  it("rejects negative and non-finite input", () => {
    expect(formatDuration(-5)).toBe("0s");
    expect(formatDuration(Number.NaN)).toBe("0s");
  });
});

describe("formatDate", () => {
  it("formats registry and ISO calendar dates without a timezone shift", () => {
    expect(formatDate("20240131")).toBe("31 Jan 2024");
    expect(formatDate("2024-01-31")).toBe("31 Jan 2024");
    expect(formatDate("2024-12-01")).toBe("1 Dec 2024");
    expect(formatDate(1_706_760_000)).toContain("2024");
  });

  it("returns an em dash for missing values and passes unknown text through", () => {
    expect(formatDate(null)).toBe("—");
    expect(formatDate(undefined)).toBe("—");
    expect(formatDate("")).toBe("—");
    expect(formatDate("not a date")).toBe("not a date");
    expect(formatDate("20240231")).toBe("20240231");
    expect(formatDate("20241301")).toBe("20241301");
  });
});

describe("path helpers", () => {
  it("finds the basename for Windows and POSIX paths", () => {
    expect(basename("C:\\Users\\Pahente\\file.txt")).toBe("file.txt");
    expect(basename("C:/Users/Pahente/archive.zip")).toBe("archive.zip");
    expect(basename("C:\\Users\\Pahente\\Downloads\\")).toBe("Downloads");
    expect(basename("C:\\")).toBe("C:");
    expect(basename("")).toBe("");
  });

  it("finds the parent folder of a nested path", () => {
    expect(parentPath("C:\\Users\\Pahente\\Downloads\\file.zip")).toBe(
      "C:\\Users\\Pahente\\Downloads",
    );
    expect(parentPath("C:/Users/Pahente/Downloads/file.zip")).toBe(
      "C:/Users/Pahente/Downloads",
    );
    expect(parentPath("C:\\Users")).toBe("C:\\");
    expect(parentPath("C:/Users")).toBe("C:/");
    expect(parentPath("/home")).toBe("/");
    expect(parentPath("file.zip")).toBeNull();
    expect(parentPath("C:\\")).toBeNull();
    expect(parentPath("")).toBeNull();
  });

  it("shortens the middle of long paths to the requested length", () => {
    const path = "C:\\Users\\Pahente\\AppData\\Local\\Programs\\Some\\Very\\Deep\\Folder\\file.bin";
    expect(truncateMiddle("short", 20)).toBe("short");
    const shortened = truncateMiddle(path, 30);
    expect(shortened).toHaveLength(30);
    expect(shortened).toContain("…");
    expect(shortened.startsWith("C:\\")).toBe(true);
    expect(shortened.endsWith("file.bin")).toBe(true);
    expect(truncateMiddle("a long value", 1)).toBe("…");
    expect(truncateMiddle("a long value", 2)).toHaveLength(2);
    expect(truncateMiddle("a long value", 0)).toBe("");
  });

  it("extracts extensions without treating dotfiles as extensions", () => {
    expect(extensionOf("photo.JPG")).toBe("jpg");
    expect(extensionOf("C:\\x\\archive.tar.gz")).toBe("gz");
    expect(extensionOf("README")).toBe("");
    expect(extensionOf(".gitignore")).toBe("");
    expect(extensionOf("trailing.")).toBe("");
    expect(extensionOf("C:\\x\\file")).toBe("");
  });
});

describe("plural", () => {
  it("uses the right form", () => {
    expect(plural(0, "file")).toBe("0 files");
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(2, "entry", "entries")).toBe("2 entries");
    expect(plural(1500, "file")).toBe("1,500 files");
  });
});
