/*
 * Browser-only stand-in for Cyron's Tauri IPC, used to render documentation
 * previews of the React frontend in a normal browser. Every value is sample
 * data. Nothing here touches a real file system, registry or process list.
 *
 * Injected with Playwright's addInitScript by scripts/capture-ui-preview.mjs.
 */
(() => {
  const GiB = 1024 ** 3;
  const MiB = 1024 ** 2;
  const now = Math.floor(Date.now() / 1000);
  const DAY = 86400;

  // Deterministic noise so captures are stable between runs.
  let seed = 7;
  const rand = () => {
    seed = (seed * 16807) % 2147483647;
    return (seed - 1) / 2147483646;
  };

  const app = (id, name, publisher, version, sizeMb, extra = {}) => ({
    id,
    name,
    version,
    publisher,
    installLocation: `C:\\Program Files\\${name}`,
    installDate: "20261102",
    uninstallString: `"C:\\Program Files\\${name}\\uninstall.exe"`,
    quietUninstallString: null,
    displayIcon: null,
    estimatedSize: sizeMb * MiB,
    hive: "HKLM",
    scope: "machine",
    architecture: "x64",
    isProtected: false,
    source: "win32",
    uninstallSupport: "standard",
    confidence: "confirmed",
    detectionReason: null,
    registryKeyName: id,
    ...extra,
  });

  const APPS = [
    app("adobe-cc", "Adobe Creative Cloud", "Adobe Inc.", "6.4.0.361", 1840),
    app("blender", "Blender", "Blender Foundation", "4.3.1", 1120),
    app("discord", "Discord", "Discord Inc.", "1.0.9172", 412, {
      scope: "user",
      hive: "HKCU",
    }),
    app("docker", "Docker Desktop", "Docker Inc.", "4.36.0", 2460),
    app("figma", "Figma", "Figma, Inc.", "124.6.5", 386, {
      scope: "user",
      hive: "HKCU",
    }),
    app("git", "Git", "The Git Development Community", "2.47.1", 330),
    app("chrome", "Google Chrome", "Google LLC", "131.0.6778.86", 620),
    app(
      "vscode",
      "Microsoft Visual Studio Code",
      "Microsoft Corporation",
      "1.95.3",
      402,
      { scope: "user", hive: "HKCU" },
    ),
    app("node", "Node.js", "Node.js Foundation", "22.11.0", 96),
    app("obs", "OBS Studio", "OBS Project", "30.2.3", 438),
    app("spotify", "Spotify", "Spotify AB", "1.2.50", 318, {
      scope: "user",
      hive: "HKCU",
    }),
    app("steam", "Steam", "Valve Corporation", "2.10.91", 760, {
      source: "steam",
      uninstallSupport: "manual_only",
    }),
    app("cyberpunk", "Cyberpunk 2077", "CD PROJEKT RED", "2.2", 71200, {
      source: "steam",
      uninstallSupport: "manual_only",
    }),
    app(
      "terminal",
      "Windows Terminal",
      "Microsoft Corporation",
      "1.21.3231",
      54,
      { source: "store", uninstallSupport: "store_managed", scope: "user" },
    ),
    app("7zip", "7-Zip 24.08 (x64)", "Igor Pavlov", "24.08", 6),
    app("vlc", "VLC media player", "VideoLAN", "3.0.21", 168),
    app("ollama", "Ollama", "Ollama", "0.4.7", 1210, {
      scope: "user",
      hive: "HKCU",
    }),
    app("portable-ripgrep", "ripgrep", null, null, 6, {
      source: "portable",
      uninstallSupport: "missing",
      confidence: "heuristic",
      uninstallString: null,
    }),
  ];

  const VOLUMES = [
    {
      mount: "C:\\",
      label: "Windows",
      fileSystem: "NTFS",
      total: 1000 * GiB,
      free: 312 * GiB,
      kind: "fixed",
    },
    {
      mount: "D:\\",
      label: "Games",
      fileSystem: "NTFS",
      total: 2000 * GiB,
      free: 744 * GiB,
      kind: "fixed",
    },
    {
      mount: "E:\\",
      label: "Projects",
      fileSystem: "NTFS",
      total: 1000 * GiB,
      free: 418 * GiB,
      kind: "fixed",
    },
  ];

  const cache = (id, tool, label, path, sizeMb, risk, category, note) => ({
    id,
    tool,
    label,
    path,
    exists: true,
    size: sizeMb * MiB,
    allocated: sizeMb * MiB,
    fileCount: Math.round(sizeMb * 37),
    dirCount: Math.round(sizeMb * 2),
    risk,
    category,
    note,
    lastModified: now - 3 * DAY,
    installState: "detected",
  });

  const CACHES = [
    cache(
      "npm",
      "npm",
      "npm cache",
      "C:\\Users\\you\\AppData\\Local\\npm-cache",
      3420,
      "safe",
      "packages",
      "Downloaded package tarballs. npm downloads them again when needed.",
    ),
    cache(
      "pnpm",
      "pnpm",
      "pnpm store",
      "C:\\Users\\you\\AppData\\Local\\pnpm\\store",
      5120,
      "caution",
      "packages",
      "Shared content-addressed store. Projects relink on the next install.",
    ),
    cache(
      "cargo",
      "Cargo",
      "Cargo registry cache",
      "C:\\Users\\you\\.cargo\\registry\\cache",
      1880,
      "safe",
      "packages",
      "Crate archives. Cargo fetches them again.",
    ),
    cache(
      "claude",
      "Claude Code",
      "Claude Code debug logs",
      "C:\\Users\\you\\.claude\\debug",
      612,
      "safe",
      "ai",
      "Debug logs, recreated on the next run.",
    ),
    cache(
      "cursor",
      "Cursor",
      "Cursor code cache",
      "C:\\Users\\you\\AppData\\Roaming\\Cursor\\Cache",
      940,
      "safe",
      "ide",
      "Chromium cache for the editor shell.",
    ),
    cache(
      "vscode",
      "VS Code",
      "VS Code cached extensions",
      "C:\\Users\\you\\AppData\\Roaming\\Code\\CachedExtensionVSIXs",
      720,
      "safe",
      "ide",
      "Extension installers kept after install.",
    ),
    cache(
      "ollama",
      "Ollama",
      "Ollama models",
      "C:\\Users\\you\\.ollama\\models",
      18400,
      "protected",
      "ai",
      "Model weights. Measured, never offered for removal.",
    ),
    cache(
      "hf",
      "Hugging Face",
      "Hugging Face hub",
      "C:\\Users\\you\\.cache\\huggingface\\hub",
      9600,
      "protected",
      "ai",
      "Model weights and datasets.",
    ),
    cache(
      "wintemp",
      "Windows",
      "Windows temporary files",
      "C:\\Windows\\Temp",
      2310,
      "caution",
      "windows",
      "Files in use are skipped.",
    ),
  ];
  const sum = (list, risk) =>
    list
      .filter((c) => !risk || c.risk === risk)
      .reduce((t, c) => t + c.size, 0);

  /* ------------------------------------------------------------ Disk tree */
  const tree = (() => {
    let modified = now - 40 * DAY;
    const file = (dir, name, size) => ({
      name,
      path: `${dir}\\${name}`,
      kind: "file",
      size,
      allocated: size,
      fileCount: 1,
      dirCount: 0,
      modified: (modified -= 3600),
      children: [],
    });
    const dir = (parent, name, spec) => {
      const path = parent ? `${parent}\\${name}` : name;
      const children = [];
      for (const [childName, value] of Object.entries(spec)) {
        if (typeof value === "number")
          children.push(file(path, childName, value * MiB));
        else children.push(dir(path, childName, value));
      }
      children.sort((a, b) => b.size - a.size);
      const size = children.reduce((t, c) => t + c.size, 0);
      return {
        name: parent ? name : path,
        path,
        kind: "dir",
        size,
        allocated: size,
        // Leaves stand in for many small files that are not drawn individually.
        fileCount:
          children.reduce((t, c) => t + c.fileCount, 0) +
          Math.round(size / (256 * 1024)),
        dirCount:
          children.reduce(
            (t, c) => t + (c.kind === "dir" ? 1 + c.dirCount : 0),
            0,
          ) + Math.round(size / (12 * MiB)),
        modified: (modified -= 7200),
        children,
      };
    };
    const many = (prefix, count, base, ext) => {
      const out = {};
      for (let i = 0; i < count; i += 1)
        out[`${prefix}${i + 1}.${ext}`] = Math.round(
          base * (0.4 + rand() * 1.4),
        );
      return out;
    };
    return dir("", "C:", {
      Windows: {
        System32: {
          ...many("driver-", 6, 600, "sys"),
          ...many("module-", 10, 420, "dll"),
        },
        WinSxS: many("amd64_component_", 12, 900, "dll"),
        Installer: many("package-", 7, 420, "msi"),
        SoftwareDistribution: many("update-", 5, 640, "cab"),
        Fonts: many("font-", 5, 90, "ttf"),
      },
      Users: {
        you: {
          AppData: {
            Docker: { "disk.vhdx": 24600 },
            "npm-cache": many("tarball-", 6, 560, "tgz"),
            Chrome: many("profile-", 4, 900, "db"),
            Temp: many("tmp-", 5, 300, "tmp"),
          },
          ".ollama": {
            "llama3.1-8b.gguf": 4920,
            "qwen2.5-coder-14b.gguf": 8990,
            "nomic-embed.gguf": 274,
          },
          Videos: many("capture-", 6, 2600, "mp4"),
          Downloads: {
            ...many("installer-", 6, 820, "exe"),
            "ubuntu-24.04.iso": 6100,
            ...many("archive-", 4, 700, "zip"),
          },
          Pictures: many("IMG_", 10, 420, "jpg"),
          Documents: {
            ...many("doc-", 6, 120, "pdf"),
            ...many("sheet-", 4, 60, "xlsx"),
          },
          Music: many("track-", 8, 180, "flac"),
        },
      },
      "Program Files": {
        Adobe: many("component-", 5, 860, "dll"),
        Docker: many("bin-", 4, 600, "exe"),
        Blender: many("lib-", 4, 300, "dll"),
        "Microsoft Office": many("app-", 5, 540, "dll"),
        NVIDIA: many("driver-", 4, 520, "dll"),
      },
      Games: {
        "Cyberpunk 2077": many("archive-", 8, 4100, "archive"),
        "Baldur's Gate 3": many("data-", 6, 5600, "pak"),
        "Hollow Knight": many("asset-", 4, 900, "assets"),
      },
      ProgramData: {
        Microsoft: many("data-", 5, 380, "dat"),
        Package_Cache: many("pkg-", 5, 260, "msi"),
      },
      "pagefile.sys": 16384,
      "hiberfil.sys": 12800,
    });
  })();

  const findNode = (node, path) => {
    if (node.path.toLowerCase() === path.toLowerCase().replace(/\\$/, ""))
      return node;
    for (const child of node.children) {
      const hit = findNode(child, path);
      if (hit) return hit;
    }
    return null;
  };

  const scanResult = (rootPath) => {
    const root = findNode(tree, rootPath) ?? tree;
    return {
      jobId: "preview",
      rootPath: rootPath.replace(/\\$/, "") === "C:" ? "C:\\" : rootPath,
      root: { ...root, path: root === tree ? "C:\\" : root.path },
      totalSize: root.size,
      totalAllocated: root.allocated,
      fileCount: root.fileCount,
      dirCount: root.dirCount,
      dedupedFiles: 1842,
      skippedReparse: 37,
      errors: [],
      extensionStats: [
        { extension: "vhdx", fileCount: 1, size: 24600 * MiB },
        { extension: "gguf", fileCount: 3, size: 14184 * MiB },
        { extension: "dll", fileCount: 38211, size: 41200 * MiB },
        { extension: "mp4", fileCount: 64, size: 15600 * MiB },
      ],
      largestFiles: [],
      elapsedMs: 4120,
      cancelled: false,
      truncated: false,
      method: "mft",
      methodNote: null,
      categories: [],
      ageBuckets: [],
      insights: [],
    };
  };

  /* --------------------------------------------------------- Live sampler */
  const CORES = 16;
  let tick = 0;
  const wave = (t, period, phase) =>
    Math.sin((t / period) * Math.PI * 2 + phase);
  const PROCESS_NAMES = [
    ["chrome.exe", true, 13.9, 1820],
    ["Code.exe", true, 9.3, 1240],
    ["explorer.exe", true, 7.3, 210],
    ["Discord.exe", true, 5.2, 640],
    ["Spotify.exe", true, 4.6, 380],
    ["Docker Desktop.exe", true, 2.1, 520],
    ["Figma.exe", true, 1.8, 760],
    ["obs64.exe", true, 3.4, 410],
    ["Cyron.exe", true, 0.6, 96],
    ["ollama.exe", false, 6.8, 5120],
    ["vmmemWSL", false, 3.1, 4200],
    ["node.exe", false, 2.4, 310],
    ["MsMpEng.exe", false, 1.2, 280],
    ["SearchIndexer.exe", false, 0.8, 90],
    ["OneDrive.exe", false, 0.4, 150],
    ["svchost.exe", false, 0.9, 64],
    ["svchost.exe", false, 0.3, 38],
    ["dwm.exe", false, 2.2, 180],
    ["csrss.exe", false, 0.2, 8],
    ["System", false, 1.1, 4],
    ["lsass.exe", false, 0.1, 22],
    ["audiodg.exe", false, 0.5, 30],
  ];
  const CRITICAL = new Set([
    "dwm.exe",
    "csrss.exe",
    "System",
    "lsass.exe",
    "svchost.exe",
    "audiodg.exe",
  ]);

  const sample = (includeProcesses) => {
    tick += 1;
    const t = tick;
    const base =
      26 +
      9 * wave(t, 23, 0) +
      6 * wave(t, 7, 1.3) +
      (t % 31 > 26 ? 22 : 0) +
      rand() * 5;
    const cpu = Math.max(4, Math.min(96, base));
    const perCore = Array.from({ length: CORES }, (_, i) =>
      Math.max(
        1,
        Math.min(
          100,
          cpu * (0.5 + ((i * 37) % 11) / 10) +
            10 * wave(t + i * 3, 9, i) +
            rand() * 6,
        ),
      ),
    );
    const mem = 18.6 * GiB + 0.4 * GiB * wave(t, 40, 0.4);
    const gpu = 34 + 22 * wave(t, 17, 2) + rand() * 6;
    return {
      capturedAt: Date.now(),
      intervalMs: 1000,
      warm: true,
      cpu: {
        utilizationPercent: cpu,
        userPercent: cpu * 0.78,
        kernelPercent: cpu * 0.22,
        perCorePercent: perCore,
        speedMhz: 4560 + Math.round(80 * wave(t, 13, 0)),
      },
      memory: {
        totalBytes: 32 * GiB,
        availableBytes: 32 * GiB - mem,
        usedBytes: mem,
        usedPercent: (mem / (32 * GiB)) * 100,
        commitUsedBytes: 24.1 * GiB,
        commitLimitBytes: 37 * GiB,
        cachedBytes: 9.8 * GiB,
        pagedPoolBytes: 1.1 * GiB,
        nonpagedPoolBytes: 0.6 * GiB,
        modifiedBytes: 0.3 * GiB,
        standbyBytes: 9.5 * GiB,
        freeBytes: 32 * GiB - mem - 9.8 * GiB,
      },
      disks: [
        {
          index: 0,
          volumes: ["C:"],
          activePercent: Math.max(0, 9 + 12 * wave(t, 11, 0) + rand() * 6),
          readBytesPerSec: (18 + 30 * rand()) * MiB,
          writeBytesPerSec: (6 + 14 * rand()) * MiB,
          responseMs: 0.4 + rand(),
        },
        {
          index: 1,
          volumes: ["D:", "E:"],
          activePercent: Math.max(0, 2 + 3 * wave(t, 19, 1) + rand() * 2),
          readBytesPerSec: rand() * 4 * MiB,
          writeBytesPerSec: rand() * MiB,
          responseMs: 1.2 + rand(),
        },
      ],
      networks: [
        {
          name: "Wi-Fi",
          kind: "wifi",
          receivedBytesPerSec: (3 + 4 * (1 + wave(t, 15, 0.7))) * MiB,
          sentBytesPerSec: (0.08 + 0.1 * rand()) * MiB,
          linkBitsPerSec: 1.2e9,
        },
      ],
      gpus: [
        {
          index: 0,
          name: "NVIDIA GeForce RTX 4070",
          luid: "0x0000d1a2",
          utilizationPercent: gpu,
          engines: [
            { name: "3D", percent: gpu },
            { name: "Copy", percent: 4 + rand() * 3 },
            { name: "Video Encode", percent: 18 + 6 * wave(t, 9, 0) },
            { name: "Video Decode", percent: 6 + rand() * 4 },
          ],
          dedicatedUsedBytes: 7.4 * GiB,
          sharedUsedBytes: 0.4 * GiB,
        },
      ],
      system: {
        processCount: 287,
        threadCount: 4012,
        handleCount: 131204,
        uptimeSeconds: 11052 + t,
      },
      processes: includeProcesses
        ? PROCESS_NAMES.map(([name, hasWindow, cpuShare, wsMb], i) => ({
            pid: 1000 + i * 412,
            name,
            path: CRITICAL.has(name)
              ? `C:\\Windows\\System32\\${name}`
              : `C:\\Program Files\\${name.replace(/\.exe$/, "")}\\${name}`,
            cpuPercent: Math.max(0, cpuShare * (cpu / 28) + rand() * 0.6),
            workingSetBytes: wsMb * MiB,
            privateBytes: wsMb * 0.8 * MiB,
            ioBytesPerSec: (cpuShare / 4 + rand()) * 0.4 * MiB,
            hasWindow,
            isCritical: CRITICAL.has(name),
            isCyron: name === "Cyron.exe",
          }))
        : null,
      hardware: {
        cpuName: "AMD Ryzen 7 7800X3D 8-Core Processor",
        baseSpeedMhz: 4200,
        sockets: 1,
        cores: 8,
        logicalProcessors: CORES,
        l1CacheBytes: 512 * 1024,
        l2CacheBytes: 8 * MiB,
        l3CacheBytes: 96 * MiB,
        installedMemoryBytes: 32 * GiB,
        gpuNames: ["NVIDIA GeForce RTX 4070"],
      },
      notes: [],
    };
  };

  /* ------------------------------------------------------------- Search */
  const SEARCH_ITEMS = [
    ["quarterly-report-2026.pdf", "C:\\Users\\you\\Documents\\Finance", 2.4],
    ["report-draft.docx", "C:\\Users\\you\\Documents", 0.3],
    ["report.config.ts", "E:\\Projects\\dashboard\\src", 0.004],
    ["incident-report.md", "E:\\Projects\\cyron\\docs", 0.012],
    ["report-assets", "E:\\Projects\\dashboard\\public", 0, true],
    [
      "crash-report-1127.dmp",
      "C:\\Users\\you\\AppData\\Local\\CrashDumps",
      184,
    ],
    ["report-template.xlsx", "C:\\Users\\you\\Documents\\Templates", 0.09],
    ["reports", "C:\\Users\\you\\OneDrive", 0, true],
    ["report-cover.png", "C:\\Users\\you\\Pictures\\Design", 3.1],
    ["test-report.html", "E:\\Projects\\cyron\\coverage", 0.8],
  ].map(([name, parentPath, sizeMb, isFolder], i) => ({
    name,
    parentPath,
    fullPath: `${parentPath}\\${name}`,
    isFolder: Boolean(isFolder),
    size: Math.round(sizeMb * MiB),
    modified: now - i * DAY * 2,
    created: now - 90 * DAY,
    extension: isFolder ? "" : name.split(".").pop(),
  }));

  const searchStatus = {
    state: "ready",
    building: false,
    ready: true,
    entries: 2184310,
    files: 1962114,
    folders: 222196,
    totalSize: 2140 * GiB,
    indexedAt: now - 600,
    buildMs: 14820,
    volumes: VOLUMES.map((v) => ({
      mount: v.mount,
      label: v.label,
      state: "done",
      error: null,
    })),
    progress: null,
    warnings: [],
    watching: true,
    message: "Index ready",
  };

  /* --------------------------------------------------------- Dispatcher */
  const handlers = {
    list_installed_apps: () => ({ apps: APPS, warnings: [] }),
    list_removed_apps: () => [],
    list_app_history: () => [],
    list_volumes: () => VOLUMES,
    list_caches: () => ({
      targets: CACHES,
      totalSize: sum(CACHES),
      totalSafe: sum(CACHES, "safe"),
      totalCaution: sum(CACHES, "caution"),
      totalProtected: sum(CACHES, "protected"),
      scannedAt: now,
      warnings: [],
    }),
    get_cleanup_mode: () => true,
    set_cleanup_mode: (args) => Boolean(args.readOnly),
    get_system_specs: () => ({
      osName: "Windows 11 Pro",
      osBuild: "26100.2454",
      osArchitecture: "x64",
      cpuBrand: "AMD Ryzen 7 7800X3D 8-Core Processor",
      cpuLogicalCores: CORES,
      totalMemoryBytes: 32 * GiB,
      availableMemoryBytes: 13.4 * GiB,
      memoryInUsePercent: 58,
      gpuBrand: "NVIDIA GeForce RTX 4070",
      uptimeSeconds: 11052,
    }),
    get_history: () => [
      {
        id: "h1",
        kind: "cleanup",
        title: "Cleaned developer caches",
        detail: "npm cache, Cargo registry cache",
        at: now - 2 * 3600,
        bytes: 5300 * MiB,
        items: 2,
        status: "completed",
      },
      {
        id: "h2",
        kind: "uninstall",
        title: "Uninstalled Zoom",
        detail: "Vendor uninstaller exited 0 and the app left the inventory",
        at: now - DAY,
        bytes: 412 * MiB,
        items: 1,
        status: "completed",
      },
      {
        id: "h3",
        kind: "scan",
        title: "Scanned C:\\",
        detail: "NTFS MFT · 1.9 million files",
        at: now - 2 * DAY,
        bytes: 0,
        items: 0,
        status: "completed",
      },
    ],
    list_quarantine: () => [
      {
        runId: "run-20261127-1412",
        createdAt: now - 2 * 3600,
        expiresAt: now + 28 * DAY,
        bytes: 5300 * MiB,
        fileCount: 18422,
        dir: "%APPDATA%\\dev.cyron.uninstaller\\quarantine\\run-20261127-1412",
        targetIds: ["npm", "cargo"],
      },
      {
        runId: "run-20261125-0930",
        createdAt: now - 2 * DAY,
        expiresAt: now + 26 * DAY,
        bytes: 612 * MiB,
        fileCount: 2210,
        dir: "%APPDATA%\\dev.cyron.uninstaller\\quarantine\\run-20261125-0930",
        targetIds: ["claude"],
      },
    ],
    get_cleanup_log: () => [
      {
        id: "l1",
        at: now - 2 * 3600,
        title: "Developer caches",
        targets: ["npm cache", "Cargo registry cache"],
        bytes: 5300 * MiB,
        files: 18422,
        quarantined: 18422,
        skipped: 0,
        failed: 0,
        status: "completed",
        detail: "Moved to quarantine",
      },
      {
        id: "l2",
        at: now - 2 * DAY,
        title: "Claude Code debug logs",
        targets: ["Claude Code debug logs"],
        bytes: 612 * MiB,
        files: 2210,
        quarantined: 2204,
        skipped: 6,
        failed: 0,
        status: "partial",
        detail: "6 files were in use and skipped",
      },
    ],
    start_scan: (args) => scanResult(args.path),
    cancel_scan: () => null,
    query_directory_children: (args) => {
      const node = findNode(tree, args.query.dirPath);
      const entries = node ? node.children : [];
      return {
        entries,
        totalEntries: entries.length,
        page: 1,
        pageSize: entries.length,
        hasMore: false,
      };
    },
    sample_live_performance: (args) =>
      sample(Boolean(args && args.includeProcesses)),
    search_index_status: () => searchStatus,
    query_search_index: (args) => {
      const q = (args.options.query || "").toLowerCase();
      const items = SEARCH_ITEMS.filter(
        (item) => !q || item.name.toLowerCase().includes(q.split(" ")[0]),
      );
      return {
        items,
        total: items.length,
        files: items.filter((i) => !i.isFolder).length,
        folders: items.filter((i) => i.isFolder).length,
        totalSize: items.reduce((t, i) => t + i.size, 0),
        offset: 0,
        limit: args.options.limit,
        hasMore: false,
        elapsedMs: 3,
        truncated: false,
      };
    },
    list_startup_items: () => [],
    get_startup_route: () => null,
    get_explorer_integration_status: () => ({
      standard: false,
      advanced: false,
    }),
    list_performance_baselines: () => [],
  };

  let callbackId = 1;
  window.__CYRON_PREVIEW__ = true;
  window.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
    plugins: {},
    transformCallback: () => callbackId++,
    unregisterCallback: () => undefined,
    convertFileSrc: (path) => path,
    invoke: async (command, args) => {
      if (command === "plugin:app|version") return "0.2.4";
      if (command.startsWith("plugin:"))
        return command.endsWith("|listen") ? callbackId++ : null;
      const handler = handlers[command];
      if (!handler) {
        console.warn(`[preview] no mock for ${command}`);
        throw new Error(
          `Preview: ${command} is not available in the browser preview.`,
        );
      }
      return handler(args || {});
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => undefined,
  };
})();
