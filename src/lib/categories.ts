import { extensionOf } from "./format";

export type CategoryKey =
  | "code"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "document"
  | "app"
  | "other";

export const CATEGORY_LABEL: Record<CategoryKey, string> = {
  code: "Code & data",
  image: "Images",
  video: "Video",
  audio: "Audio",
  archive: "Archives",
  document: "Documents",
  app: "Executables",
  other: "Other",
};

/** Tile fills live on Cyron's dark treemap canvas, so labels use dark ink. */
export const CATEGORY_COLOR: Record<CategoryKey, string> = {
  code: "#4fb79b",
  image: "#a7be55",
  video: "#d08360",
  audio: "#a291cf",
  archive: "#d3aa5c",
  document: "#6ba1cc",
  app: "#9aa6ac",
  other: "#b3a794",
};

const EXTENSIONS: Record<Exclude<CategoryKey, "other">, string[]> = {
  code: [
    "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "jsonl", "toml", "yaml", "yml",
    "xml", "html", "htm", "css", "scss", "sass", "less", "py", "pyc", "ipynb", "c", "h",
    "cpp", "hpp", "cc", "cs", "java", "kt", "go", "rb", "php", "swift", "lua", "sql",
    "sh", "ps1", "bat", "cmd", "md", "lock", "map", "d", "o", "obj", "pdb", "lib", "wasm",
    "vue", "svelte", "astro", "dart", "ex", "exs", "erl", "zig", "nim", "r", "jl", "vb",
    "csproj", "sln", "gradle", "cmake", "makefile", "gitignore", "env", "db", "sqlite",
    "sqlite3", "parquet", "arrow", "onnx", "gguf", "safetensors", "bin", "dat",
  ],
  image: [
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tif", "tiff", "avif",
    "heic", "heif", "psd", "ai", "xcf", "raw", "cr2", "nef", "dds", "ktx", "ktx2",
  ],
  video: [
    "mp4", "mkv", "mov", "avi", "wmv", "webm", "m4v", "flv", "mpg", "mpeg", "ts", "m2ts",
    "3gp", "ogv", "prproj", "aep",
  ],
  audio: ["mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "wma", "aiff", "alac", "opus", "mid"],
  archive: [
    "zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab", "iso", "img",
    "vhd", "vhdx", "vmdk", "qcow2", "msi", "msix", "appx", "dmg", "deb", "rpm", "pkg",
    "jar", "war", "apk", "aab", "nupkg", "whl", "crate",
  ],
  document: [
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "txt",
    "rtf", "csv", "tsv", "epub", "mobi", "log", "chm",
  ],
  app: [
    "exe", "dll", "sys", "so", "dylib", "com", "scr", "cpl", "ocx", "drv", "msstyles",
    "node", "pyd", "dylib",
  ],
};

const LOOKUP = new Map<string, CategoryKey>();
for (const [category, extensions] of Object.entries(EXTENSIONS) as [
  Exclude<CategoryKey, "other">,
  string[],
][]) {
  for (const extension of extensions) LOOKUP.set(extension, category);
}

export function categoryForExtension(extension: string): CategoryKey {
  return LOOKUP.get(extension.toLowerCase()) ?? "other";
}

export function categoryForFile(name: string): CategoryKey {
  return categoryForExtension(extensionOf(name));
}

export function colorForFile(name: string): string {
  return CATEGORY_COLOR[categoryForFile(name)];
}
