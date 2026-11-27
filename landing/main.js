import {
  BENCHMARKS,
  BENCHMARK_SOURCE,
  COMPARISON,
  LEGEND,
  STRESS_BENCHMARKS,
  STRESS_COMPARISON,
  STRESS_TOOLS,
  TOOLS,
} from "./data.js";
import { RELEASE } from "./release.js";

const root = document.documentElement;
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
const systemReduce = window.matchMedia("(prefers-reduced-motion: reduce)");
const THEME_KEY = "cyron-theme";
const MOTION_KEY = "cyron-motion";

root.classList.add("js");

let themeChoice = root.dataset.themeChoice || "system";
let motionChoice = root.dataset.motionChoice || "system";
let reduceMotion =
  motionChoice === "system" ? systemReduce.matches : motionChoice === "reduce";

function isDark() {
  return themeChoice === "system" ? systemDark.matches : themeChoice === "dark";
}

function isReduced() {
  return motionChoice === "system"
    ? systemReduce.matches
    : motionChoice === "reduce";
}

function persist(key, value) {
  try {
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
  } catch (error) {
    /* Storage may be unavailable; the choice still applies for this visit. */
  }
}

function syncPreferenceControls() {
  const dark = isDark();
  const reduced = isReduced();
  document.querySelectorAll("[data-pref]").forEach((button) => {
    if (button.dataset.pref === "theme")
      button.setAttribute("aria-pressed", String(dark));
    if (button.dataset.pref === "motion")
      button.setAttribute("aria-pressed", String(reduced));
  });
}

function refreshMotionPresentation() {
  document.querySelectorAll(".reveal, .bench-row").forEach((node) => {
    if (reduceMotion) node.classList.add("is-visible");
    else if (node.getBoundingClientRect().top > window.innerHeight * 1.1) {
      node.classList.remove("is-visible");
    }
  });
  if (!reduceMotion) {
    observeReveals();
    observeBenchRows();
  }
}

function applyPreferences() {
  const dark = isDark();
  reduceMotion = isReduced();

  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.themeChoice = themeChoice;
  root.dataset.motion = reduceMotion ? "reduce" : "full";
  root.dataset.motionChoice = motionChoice;
  root.style.colorScheme = dark ? "dark" : "light";
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", dark ? "#0a0a0c" : "#f5f5f7");

  syncPreferenceControls();
  refreshMotionPresentation();
  bigPlayed = false;
  observeBigNumber();
  if (typeof layoutTour === "function") layoutTour();
  updateScroll();
}

document.addEventListener("click", (event) => {
  const button = event.target.closest("[data-pref]");
  if (!button) return;
  const pref = button.dataset.pref;

  if (pref === "system") {
    themeChoice = "system";
    motionChoice = "system";
    persist(THEME_KEY, null);
    persist(MOTION_KEY, null);
  } else if (pref === "theme") {
    themeChoice = isDark() ? "light" : "dark";
    persist(THEME_KEY, themeChoice);
  } else if (pref === "motion") {
    motionChoice = isReduced() ? "full" : "reduce";
    persist(MOTION_KEY, motionChoice);
  } else {
    return;
  }

  applyPreferences();
});

const clamp = (value, min = 0, max = 1) => Math.min(max, Math.max(min, value));
const numberFormat = new Intl.NumberFormat("en-US");

function formatSeconds(value) {
  if (!Number.isFinite(value)) return "—";
  if (value > 0 && value < 0.01) return "0.00s";
  return `${value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}s`;
}

function formatFiles(value) {
  return numberFormat.format(value);
}

function breakablePath(value) {
  return value.replace(/\\/g, "\\\u200B");
}

/* Benchmark rendering ----------------------------------------------------- */
const benchList = document.querySelector("#benchList");
const benchHero = document.querySelector(".bench-hero");
const benchNumber = document.querySelector(".bench-number");
const benchKicker = document.querySelector(".bench-kicker");
const benchSub = document.querySelector(".bench-sub");
const benchChip = document.querySelector("[data-bench-chip]");
const benchPeaks = document.querySelector("[data-bench-peaks]");
const benchRuns = document.querySelector("[data-bench-runs]");
let currentBenchmarks = BENCHMARKS;

