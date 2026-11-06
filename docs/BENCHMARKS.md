# Cyron Scanner & Architectural Engine Benchmarks

Measured on Windows 11 x64 using `scripts/benchmark-competitive.mjs` and `scripts/bench-scan.mjs`.
All figures represent measured executions on identical test fixtures and real system directories on the local test machine.

---

## 1. Measured Performance & Memory Results (Cyron)

| Target | Scanner Mode | Wall-Clock Duration | Peak Working Set | Total Files | Logical Size |
|---|---|---|---|---|---|
| **Cyron Project Source** | Directory Walk | 1 ms | 4.88 MiB | 43 | 0.46 MB |
| **Node Modules (Dense Tree)** | Directory Walk | 60 ms | 6.12 MiB | 3,123 | 88.20 MB |
| **Windows System32 (Subfolder)** | Directory Walk | 206 ms | 7.77 MiB | 20,349 | 9254.45 MB |
| **Drive C:\ (Whole Volume)** | MFT (Elevated) | 14.52 s | 655.5 MiB | 2,211,292 | 332.3 GiB |
| **Drive C:\ (Whole Volume)** | Directory Walk | 155.24 s | 50.8 MiB | 2,916,028 | 405.6 GiB |

*Note: Measurements reflect warm-cache runs on a single development machine. Cold-cache performance varies depending on storage hardware and OS disk caching.*

---

## 2. Architectural Comparison

| Dimension | WinDirStat | WizTree | BleachBit | Cyron Uninstaller |
|---|---|---|---|---|
| **Scan Strategy** | Recursive tree build via standard Win32 directory APIs | Direct NTFS `$MFT` volume record parsing (requires elevation) | Path scanning against configured cleaner definitions | **Hybrid Auto Engine**: Elevated MFT on drive roots (10× faster); streaming depth-first directory walk on subfolders |
| **Hard Link Handling** | Counts hard links by path appearance | Single-counts per MFT file record | Does not calculate volume allocation | **Tracked Deduplication**: 1M ID tracking set; reports logical size and physical allocated size independently |
| **Memory Boundaries** | In-memory directory tree representation | Record buffering during MFT analysis | File path iteration per cleaner | **Strictly Capped**: Max 100K nodes in summarized IPC tree; 768 MiB MFT parse limit |
| **Cancellation Model** | Synchronous worker loop | Stops enumeration | Interrupts cleanup pass | **Atomic & Throttled**: `AtomicBool` polled per directory & entry; UI response latency < 120 ms |
| **Safety & Recovery** | Read-only analysis | Read-only analysis | Direct unrecoverable file deletion | **Review-First Quarantine**: Single-use confirmation plans, write-ahead journal with `sync_all`, atomic `MoveFileExW` restore |
| **Application Inventory** | None | None | None | **Multi-Source**: Win32 registry (64/32), Store (MSIX), Steam libraries, and portable heuristic scanning |
| **Shell Integration** | None | Explorer context menu | Explorer context menu | **HKCU Shell Verbs**: Right-click Analyze, Find Duplicates, Storage Breakdown, Uninstall Review, Related Files |

---

## 3. Counting Rules & Discrepancies Explained

1. **Path Count vs Record Count:**
   - The directory walk counts accessible filesystem paths. If a file has 3 hard links, it will be encountered 3 times during the traversal.
   - Cyron's directory walker tracks up to 1,000,000 unique file IDs (using `FileIdBothDirectoryInfo`), allowing allocated on-disk bytes to be counted exactly once while reporting path appearances accurately.
   - Standard path walkers count each hard link as a distinct file, causing large system directories (such as `C:\Windows\WinSxS`) to appear significantly larger than their physical on-disk footprint.
   - MFT parsers read raw NTFS file records from the Master File Table. Each record is counted once, matching physical cluster allocation.

2. **Subfolder vs Whole Volume Selection:**
   - On a full drive root (e.g. `C:\`), MFT reading is ~10× faster than a directory walk (14.52 s vs 155.24 s) because it performs contiguous disk reads rather than traversing millions of discrete directory structures.
   - On subfolders (such as `C:\Windows\System32`), MFT is ~14× slower (3.91 s vs 0.28 s) because it must still parse the entire volume's MFT records to filter for the target prefix.
   - Cyron automatically selects MFT only when scanning drive roots with administrative elevation, and uses streaming directory walks elsewhere.

3. **Interruption and Responsiveness:**
   - Scanning operations never block the desktop interface. An `AtomicBool` cancellation flag is checked per entry, ensuring user-initiated cancellation takes effect within 120 ms.
