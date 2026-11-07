import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { api } from "./api";

export type ThemePreference = "system" | "light" | "dark";

export interface AppSettings {
  theme: ThemePreference;
  includeHidden: boolean;
  exclusions: string[];
  /** When true the backend refuses to execute any cleanup (the default). */
  readOnly: boolean;
  /** Whether to show the floating Hunter Mode crosshair widget on the desktop. */
  showHunterWidget: boolean;
}

const STORAGE_KEY = "cyron.settings.v1";

const DEFAULTS: AppSettings = {
  theme: "system",
  includeHidden: true,
  exclusions: [],
  readOnly: true,
  showHunterWidget: false,
};

function readSettings(): AppSettings {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return {
      theme:
        parsed.theme === "light" || parsed.theme === "dark" || parsed.theme === "system"
          ? parsed.theme
          : DEFAULTS.theme,
      includeHidden:
        typeof parsed.includeHidden === "boolean" ? parsed.includeHidden : DEFAULTS.includeHidden,
      exclusions: Array.isArray(parsed.exclusions)
        ? parsed.exclusions.filter((item): item is string => typeof item === "string").slice(0, 200)
        : [],
      // Settings saved before Phase 3 have no flag; they stay read-only.
      readOnly: typeof parsed.readOnly === "boolean" ? parsed.readOnly : DEFAULTS.readOnly,
      showHunterWidget:
        typeof parsed.showHunterWidget === "boolean"
          ? parsed.showHunterWidget
          : DEFAULTS.showHunterWidget,
    };
  } catch {
    return DEFAULTS;
  }
}

interface SettingsContextValue {
  settings: AppSettings;
  setTheme: (theme: ThemePreference) => void;
  setIncludeHidden: (include: boolean) => void;
  setExclusions: (paths: string[]) => void;
  /** Enable or disable cleaning; the backend enforces the same flag. */
  setReadOnly: (readOnly: boolean) => void;
  setShowHunterWidget: (show: boolean) => void;
  reset: () => void;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings>(readSettings);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {
      // Storage can be unavailable; the session keeps working in memory.
    }
  }, [settings]);

  // The backend owns the enforcement; this mirrors the persisted preference so
  // the two cannot disagree after a restart.
  useEffect(() => {
    api.setCleanupMode(settings.readOnly).catch(() => {
      // Browser previews and offline backends have no cleanup mode to set.
    });
  }, [settings.readOnly]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved =
        settings.theme === "system" ? (media.matches ? "dark" : "light") : settings.theme;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme]);

  const setTheme = useCallback(
    (theme: ThemePreference) => setSettings((current) => ({ ...current, theme })),
    [],
  );
  const setIncludeHidden = useCallback(
    (includeHidden: boolean) => setSettings((current) => ({ ...current, includeHidden })),
    [],
  );
  const setExclusions = useCallback(
    (exclusions: string[]) => setSettings((current) => ({ ...current, exclusions })),
    [],
  );
  const setReadOnly = useCallback(
    (readOnly: boolean) => setSettings((current) => ({ ...current, readOnly })),
    [],
  );
  const setShowHunterWidget = useCallback(
    (showHunterWidget: boolean) => setSettings((current) => ({ ...current, showHunterWidget })),
    [],
  );
  const reset = useCallback(() => setSettings(DEFAULTS), []);

  const value = useMemo(
    () => ({
      settings,
      setTheme,
      setIncludeHidden,
      setExclusions,
      setReadOnly,
      setShowHunterWidget,
      reset,
    }),
    [settings, setTheme, setIncludeHidden, setExclusions, setReadOnly, setShowHunterWidget, reset],
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) throw new Error("useSettings must be used inside SettingsProvider");
  return context;
}