function applyRelease() {
  document.querySelectorAll("[data-release-version]").forEach((node) => {
    node.textContent = RELEASE.tag;
  });
  document.querySelectorAll("[data-release-number]").forEach((node) => {
    node.textContent = RELEASE.version;
  });

  const download = document.querySelector("[data-release-download]");
  if (download) download.href = RELEASE.releaseUrl;
  const notes = document.querySelector("[data-release-notes]");
  if (notes) notes.href = RELEASE.notesUrl;
  const benchmarkSource = document.querySelector("[data-benchmark-source]");
  if (benchmarkSource) benchmarkSource.href = RELEASE.benchmarkUrl;

  const label = document.querySelector("[data-release-label]");
  if (label) label.textContent = RELEASE.downloadLabel;
  const channel = document.querySelector("[data-release-channel]");
  if (channel) channel.textContent = RELEASE.channel;
  const warning = document.querySelector("[data-release-warning]");
  if (warning) warning.textContent = RELEASE.warning;
}

function renderBenchmarks(rows) {
  currentBenchmarks = rows;
  if (!benchList) return;
  const max = Math.max(...rows.map((row) => row.median), 0.0001);
  benchList.innerHTML = rows
    .map((row) => {
      const width = row.median > 0 ? Math.max(2, (row.median / max) * 100) : 0;
      return `
        <article class="bench-row">
          <div class="bench-meta">
            <span class="bench-label" title="${row.full.replace(/"/g, "&quot;")}">${breakablePath(row.label)}</span>
            <span class="bench-files">${formatFiles(row.files)} files · ${row.size} · peak ${row.peak}</span>
          </div>
          <div
            class="bench-track"
            role="img"
            aria-label="${row.label.replace(/"/g, "&quot;")}: median ${formatSeconds(row.median)} for ${formatFiles(row.files)} files"
          >
            <div class="bench-fill" style="--w: ${width}%"></div>
          </div>
          <span class="bench-value">${formatSeconds(row.median)}</span>
        </article>`;
    })
    .join("");
  observeBenchRows();
}

let benchObserver;
function observeBenchRows() {
  if (!benchList) return;
  benchObserver?.disconnect();
  benchObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) entry.target.classList.add("is-visible");
      }
    },
    { rootMargin: "0px 0px -5% 0px", threshold: 0.1 },
  );
  benchList.querySelectorAll(".bench-row").forEach((row) => {
    if (reduceMotion) row.classList.add("is-visible");
    benchObserver.observe(row);
  });
}

/* Big number count-up ----------------------------------------------------- */
let bigObserver;
let bigTarget = BENCHMARKS[0].median;
let bigPlayed = false;

function renderBenchHero(row, stress = false) {
  bigTarget = row.median;
  if (benchNumber) {
    benchNumber.dataset.count = String(row.median);
    if (reduceMotion) {
      benchNumber.textContent = row.median.toLocaleString("en-US", {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      });
    }
  }
  if (benchKicker)
    benchKicker.textContent = stress ? breakablePath(row.label) : row.full;
  if (benchSub) {
    benchSub.innerHTML = stress
      ? "to count <strong>1,284,000,000 files</strong> and 9,999.9 GiB"
      : `to count <strong>${formatFiles(row.files)} files</strong> and ${row.size}`;
  }
  if (benchChip) {
    benchChip.textContent = stress
      ? "Stress fixture · not a measurement"
      : `Median of ${BENCHMARK_SOURCE.runs} runs · ${BENCHMARK_SOURCE.methodLabel}`;
  }
  if (benchRuns) benchRuns.textContent = String(BENCHMARK_SOURCE.runs);
  if (benchPeaks) {
    const peaks = BENCHMARKS.map((entry) =>
      Number.parseFloat(entry.peak),
    ).filter(Number.isFinite);
    benchPeaks.textContent =
      stress || !peaks.length
        ? "—"
        : `${Math.min(...peaks)}–${Math.max(...peaks)} MiB`;
  }
}

