# Cyron scanner benchmarks

Measured with `scripts/bench-scan.mjs` on 2026-11-06. These are numbers from one machine, not a
product claim. Every MFT number requires an elevated (administrator) process; the harness refuses
to mislabel a fallback run as MFT.

## Environment

- Platform: Windows 11 build 10.0.26200 (x64)
- Node: v24.18.0
- Scanner: release build (`cargo build --release --example scan_bench`)
- OS file cache not flushed between runs; all numbers below are effectively warm-cache
- One run per target/method (the `Runs` column says so)

## Results

| Target | Method | Runs | Time | Peak working set | Files | Directories | Logical size |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `C:\` | MFT (elevated) | 1 | 14.52 s | 655.5 MiB | 2,211,292 | 604,557 | 332.3 GiB |
| `C:\` | directory walk | 1 | 155.24 s | 50.8 MiB | 2,916,028 | — | 405.6 GiB |
| `C:\Windows\System32` | MFT (elevated) | 1 | 3.91 s | 571.9 MiB | 4,488 | 5,369 | 5.66 GiB |
| `C:\Windows\System32` | directory walk | 2 | 0.28 s (median) | 7.8 MiB | 20,349 | — | 9.0 GiB |
| analyzer `node_modules` fixture | directory walk | 2 | 0.07 s (median) | 6.5 MiB | 3,097 | — | 85.2 MiB |

Commands:

```powershell
# Directory only; safe without elevation.
node scripts/bench-scan.mjs --path C:\ --method directory --runs 1

# Both methods. Must run in an Administrator terminal for MFT.
node scripts/bench-scan.mjs --path C:\ --method both --runs 1 --write
```

## What the numbers actually show

1. **Whole-volume MFT is about 10× faster than the directory walk on this machine** (14.5 s
   versus 155.2 s for `C:\`). The MFT path reads `$MFT` sequentially instead of opening every
   directory.
2. **MFT costs far more memory** — roughly 655 MiB peak versus 51 MiB. The MFT reader parses all
   records into a bounded in-memory map; the walk streams directory by directory.
3. **MFT is the wrong choice for a small subfolder.** `C:\Windows\System32` took 3.91 s with MFT
   because the reader loads and filters the whole volume's record set, while the directory walk
   finished in 0.28 s. Cyron therefore selects MFT automatically only when the scan root is a
   drive root; anywhere else it uses the directory walk. An explicit MFT request on a subfolder is
   allowed but is measurably slower.
4. **Totals are not directly comparable.** The MFT path counts each file record once, so files
   with several hard links contribute once. The directory walk counts every path, deduplicating
   hard links only while its one-million-id tracking set has room. That is why the same volume
   reports 2.21M files / 332.3 GiB with MFT and 2.92M files / 405.6 GiB with the walk. Both are
   correct under their own counting rule; neither is "the" size of the volume.

## What is not measured

- **Cold-cache behaviour.** No run flushes the OS cache, so the first read of a target may be cold
  and later runs warm. Do not present these as cold-start numbers.
- **CPU time, disk throughput or per-directory latency.** Only wall-clock scan duration and
  process peak working set are recorded.
- **Volumes above the record budget.** The MFT reader enforces a 768 MiB parse budget (4,910,404
  records at the current record size) and rejects a volume whose record count exceeds it before
  allocating; such a scan falls back to the directory walk. No measured fallback time is recorded
  for that case.
- **Comparison with WizTree or other products.** None was run. Do not claim parity or superiority.

## Interpreting the peak working set

The harness reports the process's peak working set after the scan call, which includes the runtime
and any retained result tree — an upper bound, not the incremental cost of the scan. Median of
multiple runs is the right summary for timings; a single-run column is labelled as such.
