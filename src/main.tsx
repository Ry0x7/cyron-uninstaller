import { createRoot } from "react-dom/client";
import App from "./App";
import { SettingsProvider } from "./lib/settings";
import "./styles/app.css";

// Paint the persisted theme before React mounts so the window never flashes
// the wrong palette. SettingsProvider stays the source of truth afterwards.
function applyInitialTheme(): void {
  let preference: string = "system";
  try {
    const raw = window.localStorage.getItem("cyron.settings.v1");
    if (raw) {
      const parsed: unknown = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && "theme" in parsed) {
        const stored = (parsed as { theme?: unknown }).theme;
        if (stored === "light" || stored === "dark") preference = stored;
      }
    }
  } catch {
    // A broken or unavailable store falls back to the system preference.
  }
  const resolved =
    preference === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : preference;
  document.documentElement.dataset.theme = resolved;
}

applyInitialTheme();

const container = document.getElementById("root");
if (!container) throw new Error("Cyron root element is missing");

// StrictMode double-invokes effects, which would run disk scans twice in dev.
createRoot(container).render(
  <SettingsProvider>
    <App />
  </SettingsProvider>,
);