function animateBigNumber(target) {
  if (!benchNumber) return;
  bigTarget = target;
  if (reduceMotion) {
    benchNumber.textContent = target.toLocaleString("en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
    return;
  }
  const start = performance.now();
  const duration = 880;
  function frame(now) {
    const t = clamp((now - start) / duration);
    const eased = 1 - Math.pow(1 - t, 3);
    benchNumber.textContent = (target * eased).toLocaleString("en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
    if (t < 1) requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

function observeBigNumber() {
  if (!benchHero || !benchNumber) return;
  bigObserver?.disconnect();
  bigObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting || bigPlayed) continue;
        bigPlayed = true;
        animateBigNumber(bigTarget);
      }
    },
    { threshold: 0.35 },
  );
  bigObserver.observe(benchHero);
}

/* Comparison rendering ---------------------------------------------------- */
const compareTable = document.querySelector("#compareTable");

function renderComparison(rows, tools) {
  if (!compareTable) return;
  compareTable.innerHTML = `
    <caption class="sr-only">
      Documented capability comparison between Cyron and four other Windows utilities. A
      documented feature is not proof that it works on this machine, and no competitor timing was
      measured.
    </caption>
    <thead>
      <tr>
        <th scope="col">Capability</th>
        ${tools.map((tool) => `<th scope="col">${tool}</th>`).join("")}
      </tr>
    </thead>
    <tbody>
      ${rows
        .map(
          (row) => `
        <tr>
          <th scope="row">${row.label}</th>
          ${row.values
            .map((value) => {
              const label = LEGEND[value] ?? value;
              const title =
                value === "reported"
                  ? "Documented by a third party; not verified from the official feature page"
                  : value === "unknown"
                    ? "Not established from the cited source"
                    : value === "notrun"
                      ? "No same-machine benchmark was run"
                      : "";
              return `<td><span class="mark ${value}"${title ? ` title="${title}"` : ""}><i aria-hidden="true"></i>${label}</span></td>`;
            })
            .join("")}
        </tr>`,
        )
        .join("")}
    </tbody>`;
}

/* Reveal, nav and scroll-linked zoom ------------------------------------- */
const revealObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add("is-visible");
      revealObserver.unobserve(entry.target);
    }
  },
  { rootMargin: "0px 0px -7% 0px", threshold: 0.06 },
);

function observeReveals() {
  document.querySelectorAll(".reveal").forEach((node) => {
    if (reduceMotion) node.classList.add("is-visible");
    else revealObserver.observe(node);
  });
}

const progressBar = document.querySelector(".scroll-progress span");
const nav = document.querySelector("#nav");
const showcaseGrid = document.querySelector(".showcase-grid");
const showcaseFrame = document.querySelector("#showcaseFrame");
const heroStage = document.querySelector(".hero-stage");
let scrollQueued = false;

function updateScroll() {
  scrollQueued = false;
  const scrollTop = window.scrollY || document.documentElement.scrollTop;
  const max = document.documentElement.scrollHeight - window.innerHeight;
  if (progressBar)
    progressBar.style.width = `${max > 0 ? clamp(scrollTop / max) * 100 : 0}%`;
  if (nav) nav.classList.toggle("is-scrolled", scrollTop > 12);

  if (heroStage) {
    const rect = heroStage.getBoundingClientRect();
    const hp = reduceMotion
      ? 1
      : clamp((window.innerHeight - rect.top) / (window.innerHeight * 0.75));
    heroStage.style.setProperty("--hp", hp.toFixed(3));
  }
  updateManifesto();
  updateTour();

  if (showcaseGrid && showcaseFrame) {
    if (reduceMotion) {
      showcaseFrame.style.setProperty("--p", "0");
    } else {
      const rect = showcaseGrid.getBoundingClientRect();
      const start = window.innerHeight * 0.85;
      const distance = rect.height + window.innerHeight * 0.3;
      const p = distance > 0 ? clamp((start - rect.top) / distance) : 0;
      showcaseFrame.style.setProperty("--p", p.toFixed(3));
    }
  }
}

