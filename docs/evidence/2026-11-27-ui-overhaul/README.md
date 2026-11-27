# UI overhaul evidence — 2026-11-27

These screenshots come from the Vite frontend running in headless Chromium, with the Tauri IPC calls replaced by mock data. They show layout and styling only. They are **not** native Windows captures. The live performance sampler (`perf_live.rs`) and the scan changes were compile-checked for `x86_64-pc-windows-msvc` but have not yet been run on Windows.

| File | Shows |
|---|---|
| `perf-cpu-light.png` | Performance → Monitor, CPU pane with the resource rail |
| `perf-cores-dark.png` | CPU logical-processor grid (dark) |
| `perf-memory-light.png` | Memory pane with composition bar |
| `perf-network-dark.png` | Network adapter send/receive chart |
| `perf-gpu-dark.png` | GPU engines and dedicated memory |
| `perf-processes-light.png` | Performance → Processes, grouped and heat-tinted |
| `disk-map-light.png`, `disk-map-dark.png` | Disk space nested treemap with framed folders |
| `disk-lazy-dark.png` | A summarized folder after on-demand loading |

UNVERIFIED: native WebView2 rendering, PDH counter values on real hardware, and treemap frame rate on a full-drive MFT scan.
