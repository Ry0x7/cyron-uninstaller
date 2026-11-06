import { describe, it, expect } from "vitest";
import {
  CATEGORY_COLOR,
  CATEGORY_LABEL,
  categoryForExtension,
  categoryForFile,
  colorForFile,
  type CategoryKey,
} from "./categories";

describe("file categories", () => {
  it("classifies representative extensions per group", () => {
    const cases: [string, CategoryKey][] = [
      ["rs", "code"],
      ["tsx", "code"],
      ["json", "code"],
      ["gguf", "code"],
      ["png", "image"],
      ["jpg", "image"],
      ["mp4", "video"],
      ["mkv", "video"],
      ["mp3", "audio"],
      ["flac", "audio"],
      ["zip", "archive"],
      ["7z", "archive"],
      ["iso", "archive"],
      ["pdf", "document"],
      ["docx", "document"],
      ["exe", "app"],
      ["dll", "app"],
    ];
    for (const [extension, expected] of cases) {
      expect(categoryForExtension(extension)).toBe(expected);
    }
  });

  it("is case-insensitive and tolerates a leading dot", () => {
    expect(categoryForExtension("PNG")).toBe("image");
    expect(categoryForExtension(".PNG")).toBe("other");
    expect(categoryForFile("Holiday.MOV")).toBe("video");
    expect(categoryForFile("C:\\Users\\Pahente\\Photo.JPEG")).toBe("image");
  });

  it("falls back to other for unknown, missing or dotfile names", () => {
    expect(categoryForExtension("")).toBe("other");
    expect(categoryForExtension("unknownext")).toBe("other");
    expect(categoryForFile("README")).toBe("other");
    expect(categoryForFile(".gitignore")).toBe("other");
    expect(categoryForFile("C:\\folder\\")).toBe("other");
  });

  it("is not confused by prototype property names", () => {
    expect(categoryForExtension("constructor")).toBe("other");
    expect(categoryForExtension("__proto__")).toBe("other");
    expect(categoryForExtension("toString")).toBe("other");
    expect(categoryForFile("__proto__.txt")).toBe("document");
  });

  it("exposes a valid, distinct colour per category", () => {
    const keys = Object.keys(CATEGORY_LABEL) as CategoryKey[];
    expect(keys).toHaveLength(8);
    const colours = new Set<string>();
    for (const key of keys) {
      expect(CATEGORY_LABEL[key].length).toBeGreaterThan(0);
      expect(CATEGORY_COLOR[key]).toMatch(/^#[0-9a-f]{6}$/i);
      colours.add(CATEGORY_COLOR[key].toLowerCase());
    }
    expect(colours.size).toBe(keys.length);
  });

  it("returns the colour matching the file type", () => {
    expect(colorForFile("clip.mp4")).toBe(CATEGORY_COLOR.video);
    expect(colorForFile("notes.md")).toBe(CATEGORY_COLOR.code);
    expect(colorForFile("unknown.xyz")).toBe(CATEGORY_COLOR.other);
  });
});
