import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type ThemePreference = "system" | "light" | "dark";

export interface AppSettings {
  theme: ThemePreference;
  includeHidden: boolean;
  exclusions: string[];
}

const STORAGE_KEY = "cyron.settings.v1";

const DEFAULTS: AppSettings = {
  theme: "system",
  includeHidden: true,
  exclusions: [],
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
  const reset = useCallback(() => setSettings(DEFAULTS), []);

  const value = useMemo(
    () => ({ settings, setTheme, setIncludeHidden, setExclusions, reset }),
    [settings, setTheme, setIncludeHidden, setExclusions, reset],
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) throw new Error("useSettings must be used inside SettingsProvider");
  return context;
}