function queueScroll() {
  if (scrollQueued) return;
  scrollQueued = true;
  requestAnimationFrame(updateScroll);
}

/* Capability steps -------------------------------------------------------- */
const steps = [...document.querySelectorAll(".step")];
const panels = [...document.querySelectorAll(".panel")];
const stepDots = [...document.querySelectorAll("[data-goto]")];
const showcaseTitle = document.querySelector("#showcaseTitle");
const showcaseLive = document.querySelector("#showcaseFrame .frame-live");

function activateStep(index) {
  steps.forEach((step, i) => step.classList.toggle("is-active", i === index));
  panels.forEach((panel, i) =>
    panel.classList.toggle("is-active", i === index),
  );
  stepDots.forEach((dot, i) => {
    if (i === index) dot.setAttribute("aria-current", "true");
    else dot.removeAttribute("aria-current");
  });
  const step = steps[index];
  if (showcaseTitle) showcaseTitle.textContent = step?.dataset.title ?? "Cyron";
  if (showcaseLive)
    showcaseLive.innerHTML = `<i></i>${step?.dataset.live ?? "Review"}`;
}

const stepObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const index = Number(entry.target.dataset.step);
      if (Number.isInteger(index)) activateStep(index);
    }
  },
  { rootMargin: "-38% 0px -38% 0px", threshold: 0 },
);

steps.forEach((step) => stepObserver.observe(step));

stepDots.forEach((dot) => {
  dot.addEventListener("click", () => {
    const index = Number(dot.dataset.goto);
    activateStep(index);
    steps[index]?.scrollIntoView({
      behavior: reduceMotion ? "auto" : "smooth",
      block: "center",
    });
  });
});

/* Section spy for the navigation ------------------------------------------ */
const spyLinks = [...document.querySelectorAll("[data-spy]")];
const spyObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      spyLinks.forEach((link) => {
        const current = link.dataset.spy === entry.target.id;
        link.classList.toggle("is-current", current);
        if (current) link.setAttribute("aria-current", "true");
        else link.removeAttribute("aria-current");
      });
    }
  },
  { rootMargin: "-45% 0px -50% 0px", threshold: 0 },
);
spyLinks.forEach((link) => {
  const section = document.getElementById(link.dataset.spy);
  if (section) spyObserver.observe(section);
});

/* Manifesto: split into words that light up with scroll ------------------- */
const manifesto = document.querySelector("[data-scrub]");
const KEY_WORDS = new Set(["measures", "plan", "nothing", "back."]);
let manifestoWords = [];
if (manifesto) {
  const words = manifesto.textContent.trim().split(/\s+/);
  manifesto.setAttribute("aria-label", words.join(" "));
  manifesto.innerHTML = words
    .map((word) => {
      const key = KEY_WORDS.has(word.toLowerCase().replace(/[—,]/g, ""))
        ? " is-key"
        : "";
      return `<span class="word${key}" aria-hidden="true">${word}</span>`;
    })
    .join(" ");
  manifestoWords = [...manifesto.querySelectorAll(".word")];
}

function updateManifesto() {
  if (!manifesto || !manifestoWords.length) return;
  if (reduceMotion) {
    manifestoWords.forEach((word) => word.classList.add("is-lit"));
    return;
  }
  const rect = manifesto.getBoundingClientRect();
  const start = window.innerHeight * 0.85;
  const end = window.innerHeight * 0.3;
  const p = clamp((start - rect.top) / (start - end + rect.height * 0.6));
  const lit = Math.round(p * manifestoWords.length);
  manifestoWords.forEach((word, i) => word.classList.toggle("is-lit", i < lit));
}

/* Tour: pin and translate horizontally on wide screens -------------------- */
const tour = document.querySelector(".tour");
const tourTrack = document.querySelector(".tour-track");
const tourViewport = document.querySelector(".tour-viewport");
let tourDistance = 0;

