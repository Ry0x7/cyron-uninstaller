/** Shared data contract. Field names match the Rust serde camelCase output. */

export type ViewId =
  | "overview"
  | "search"
  | "uninstaller"
  | "analyzer"
  | "performance"
  | "cleaner"
  | "activity"
  | "settings";

export type Risk = "safe" | "caution" | "protected";

export type InstallScope = "machine" | "user";

export type AppSource = "win32" | "store" | "steam" | "portable";

export type UninstallSupport =
  | "standard"
  | "silent_only"
  | "store_managed"
  | "manual_only"
  | "missing";

export type DetectionConfidence = "confirmed" | "heuristic";

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
  architecture: "x64" | "x86" | "neutral" | string;
  isProtected: boolean;
  source: AppSource;
  uninstallSupport: UninstallSupport;
  confidence: DetectionConfidence;
  detectionReason?: string | null;
  registryKeyName?: string | null;
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
  /** detected, not-detected or unknown. Only not-detected is shown as orphaned. */
  installState?: "detected" | "not-detected" | "unknown";
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

/* ------------------------------------------------------- developer storage */

export interface DeveloperArtifact {
  id: string;
  projectId: string;
  projectName: string;
  path: string;
  kind: string;
  label: string;
  risk: Risk;
  confidence: string;
  evidence: string;
  /** What must be run to reproduce this artifact. */
  rebuild: string;
  /** What is lost or affected while the artifact is in quarantine. */
  loss: string;
  size: number;
  allocated: number;
  fileCount: number;
  dirCount: number;
  lastModified: number | null;
  activity: "active" | "recent" | "old" | "unknown";
  stale: boolean;
  skippedLinks: number;
  truncated: boolean;
}

export interface ProtectedComponent {
  path: string;
  kind: string;
  reason: string;
  size: number;
}

export interface DeveloperProject {
  id: string;
  name: string;
  path: string;
  kind: string;
  markers: string[];
  frameworks: string[];
  modified: number | null;
  activity: "active" | "recent" | "old" | "unknown";
  artifacts: DeveloperArtifact[];
  protected: ProtectedComponent[];
  sourceProtected: boolean;
}

export interface DeveloperTotals {
  projectCount: number;
  artifactCount: number;
  artifactBytes: number;
  artifactAllocated: number;
  protectedBytes: number;
  activeProjects: number;
  oldProjects: number;
}

export interface ProjectRoot {
  path: string;
  source: "selected" | "automatic";
  exists: boolean;
}

export interface DeveloperStorageScan {
  scanId: string;
  scannedAt: number;
  roots: ProjectRoot[];
  projects: DeveloperProject[];
  totals: DeveloperTotals;
  warnings: string[];
}

export interface ArtifactVerification {
  id: string;
  path: string;
  label: string;
  status: "removed" | "remaining" | "changed";
  expectedBytes: number;
  remainingBytes: number;
  remainingFiles: number;
}

export interface DeveloperCleanupVerification {
  scanId: string;
  artifacts: ArtifactVerification[];
  remainingBytes: number;
  removedArtifacts: number;
  remainingArtifacts: number;
  restorableRunIds: string[];
  errors: string[];
}

