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
let reduceMotion = motionChoice === "system" ? systemReduce.matches : motionChoice === "reduce";

function isDark() {
  return themeChoice === "system" ? systemDark.matches : themeChoice === "dark";
}

function isReduced() {
  return motionChoice === "system" ? systemReduce.matches : motionChoice === "reduce";
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
    if (button.dataset.pref === "theme") button.setAttribute("aria-pressed", String(dark));
    if (button.dataset.pref === "motion") button.setAttribute("aria-pressed", String(reduced));
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
  if (benchKicker) benchKicker.textContent = stress ? breakablePath(row.label) : row.full;
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
    const peaks = BENCHMARKS.map((entry) => Number.parseFloat(entry.peak)).filter(Number.isFinite);
    benchPeaks.textContent =
      stress || !peaks.length ? "—" : `${Math.min(...peaks)}–${Math.max(...peaks)} MiB`;
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
let scrollQueued = false;

function updateScroll() {
  scrollQueued = false;
  const scrollTop = window.scrollY || document.documentElement.scrollTop;
  const max = document.documentElement.scrollHeight - window.innerHeight;
  if (progressBar) progressBar.style.width = `${max > 0 ? clamp(scrollTop / max) * 100 : 0}%`;
  if (nav) nav.classList.toggle("is-scrolled", scrollTop > 12);

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
const showcaseTitle = document.querySelector("#showcaseTitle");
const showcaseLive = document.querySelector("#showcaseFrame .frame-live");
const stepTitles = ["Applications", "Disk space", "Caches", "Performance"];
const stepLive = ["Review", "Explore", "Review", "Read-only"];

function activateStep(index) {
  steps.forEach((step, i) => step.classList.toggle("is-active", i === index));
  panels.forEach((panel, i) => panel.classList.toggle("is-active", i === index));
  if (showcaseTitle) showcaseTitle.textContent = stepTitles[index] ?? "Cyron";
  if (showcaseLive) showcaseLive.innerHTML = `<i></i>${stepLive[index] ?? "Review"}`;
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
  renderBenchHero(stressMode ? STRESS_BENCHMARKS[0] : BENCHMARKS[0], stressMode);

  renderBenchmarks(benchmarks);
  renderComparison(comparison, tools);
  root.dataset.stress = stressMode ? "true" : "false";

  stressToggle?.querySelectorAll("button").forEach((button) => {
    button.setAttribute("aria-pressed", String(button.dataset.mode === (stressMode ? "stress" : "demo")));
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
window.addEventListener("resize", queueScroll, { passive: true });
window.addEventListener("load", queueScroll, { once: true });

systemDark.addEventListener("change", () => {
  if (themeChoice === "system") applyPreferences();
});

systemReduce.addEventListener("change", () => {
  if (motionChoice === "system") applyPreferences();
});