function layoutTour() {
  if (!tour || !tourTrack) return;
  const pin =
    !reduceMotion && window.innerWidth > 900 && window.innerHeight > 560;
  tour.classList.toggle("is-pinned", pin);
  if (!pin) {
    tour.style.height = "";
    tourTrack.style.removeProperty("--tx");
    return;
  }
  tourDistance = Math.max(0, tourTrack.scrollWidth - window.innerWidth);
  tour.style.height = `${window.innerHeight + tourDistance}px`;
}

function updateTour() {
  if (!tour || !tourTrack) return;
  let p;
  if (tour.classList.contains("is-pinned")) {
    const rect = tour.getBoundingClientRect();
    p = tourDistance > 0 ? clamp(-rect.top / tourDistance) : 0;
    tourTrack.style.setProperty("--tx", (p * tourDistance).toFixed(1));
  } else if (tourViewport) {
    const max = tourViewport.scrollWidth - tourViewport.clientWidth;
    p = max > 0 ? tourViewport.scrollLeft / max : 0;
  }
  tour.style.setProperty("--tp", (p ?? 0).toFixed(3));
}

tourViewport?.addEventListener("scroll", queueScroll, { passive: true });

/* Lightbox for screenshots ------------------------------------------------ */
const lightbox = document.querySelector("#lightbox");
const lightboxImage = document.querySelector("#lightboxImage");
const lightboxCaption = document.querySelector("#lightboxCaption");

document.addEventListener("click", (event) => {
  const trigger = event.target.closest("[data-zoom]");
  if (trigger && lightbox?.showModal) {
    const image = [...trigger.querySelectorAll("img")].find(
      (img) => img.offsetParent !== null,
    );
    if (!image || !lightboxImage) return;
    lightboxImage.src = image.currentSrc || image.src;
    lightboxImage.alt = image.alt;
    if (lightboxCaption) {
      const caption = trigger.closest(".tour-card")?.querySelector("p");
      const name = caption?.querySelector("strong")?.textContent ?? "";
      const detail = (caption?.textContent ?? "").slice(name.length).trim();
      lightboxCaption.textContent = `${name ? `${name}: ` : ""}${detail} Preview with sample data.`;
    }
    lightbox.showModal();
    return;
  }
  // Clicking the dimmed backdrop closes the dialog.
  if (event.target === lightbox) lightbox.close();
});

/* Pointer spotlight and magnetic button ----------------------------------- */
document.addEventListener(
  "pointermove",
  (event) => {
    const card = event.target.closest?.(
      ".card, .vibe-card, .vibe-aside, .sequence",
    );
    if (card) {
      const rect = card.getBoundingClientRect();
      card.style.setProperty("--mx", `${event.clientX - rect.left}px`);
      card.style.setProperty("--my", `${event.clientY - rect.top}px`);
    }
  },
  { passive: true },
);

