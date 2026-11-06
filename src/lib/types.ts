/** Shared data contract. Field names match the Rust serde camelCase output. */

export type ViewId =
  | "overview"
  | "uninstaller"
  | "analyzer"
  | "cleaner"
  | "activity"
  | "settings";

export type Risk = "safe" | "caution" | "protected";

export type InstallScope = "machine" | "user";

export interface InstalledApp {
  id: string;
  name: string;
  version: string | null;
  publisher: string | null;
  installLocation: string | null;
  installDate: string | null;
  uninstallString: string | null;
  quietUninstallString: string | null;
  displayIcon: string | null;
  /** EstimatedSize from the registry, converted to bytes. */
  estimatedSize: number | null;
  hive: string;
  scope: InstallScope;
  architecture: "x64" | "x86";
  isProtected: boolean;
}

export interface AppInventory {
  apps: InstalledApp[];
  warnings: string[];
}

export interface VolumeInfo {
  mount: string;
  label: string;
  fileSystem: string;
  total: number;
  free: number;
  kind: string;
}

export interface Measured {
  size: number;
  allocated: number;
  fileCount: number;
  dirCount: number;
  elapsedMs: number;
  errors: string[];
}

export type ScanNodeKind = "dir" | "file" | "group";

export interface ScanNode {
  name: string;
  /** Empty for synthetic aggregate nodes. */
  path: string;
  kind: ScanNodeKind;
  size: number;
  allocated: number;
  fileCount: number;
  dirCount: number;
  modified: number | null;
  children: ScanNode[];
}

export interface ExtensionStat {
  extension: string;
  fileCount: number;
  size: number;
}

export interface FileStat {
  name: string;
  path: string;
  size: number;
  allocated: number;
  extension: string;
  modified: number | null;
}

export type ScanMethod = "auto" | "mft" | "directory";

export interface ScanOptions {
  includeHidden: boolean;
  exclusions: string[];
  collectTree: boolean;
  maxDepth: number;
  /** auto tries the NTFS MFT reader first and falls back to the directory walk. */
  method: ScanMethod;
}

export interface StorageCategory {
  key: string;
  label: string;
  bytes: number;
  allocated: number;
  fileCount: number;
  /** True when a known path rule matched, false when inferred from extensions. */
  known: boolean;
  note: string;
}

export interface AgeBucket {
  key: string;
  label: string;
  bytes: number;
  fileCount: number;
}

export interface StorageInsight {
  key: string;
  title: string;
  detail: string;
  bytes: number;
  fileCount: number;
  samplePath: string | null;
}

export interface ScanResult {
  jobId: string;
  rootPath: string;
  root: ScanNode;
  totalSize: number;
  totalAllocated: number;
  fileCount: number;
  dirCount: number;
  dedupedFiles: number;
  skippedReparse: number;
  errors: string[];
  extensionStats: ExtensionStat[];
  largestFiles: FileStat[];
  elapsedMs: number;
  cancelled: boolean;
  truncated: boolean;
  /** "mft" or "directory". */
  method: string;
  /** Why the requested method was not used, when that happened. */
  methodNote: string | null;
  categories: StorageCategory[];
  ageBuckets: AgeBucket[];
  insights: StorageInsight[];
}

export interface ScanProgress {
  jobId: string;
  files: number;
  dirs: number;
  bytes: number;
  currentDir: string;
}

export interface CacheTarget {
  id: string;
  tool: string;
  label: string;
  path: string;
  exists: boolean;
  size: number;
  allocated: number;
  fileCount: number;
  dirCount: number;
  risk: Risk;
  /** ai, ide, packages, containers or windows. */
  category: string;
  note: string;
  lastModified: number | null;
}

export interface CacheInventory {
  targets: CacheTarget[];
  totalSize: number;
  totalSafe: number;
  totalCaution: number;
  totalProtected: number;
  scannedAt: number;
  warnings: string[];
}

export interface HistoryEntry {
  id: string;
  kind: string;
  title: string;
  detail: string;
  at: number;
  bytes: number;
  items: number;
  status: "completed" | "cancelled" | "failed";
}

export const SCAN_EVENT = "cyron-scan-progress";
export const DUPLICATE_EVENT = "cyron-duplicate-progress";

export interface DuplicateOptions {
  /** Files smaller than this are ignored; the backend enforces a floor. */
  minSize: number;
  includeHidden: boolean;
  exclusions: string[];
}

export interface DuplicateFile {
  path: string;
  size: number;
  allocated: number;
  modified: number | null;
  /** True when this path is an additional hard link to an already-listed file. */
  hardLink: boolean;
}

export interface DuplicateGroup {
  id: string;
  size: number;
  /** Recoverable bytes if all but one copy were removed. */
  wasted: number;
  files: DuplicateFile[];
}

export interface DuplicateReport {
  jobId: string;
  rootPath: string;
  groups: DuplicateGroup[];
  totalGroups: number;
  totalFiles: number;
  totalWasted: number;
  candidateFiles: number;
  hashedFiles: number;
  scannedFiles: number;
  errors: string[];
  elapsedMs: number;
  cancelled: boolean;
  truncated: boolean;
}

export interface DuplicateProgress {
  jobId: string;
  /** "index" | "partial" | "full". */
  phase: string;
  files: number;
  bytesHashed: number;
  currentPath: string;
}