export interface HistoryEntry {
  id: string;
  kind: string;
  title: string;
  detail: string;
  at: number;
  bytes: number;
  items: number;
  status: "completed" | "partial" | "cancelled" | "failed";
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

/* ---------------------------------------------------------------- cleanup */

/** A single catalog location as the cleanup planner reports it. */
export interface CleanupTarget {
  id: string;
  tool: string;
  label: string;
  path: string;
  note: string;
  /** Plain-language consequence of moving these files. */
  impact: string;
  risk: Risk;
  /** ai, ide, packages, containers or windows. */
  category: string;
  exists: boolean;
  size: number;
  fileCount: number;
  dirCount: number;
  /** False for protected data, which the backend refuses to plan. */
  includable: boolean;
}

export interface CleanupPreviewFile {
  lastModified: number | null;
  path: string;
  size: number;
  targetId: string;
}

export interface CleanupPreviewSkip {
  path: string;
  reason: string;
}

/** Result of a dry run. The plan id is the only thing execution accepts. */
export interface CleanupPreview {
  planId: string;
  generatedAt: number;
  /** Unix seconds; an expired plan cannot be executed. */
  expiresAt: number;
  targets: CleanupTarget[];
  files: CleanupPreviewFile[];
  skipped: CleanupPreviewSkip[];
  /** Exact count and byte total over every planned file, not just the listed ones. */
  fileCount: number;
  bytes: number;
  totalFiles: number;
  /** True while the backend read-only switch is on. */
  readOnly: boolean;
  /** Files omitted from `files` because of the display cap. */
  hiddenCount: number;
  warnings: string[];
  truncated: boolean;
}

export interface CleanupItemResult {
  targetId: string;
  path: string;
  bytes: number;
  status: "quarantined" | "skipped" | "failed";
  reason: string | null;
  quarantinePath: string | null;
}

export interface CleanupReport {
  runId: string;
  planId: string;
  bytes: number;
  spaceMoved: number;
  spaceFreed: number;
  files: number;
  dirsRemoved: number;
  quarantined: number;
  skipped: number;
  failed: number;
  items: CleanupItemResult[];
  errors: string[];
  elapsedMs: number;
  cancelled: boolean;
  quarantineDir: string | null;
  /** Unix seconds when the quarantine batch becomes purgeable. */
  purgeAt: number;
}

export interface CleanupProgress {
  runId: string;
  files: number;
  totalFiles: number;
  bytes: number;
  currentPath: string;
}

export interface QuarantineBatch {
  runId: string;
  createdAt: number;
  expiresAt: number;
  bytes: number;
  fileCount: number;
  dir: string;
  targetIds: string[];
}

export interface RestoreReport {
  runId: string;
  restored: number;
  skipped: number;
  failed: number;
  errors: string[];
}

export interface PurgeReport {
  runId: string;
  removedFiles: number;
  freedBytes: number;
}

export interface CleanupLogEntry {
  id: string;
  at: number;
  title: string;
  targets: string[];
  bytes: number;
  files: number;
  quarantined: number;
  skipped: number;
  failed: number;
  status: "completed" | "partial" | "cancelled" | "failed" | "blocked";
  detail: string;
}

export const CLEANUP_EVENT = "cyron-cleanup-progress";

/* ----------------------------------------------------------------- phase 4 */

export interface AppProcess {
  pid: number;
  name: string;
  path: string | null;
  isCritical: boolean;
  title: string | null;
  creationTime: number;
}

export interface ProcessTerminateResult {
  pid: number;
  name: string;
  success: boolean;
  graceful: boolean;
  error: string | null;
}

export interface ParsedUninstallCommand {
  executable: string;
  arguments: string[];
  isMsi: boolean;
  isValid: boolean;
  rejectionReason: string | null;
}

export interface UninstallPlan {
  planId: string;
  appId: string;
  appName: string;
  executable: string;
  arguments: string[];
  isMsi: boolean;
  expiresAt: number;
}

export interface UninstallExecutionResult {
  appId: string;
  exitCode: number | null;
  status: "completed" | "remaining" | "restart_required" | "failed" | "cancelled";
  restartRequired: boolean;
  message: string;
  elapsedMs: number;
}

export interface StoreUninstallPlan {
  planId: string;
  appId: string;
  appName: string;
  packageFullName: string;
  packageFamilyName: string;
  publisher: string | null;
  publisherId: string | null;
  installLocation: string | null;
  expiresAt: number;
}

export interface StoreUninstallResult {
  appId: string;
  status: string;
  packageFullName: string;
  message: string;
  elapsedMs: number;
}

export interface AppHistoryRecord {
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  source: string;
  installLocation: string | null;
  installDate: string | null;
  firstSeenAt: number;
  lastSeenAt: number;
  removedAt: number | null;
  removalMethod: string | null;
}

export interface LeftoverCandidate {
  id: string;
  kind: "directory" | "file" | "registry_key";
  path: string;
  confidence: "high" | "medium" | "low";
  risk: "safe" | "caution" | "protected";
  reason: string;
  size: number;
  fileCount: number;
  selected: boolean;
}

export interface LeftoverScanReport {
  planId: string;
  appId: string;
  appName: string;
  candidates: LeftoverCandidate[];
  totalSize: number;
  totalFiles: number;
  registryKeysCount: number;
  expiresAt: number;
}

export interface LeftoverCleanReport {
  runId: string;
  appId: string;
  quarantinedFiles: number;
  quarantinedBytes: number;
  deletedRegistryKeys: number;
  registryBackupPath: string | null;
  quarantineDir: string | null;
  errors: string[];
}

export interface BatchUninstallItemPlan {
  appId: string;
  appName: string;
  rawCommand: string;
  executable: string;
  arguments: string[];
  isMsi: boolean;
  isValid: boolean;
  rejectionReason: string | null;
}

export interface BatchUninstallPlan {
  planId: string;
  items: BatchUninstallItemPlan[];
  expiresAt: number;
}

export interface BatchUninstallItemResult {
  appId: string;
  name: string;
  status: string;
  exitCode: number | null;
  message: string;
}

export interface BatchUninstallReport {
  total: number;
  completed: number;
  failed: number;
  cancelled: number;
  remaining: number;
  restartRequired: number;
  results: BatchUninstallItemResult[];
}

export interface StartupRoute {
  view: ViewId;
  path?: string | null;
  subview?: string | null;
  targetAppId?: string | null;
  action?: string | null;
  note?: string | null;
}

export interface ExplorerIntegrationStatus {
  isRegistered: boolean;
  standardActionsRegistered: boolean;
  advancedActionsRegistered: boolean;
  folderActionsRegistered: boolean;
  exeActionsRegistered: boolean;
  registeredExePath: string | null;
  currentExePath: string;
  needsRepair: boolean;
  details: string[];
}

export interface LockingProcessInfo {
  pid: number;
  name: string;
  path?: string | null;
  windowTitle?: string | null;
  isCritical: boolean;
  creationTime: number;
}

export interface FileTargetInspection {
  path: string;
  canonicalPath: string;
  isDirectory: boolean;
  exists: boolean;
  sizeBytes: number;
  fileCount: number;
  dirCount: number;
  isReadonly: boolean;
  isHidden: boolean;
  isSystem: boolean;
  isReparsePoint: boolean;
  owner?: string | null;
  reasonsForFailure: string[];
  sampleItems: string[];
}

export interface ForceDeletePlanPreview {
  planId: string;
  inspection: FileTargetInspection;
  lockingProcesses: LockingProcessInfo[];
  expiresAt: number;
}

export interface ForceDeleteResult {
  success: boolean;
  mode: "quarantine" | "permanent";
  deletedFiles: number;
  deletedDirs: number;
  freedBytes: number;
  quarantineBatchId?: string | null;
  message: string;
  warnings: string[];
}

export interface KillProcessPlanPreview {
  planId: string;
  exePath: string;
  matchingProcesses: LockingProcessInfo[];
  expiresAt: number;
}

export interface CleanableInspection {
  path: string;
  isRecognized: boolean;
  providerId: string | null;
  toolLabel: string | null;
  risk: Risk | null;
  category: string | null;
  explanation: string;
  isCleanable: boolean;
  expectedReclaimableBytes: number;
  totalOccupiedBytes: number;
}

export interface CleanupProviderDef {
  schemaVersion: number;
  id: string;
  tool: string;
  label: string;
  pathTemplate: string;
  risk: Risk;
  category: string;
  impact: string;
  patterns: string[];
}

export interface DirectoryQuery {
  jobId: string;
  dirPath: string;
  page: number;
  pageSize: number;
  sortField?: "size" | "name" | "modified" | "files";
  sortDir?: "asc" | "desc";
  search?: string;
  minSize?: number;
  extension?: string;
}

export interface DirectoryPage {
  entries: ScanNode[];
  totalEntries: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

export interface SearchQuery {
  jobId: string;
  query: string;
  extension?: string;
  minSize?: number;
  maxSize?: number;
  limit: number;
  offset: number;
  sortField?: string;
  sortDir?: "asc" | "desc";
}

export interface SearchResultPage {
  items: FileStat[];
  totalMatches: number;
  limit: number;
  offset: number;
}

export interface IndexStats {
  jobId: string;
  totalFiles: number;
  totalDirs: number;
  totalSize: number;
  totalAllocated: number;
  isComplete: boolean;
}

export interface HunterTargetInfo {
  hwnd: number;
  pid: number;
  processName: string;
  processPath: string | null;
  windowTitle: string | null;
  matchedAppId: string | null;
  matchedAppName: string | null;
  isCritical: boolean;
  hunterTargetId: string;
  canTerminate: boolean;
  creationTime: number;
}

export interface BrokenEntryPlan {
  planId: string;
  appId: string;
  appName: string;
  registryKeyPath: string;
  subkeyName: string;
  hive: string;
  fingerprint: string;
  expiresAt: number;
}

export interface RegistryRestoreReport {
  keysRestored: number;
  valuesRestored: number;
  errors: string[];
}

export interface SystemSpecs {
  osName: string;
  osBuild: string;
  osArchitecture: string;
  cpuBrand: string;
  cpuLogicalCores: number;
  totalMemoryBytes: number;
  availableMemoryBytes: number;
  memoryInUsePercent: number;
  gpuBrand: string | null;
  uptimeSeconds: number;
}

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  location: string;
  enabled: boolean;
  publisher: string | null;
  isProtected: boolean;
  canToggle: boolean;
}


export interface CpuMetrics {
  utilizationPercent: number;
  userPercent: number;
  kernelPercent: number;
  idlePercent: number;
  logicalCores: number;
}

export interface MemoryMetrics {
  totalBytes: number;
  availableBytes: number;
  usedPercent: number;
  commitUsedBytes: number;
  commitLimitBytes: number;
  commitPercent: number;
}

export interface DiskIoMetrics {
  status: "measured" | "unavailable";
  busyPercent: number | null;
  queueLength: number | null;
  note: string | null;
}

export interface DriverInfo {
  class: "Display" | "Storage" | "Network";
  name: string;
  version: string | null;
  /** ISO date (YYYY-MM-DD) recorded by the driver package. */
  date: string | null;
}

export interface DriverAnalysis {
  items: DriverInfo[];
  note: string | null;
}

export interface SystemEvent {
  observedAt: string;
  source: string;
  eventId: number;
  level: "critical" | "error" | "warning";
  category: "display" | "storage" | "hardware" | "power" | "driver" | "service" | "system";
  summary: string;
}

export interface SystemEventAnalysis {
  status: "available" | "denied" | "unavailable";
  note: string;
  items: SystemEvent[];
}

export interface ProcessUsage {
  pid: number;
  name: string;
  path: string | null;
  cpuPercent: number;
  workingSetBytes: number;
  privateBytes: number;
  ioReadBytesPerSec: number;
  ioWriteBytesPerSec: number;
  isCritical: boolean;
  isCyron: boolean;
}

export interface StartupEvidence {
  id: string;
  name: string;
  command: string;
  location: string;
  scope: "user" | "machine";
  enabled: boolean;
  canToggle: boolean;
  isProtected: boolean;
  launchPath: string | null;
  currentCpuPercent: number;
  currentWorkingSetBytes: number;
  bootImpactMs: number | null;
  bootTotalMs: number | null;
  evidenceSource: "diagnostics-performance" | "live-process" | "inventory";
}

export interface ScheduledStartupTask {
  name: string;
  path: string;
  author: string;
  enabled: boolean;
  triggers: ("boot" | "logon" | "registration")[];
  delay: string | null;
  command: string;
  arguments: string;
  isMicrosoft: boolean;
}

export interface BootSummary {
  observedAt: string;
  bootTimeMs: number;
  mainPathBootMs: number;
  postBootTimeMs: number;
  startupAppCount: number;
  degradationMs: number;
  isDegradation: boolean;
}

export interface BootOffender {
  eventId: number;
  name: string;
  friendlyName: string;
  path: string;
  totalTimeMs: number;
  degradationTimeMs: number;
  category: "application" | "driver" | "service" | "device" | "system";
}

export interface StartupAnalysis {
  items: StartupEvidence[];
  scheduledTasks: ScheduledStartupTask[];
  boot: BootSummary | null;
  offenders: BootOffender[];
  eventLogStatus: "available" | "denied" | "unavailable";
  eventLogNote: string;
  enabledCount: number;
  totalCount: number;
}

export interface RecommendationAction {
  kind: "startup" | "view" | "page" | "trace" | "none";
  label: string;
  targetId: string | null;
  view: ViewId | null;
  page: string | null;
}

export interface RecommendationEvidence {
  label: string;
  value: string;
}

export interface PerformanceRecommendation {
  id: string;
  category: "cpu" | "memory" | "disk" | "storage" | "startup" | "system";
  severity: "attention" | "review" | "info";
  title: string;
  summary: string;
  evidence: RecommendationEvidence[];
  expectedEffect: string;
  risk: string;
  verification: string;
  action: RecommendationAction;
}

export interface PerformanceFinding {
  id: string;
  tone: "ok" | "watch" | "attention";
  title: string;
  detail: string;
}

export interface PerformanceTraceStatus {
  available: boolean;
  active: boolean;
  startedAt: number | null;
  outputPath: string | null;
  message: string;
}

export interface PerformanceSnapshot {
  capturedAt: number;
  sampleMs: number;
  elevated: boolean;
  cpu: CpuMetrics;
  memory: MemoryMetrics;
  diskIo: DiskIoMetrics;
  drivers: DriverAnalysis;
  systemEvents: SystemEventAnalysis;
  volumes: VolumeInfo[];
  processes: ProcessUsage[];
  processCount: number;
  unmeasuredProcessCount: number;
  startup: StartupAnalysis;
  recommendations: PerformanceRecommendation[];
  findings: PerformanceFinding[];
  limitations: string[];
  trace: PerformanceTraceStatus;
}

export interface PerformanceBaseline {
  id: string;
  label: string;
  recordedAt: number;
  cpuPercent: number;
  memoryUsedPercent: number;
  availableMemoryBytes: number;
  commitPercent: number;
  diskBusyPercent: number | null;
  diskQueueLength: number | null;
  topCpuProcess: string | null;
  topMemoryProcess: string | null;
  bootTimeMs: number | null;
  enabledStartupCount: number;
}

/* ------------------------------------------------------- Native file index */

/** Lifecycle of Cyron's own on-disk search index. */
export type SearchIndexState = "empty" | "loading" | "building" | "ready" | "error";

export interface SearchIndexVolumeStatus {
  mount: string;
  label: string;
  state: "queued" | "indexing" | "done" | "error";
  error: string | null;
}

export interface SearchIndexProgress {
  /** Drive mount currently being walked, e.g. "C:\\". */
  volume: string;
  entries: number;
  currentDir: string;
}

export interface SearchIndexStatus {
  state: SearchIndexState;
  building: boolean;
  ready: boolean;
  entries: number;
  files: number;
  folders: number;
  totalSize: number;
  /** Unix seconds of the last completed build or load. */
  indexedAt: number | null;
  buildMs: number | null;
  volumes: SearchIndexVolumeStatus[];
  progress: SearchIndexProgress | null;
  warnings: string[];
  /** True while the backend is watching indexed volumes for changes. */
  watching: boolean;
  message: string;
}

export type SearchSortField =
  | "name"
  | "path"
  | "size"
  | "modified"
  | "created"
  | "extension";

export type SearchFilter =
  | "all"
  | "files"
  | "folders"
  | "audio"
  | "video"
  | "image"
  | "document"
  | "executable"
  | "compressed";

export interface SearchQueryOptions {
  query: string;
  matchCase: boolean;
  wholeWord: boolean;
  matchPath: boolean;
  regex: boolean;
  filter: SearchFilter;
  sort: SearchSortField;
  descending: boolean;
  offset: number;
  limit: number;
}

export interface SearchIndexItem {
  name: string;
  /** Parent folder, without the file or folder name. */
  parentPath: string;
  fullPath: string;
  isFolder: boolean;
  size: number;
  /** Unix seconds, when the file system reported it. */
  modified: number | null;
  created: number | null;
  extension: string;
}

export interface SearchIndexPage {
  items: SearchIndexItem[];
  total: number;
  files: number;
  folders: number;
  totalSize: number;
  offset: number;
  limit: number;
  hasMore: boolean;
  elapsedMs: number;
  /** True when the index stopped at its entry cap. */
  truncated: boolean;
}

export type SearchExportFormat = "csv" | "txt";

export interface SearchExportReport {
  path: string;
  rows: number;
  skipped: number;
  truncated: boolean;
}

export type AnalyzerAction = "recycle" | "permanent";
export interface AnalyzerActionTarget {
  path: string;
  files: number;
  dirs: number;
  bytes: number;
}
export interface AnalyzerActionPreview {
  planId: string;
  operation: AnalyzerAction;
  targets: AnalyzerActionTarget[];
  totalFiles: number;
  totalDirs: number;
  totalBytes: number;
  expiresAt: number;
  warnings: string[];
}
export interface AnalyzerActionProgress {
  planId: string;
  completed: number;
  total: number;
  currentPath: string;
}
export interface AnalyzerActionReport {
  planId: string;
  operation: AnalyzerAction;
  items: {path: string; status: "completed" | "failed" | "cancelled"; bytes: number; error?: string | null}[];
  completedFiles: number;
  completedDirs: number;
  completedBytes: number;
  cancelled: boolean;
}