document.querySelectorAll(".magnetic").forEach((button) => {
  button.addEventListener("pointermove", (event) => {
    if (reduceMotion || event.pointerType !== "mouse") return;
    const rect = button.getBoundingClientRect();
    const x = (event.clientX - rect.left - rect.width / 2) * 0.18;
    const y = (event.clientY - rect.top - rect.height / 2) * 0.28;
    button.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px)`;
  });
  button.addEventListener("pointerleave", () => {
    button.style.transform = "";
  });
});

/* Count-up figures --------------------------------------------------------- */
const countObserver = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      countObserver.unobserve(entry.target);
      const node = entry.target;
      const target = Number(node.dataset.countTo);
      if (reduceMotion || !Number.isFinite(target) || target < 2) continue;
      const start = performance.now();
      const frame = (now) => {
        const t = clamp((now - start) / 900);
        node.textContent = String(
          Math.round(target * (1 - Math.pow(1 - t, 3))),
        );
        if (t < 1) requestAnimationFrame(frame);
      };
      requestAnimationFrame(frame);
    }
  },
  { threshold: 0.6 },
);
document
  .querySelectorAll("[data-count-to]")
  .forEach((node) => countObserver.observe(node));

/* Live monitor demo (simulated, never reads this computer) ---------------- */
const liveDemo = document.querySelector("[data-live-demo]");
const LIVE_POINTS = 61;

const RESOURCES = {
  cpu: {
    title: "CPU",
    sub: "8 cores · 16 logical processors",
    max: "100%",
    next(t) {
      const spike = t % 29 > 24 ? 24 : 0;
      return clamp(
        (24 +
          9 * Math.sin(t / 3.7) +
          6 * Math.sin(t / 1.3) +
          spike +
          Math.random() * 6) /
          100,
      );
    },
    second: (v) => v * 0.26,
    rail: (v) => `${Math.round(v * 100)}% · ${(4.48 + v * 0.3).toFixed(2)} GHz`,
    stats: (v, t) => [
      ["Utilization", `${Math.round(v * 100)}%`],
      ["Speed", `${(4.48 + v * 0.3).toFixed(2)} GHz`],
      ["Processes", String(285 + Math.round(Math.sin(t / 9) * 3))],
      ["Up time", uptime(11052 + t)],
    ],
  },
  memory: {
    title: "Memory",
    sub: "32 GB DDR5 · 6000 MT/s",
    max: "32 GB",
    next: (t) => clamp(0.57 + 0.025 * Math.sin(t / 11) + Math.random() * 0.006),
    rail: (v) => `${(v * 32).toFixed(1)} / 32 GB (${Math.round(v * 100)}%)`,
    stats: (v) => [
      ["In use", `${(v * 32).toFixed(1)} GB`],
      ["Available", `${(32 - v * 32).toFixed(1)} GB`],
      ["Committed", "24.1 / 37 GB"],
      ["Cached", "9.8 GB"],
    ],
  },
  disk: {
    title: "Disk 0 (C:)",
    sub: "NVMe SSD · 1 TB",
    max: "100%",
    next(t) {
      const burst = t % 17 < 3 ? 38 : 0;
      return clamp(
        (7 + 6 * Math.sin(t / 2.3) + burst + Math.random() * 7) / 100,
      );
    },
    second: (v) => v * 0.45,
    rail: (v) => `${Math.round(v * 100)}% active`,
    stats: (v) => [
      ["Active time", `${Math.round(v * 100)}%`],
      ["Read", `${(v * 410).toFixed(0)} MB/s`],
      ["Write", `${(v * 120).toFixed(0)} MB/s`],
      ["Response", `${(0.3 + v * 1.8).toFixed(1)} ms`],
    ],
  },
  network: {
    title: "Wi-Fi",
    sub: "Wi-Fi 6E · 1.2 Gbps link",
    max: "100 Mbps",
    next: (t) =>
      clamp(
        (32 +
          22 * Math.sin(t / 4.1) +
          14 * Math.sin(t / 1.7) +
          Math.random() * 10) /
          100,
      ),
    second: (v) => v * 0.08,
    rail: (v) => `R ${Math.round(v * 100)} Mbps · S ${Math.round(v * 8)} Mbps`,
    stats: (v) => [
      ["Receive", `${Math.round(v * 100)} Mbps`],
      ["Send", `${Math.round(v * 8)} Mbps`],
      ["Signal", "Excellent"],
      ["Adapter", "Intel AX211"],
    ],
  },
  gpu: {
    title: "GPU 0",
    sub: "Discrete graphics · 12 GB",
    max: "100%",
    next: (t) =>
      clamp(
        (36 +
          24 * Math.sin(t / 5.3) +
          8 * Math.sin(t / 1.9) +
          Math.random() * 5) /
          100,
      ),
    second: (v) => clamp(v * 0.5 + 0.08),
    rail: (v) => `${Math.round(v * 100)}% · 3D`,
    stats: (v) => [
      ["3D", `${Math.round(v * 100)}%`],
      ["Video encode", `${Math.round(clamp(v * 0.5 + 0.08) * 100)}%`],
      ["Dedicated", `${(6.8 + v * 1.6).toFixed(1)} / 12 GB`],
      ["Temperature", `${Math.round(48 + v * 22)} °C`],
    ],
  },
};

function uptime(seconds) {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function seriesPath(values, width, height, offset = 0, area = false) {
  const step = width / (LIVE_POINTS - 2);
  let d = "";
  values.forEach((value, i) => {
    const x = (i - offset) * step;
    const y = height - value * height * 0.96;
    d += `${i === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
  });
  if (area)
    d += `L${((values.length - 1 - offset) * step).toFixed(1)} ${height}L${(-offset * step).toFixed(1)} ${height}Z`;
  return d;
}

if (liveDemo) {
  const history = {};
  let t = 0;
  for (const [key, res] of Object.entries(RESOURCES)) {
    history[key] = [];
    for (let i = 0; i < LIVE_POINTS; i += 1)
      history[key].push(res.next(i - LIVE_POINTS));
  }
  let active = "cpu";
  let paused = false;
  let visible = false;
  let lastTick = performance.now();
  let rafId = 0;

  const title = liveDemo.querySelector("[data-live-title]");
  const sub = liveDemo.querySelector("[data-live-sub]");
  const max = liveDemo.querySelector("[data-live-max]");
  const stats = liveDemo.querySelector("[data-live-stats]");
  const line = liveDemo.querySelector(".live-line");
  const line2 = liveDemo.querySelector(".live-line-2");
  const areaPath = liveDemo.querySelector(".live-area");
  const pauseButton = liveDemo.querySelector("[data-live-pause]");
  const pauseLabel = liveDemo.querySelector("[data-live-pause-label]");
  const tabs = [...liveDemo.querySelectorAll(".live-res")];

  const renderStats = () => {
    const values = history[active];
    const value = values[values.length - 1];
    if (stats) {
      stats.innerHTML = RESOURCES[active]
        .stats(value, t)
        .map(([label, text]) => `<div><dt>${label}</dt><dd>${text}</dd></div>`)
        .join("");
    }
    for (const [key, res] of Object.entries(RESOURCES)) {
      const node = liveDemo.querySelector(`[data-res-value="${key}"]`);
      const series = history[key];
      if (node) node.textContent = res.rail(series[series.length - 1]);
    }
  };

  const renderSparks = () => {
    for (const tab of tabs) {
      const series = history[tab.dataset.res].slice(-30);
      const step = 72 / (series.length - 1);
      let d = "";
      series.forEach((value, i) => {
        d += `${i === 0 ? "M" : "L"}${(i * step).toFixed(1)} ${(30 - value * 28).toFixed(1)}`;
      });
      tab.querySelector(".spark-line")?.setAttribute("d", d);
      tab.querySelector(".spark-area")?.setAttribute("d", `${d}L72 30L0 30Z`);
    }
  };

  const renderChart = (offset) => {
    const values = history[active];
    line?.setAttribute("d", seriesPath(values, 600, 200, offset));
    areaPath?.setAttribute("d", seriesPath(values, 600, 200, offset, true));
    const second = RESOURCES[active].second;
    line2?.setAttribute(
      "d",
      second ? seriesPath(values.map(second), 600, 200, offset) : "",
    );
  };

  const select = (key) => {
    active = key;
    liveDemo.dataset.res = key;
    tabs.forEach((tab) =>
      tab.setAttribute("aria-selected", String(tab.dataset.res === key)),
    );
    if (title) title.textContent = RESOURCES[key].title;
    if (sub) sub.textContent = RESOURCES[key].sub;
    if (max) max.textContent = RESOURCES[key].max;
    renderStats();
    renderChart(0);
  };

  const advance = () => {
    t += 1;
    for (const [key, res] of Object.entries(RESOURCES)) {
      history[key].push(res.next(t));
      if (history[key].length > LIVE_POINTS) history[key].shift();
    }
    renderStats();
    renderSparks();
  };

  const loop = (now) => {
    rafId = 0;
    if (!visible || paused) return;
    let elapsed = now - lastTick;
    if (elapsed >= 1000) {
      advance();
      lastTick = now;
      elapsed = 0;
    }
    // Smoothly slide the chart toward the next sample between ticks.
    renderChart(reduceMotion ? 0 : clamp(elapsed / 1000));
    if (reduceMotion) {
      window.setTimeout(() => requestFrame(), 1000);
      return;
    }
    requestFrame();
  };

  const requestFrame = () => {
    if (!rafId) rafId = requestAnimationFrame(loop);
  };

  tabs.forEach((tab, index) => {
    tab.addEventListener("click", () => select(tab.dataset.res));
    tab.addEventListener("keydown", (event) => {
      if (
        !["ArrowDown", "ArrowRight", "ArrowUp", "ArrowLeft"].includes(event.key)
      )
        return;
      event.preventDefault();
      const delta =
        event.key === "ArrowDown" || event.key === "ArrowRight" ? 1 : -1;
      const next = tabs[(index + delta + tabs.length) % tabs.length];
      next.focus();
      select(next.dataset.res);
    });
  });

  pauseButton?.addEventListener("click", () => {
    paused = !paused;
    pauseButton.setAttribute("aria-pressed", String(paused));
    if (pauseLabel) pauseLabel.textContent = paused ? "Resume" : "Pause";
    if (!paused) {
      lastTick = performance.now();
      requestFrame();
    }
  });

  new IntersectionObserver(
    (entries) => {
      visible = entries.some((entry) => entry.isIntersecting);
      if (visible) {
        lastTick = performance.now();
        requestFrame();
      }
    },
    { threshold: 0.1 },
  ).observe(liveDemo);

  select("cpu");
  renderSparks();
}

/* Dev-only worst-case toggle --------------------------------------------- */
const isDev =
  ["localhost", "127.0.0.1", "::1", ""].includes(window.location.hostname) ||
  window.location.protocol === "file:";
const params = new URLSearchParams(window.location.search);
let stressMode = isDev && params.get("data") === "worst";
const stressToggle = document.querySelector("#stressToggle");

function applyStressMode(next, updateUrl = true) {
  stressMode = isDev && next;
  const benchmarks = stressMode ? STRESS_BENCHMARKS : BENCHMARKS;
  const comparison = stressMode ? STRESS_COMPARISON : COMPARISON;
  const tools = stressMode ? STRESS_TOOLS : TOOLS;

  bigPlayed = false;
  renderBenchHero(
    stressMode ? STRESS_BENCHMARKS[0] : BENCHMARKS[0],
    stressMode,
  );

  renderBenchmarks(benchmarks);
  renderComparison(comparison, tools);
  root.dataset.stress = stressMode ? "true" : "false";

  stressToggle?.querySelectorAll("button").forEach((button) => {
    button.setAttribute(
      "aria-pressed",
      String(button.dataset.mode === (stressMode ? "stress" : "demo")),
    );
  });

  if (updateUrl) {
    const url = new URL(window.location.href);
    if (stressMode) url.searchParams.set("data", "worst");
    else url.searchParams.delete("data");
    history.replaceState(null, "", url);
  }

  observeBigNumber();
}

if (isDev && stressToggle) {
  stressToggle.hidden = false;
  stressToggle.addEventListener("click", (event) => {
    const button = event.target.closest("button[data-mode]");
    if (!button) return;
    applyStressMode(button.dataset.mode === "stress");
  });
}

/* Init -------------------------------------------------------------------- */
applyRelease();
renderBenchHero(BENCHMARKS[0]);
renderBenchmarks(BENCHMARKS);
renderComparison(COMPARISON, TOOLS);
applyPreferences();
if (stressMode) applyStressMode(true, false);
updateScroll();

window.addEventListener("scroll", queueScroll, { passive: true });
window.addEventListener(
  "resize",
  () => {
    layoutTour();
    queueScroll();
  },
  { passive: true },
);
window.addEventListener(
  "load",
  () => {
    layoutTour();
    queueScroll();
  },
  { once: true },
);

systemDark.addEventListener("change", () => {
  if (themeChoice === "system") applyPreferences();
});

systemReduce.addEventListener("change", () => {
  if (motionChoice === "system") applyPreferences();
});
