import { memo, useCallback, useEffect, useMemo, useState, type ComponentType, type ReactNode } from "react";
import { api } from "./lib/api";
import { IconButton } from "./components/ui";
import {
  IconActivity,
  IconAnalyzer,
  IconApps,
  IconCleaner,
  IconMonitor,
  IconMoon,
  IconOverview,
  IconPanel,
  IconSettings,
  IconShield,
  IconSun,
  type IconProps,
} from "./components/Icons";
import { Activity } from "./features/activity/Activity";
import { Analyzer } from "./features/analyzer/Analyzer";
import { Cleaner } from "./features/cleaner/Cleaner";
import { HunterMode } from "./features/hunter/HunterMode";
import { Overview } from "./features/overview/Overview";
import { Settings } from "./features/settings/Settings";
import { Uninstaller } from "./features/uninstaller/Uninstaller";
import { loadInstalledApps } from "./lib/api";
import { useSettings, type ThemePreference } from "./lib/settings";
import type { InstalledApp, StartupRoute, ViewId } from "./lib/types";

const COLLAPSE_KEY = "cyron.sidebar.collapsed";

interface NavEntry {
  id: ViewId;
  label: string;
  icon: ComponentType<IconProps>;
}

const PRIMARY_NAV: NavEntry[] = [
  { id: "overview", label: "Overview", icon: IconOverview },
  { id: "uninstaller", label: "Uninstaller", icon: IconApps },
  { id: "analyzer", label: "Disk Analyzer", icon: IconAnalyzer },
  { id: "cleaner", label: "Cleaner", icon: IconCleaner },
  { id: "activity", label: "Activity", icon: IconActivity },
];

function viewLabel(view: ViewId): string {
  return PRIMARY_NAV.find((entry) => entry.id === view)?.label ?? "Settings";
}

const THEMES: { value: ThemePreference; label: string; icon: ComponentType<IconProps> }[] = [
  { value: "system", label: "System theme", icon: IconMonitor },
  { value: "light", label: "Light theme", icon: IconSun },
  { value: "dark", label: "Dark theme", icon: IconMoon },
];

function readCollapsed(): boolean {
  try {
    return window.localStorage.getItem(COLLAPSE_KEY) === "1";
  } catch {
    return false;
  }
}

function BrandMark() {
  return (
    <svg width="17" height="17" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path
        d="M20.1 14.4A8.4 8.4 0 1 1 16.6 5"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.6"
        strokeLinecap="round"
      />
      <circle cx="12" cy="12" r="2.3" fill="currentColor" />
    </svg>
  );
}

function Sidebar({
  view,
  collapsed,
  onNavigate,
  onToggleCollapsed,
}: {
  view: ViewId;
  collapsed: boolean;
  onNavigate: (view: ViewId) => void;
  onToggleCollapsed: () => void;
}) {
  const { settings, setTheme } = useSettings();

  return (
    <aside className="sidebar" aria-label="Cyron sections">
      <button
        type="button"
        className="sidebar__brand"
        onClick={() => onNavigate("overview")}
        aria-label="Cyron overview"
        style={{
          background: "transparent",
          border: 0,
          textAlign: "left",
          width: "100%",
          cursor: "pointer",
        }}
      >
        <span className="brand-mark">
          <BrandMark />
        </span>
        <span className="brand-text">
          <span className="brand-name">Cyron</span>
          <span className="brand-sub">Review-first cleaning</span>
        </span>
      </button>

      <nav className="sidebar__nav" aria-label="Primary">
        {PRIMARY_NAV.map((entry) => {
          const Icon = entry.icon;
          const active = view === entry.id;
          return (
            <button
              key={entry.id}
              type="button"
              className="nav__item"
              aria-current={active ? "page" : undefined}
              title={entry.label}
              onClick={() => onNavigate(entry.id)}
            >
              <span className="nav__icon">
                <Icon size={17} />
              </span>
              <span className="nav__label">{entry.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar__foot">
        <div className="sidebar__note">
          <IconShield size={14} />
          <span>
            <strong>Review first</strong>
Preview, then 7-day quarantine.
          </span>
        </div>

        <button
          type="button"
          className="nav__item"
          aria-current={view === "settings" ? "page" : undefined}
          title="Settings"
          onClick={() => onNavigate("settings")}
        >
          <span className="nav__icon">
            <IconSettings size={17} />
          </span>
          <span className="nav__label">Settings</span>
        </button>

        <div className="sidebar__tools">
          <div className="theme-switch" role="group" aria-label="Theme">
            {THEMES.map((theme) => {
              const Icon = theme.icon;
              const active = settings.theme === theme.value;
              return (
                <button
                  key={theme.value}
                  type="button"
                  className="theme-switch__item"
                  aria-pressed={active}
                  aria-label={theme.label}
                  title={theme.label}
                  onClick={() => setTheme(theme.value)}
                >
                  <Icon size={13} />
                </button>
              );
            })}
          </div>
          <IconButton
            label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className="collapse-btn"
            onClick={onToggleCollapsed}
            aria-expanded={!collapsed}
          >
            <IconPanel size={16} />
          </IconButton>
        </div>
      </div>
    </aside>
  );
}

interface ViewSlotProps {
  id: ViewId;
  active: boolean;
  label: string;
  children: ReactNode;
}

const ViewSlot = memo(function ViewSlot({ active, label, children }: ViewSlotProps) {
  return (
    <section
      className="view-slot"
      hidden={!active}
      aria-label={label}
    >
      {children}
    </section>
  );
});

export default function App() {
  const { settings } = useSettings();
  const [view, setView] = useState<ViewId>("overview");
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [visited, setVisited] = useState<ViewId[]>(["overview"]);
  const [startupRoute, setStartupRoute] = useState<StartupRoute | null>(null);
  const [cleanerTargetId, setCleanerTargetId] = useState<string | null>(null);
  const [apps, setApps] = useState<InstalledApp[]>([]);

  useEffect(() => {
    loadInstalledApps().then((inv) => {
      if (inv?.apps) setApps(inv.apps);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    document.title = view === "overview" ? "Cyron" : `${viewLabel(view)} · Cyron`;
  }, [view]);

  useEffect(() => {
    try {
      window.localStorage.setItem(COLLAPSE_KEY, collapsed ? "1" : "0");
    } catch {
      // The session still works when storage is unavailable.
    }
  }, [collapsed]);

  const navigate = useCallback((next: ViewId) => {
    setView(next);
    setVisited((current) => (current.includes(next) ? current : [...current, next]));
  }, []);

  useEffect(() => {
    api.getStartupRoute()
      .then((route) => {
        if (route) {
          setStartupRoute(route);
          navigate(route.view);
        }
      })
      .catch(() => {});
  }, [navigate]);

  const navigateToCleanerWithTarget = useCallback(
    (targetId?: string) => {
      setCleanerTargetId(targetId ?? null);
      navigate("cleaner");
    },
    [navigate],
  );

  const overviewView = useMemo(() => <Overview onNavigate={navigate} />, [navigate]);
  const uninstallerInitialAppId = startupRoute?.view === "uninstaller" ? startupRoute.targetAppId : undefined;
  const uninstallerInitialQuery = startupRoute?.view === "uninstaller" ? startupRoute.path : undefined;
  const uninstallerView = useMemo(
    () => <Uninstaller initialAppId={uninstallerInitialAppId} initialQuery={uninstallerInitialQuery} />,
    [uninstallerInitialAppId, uninstallerInitialQuery],
  );
  const analyzerInitialPath = startupRoute?.view === "analyzer" ? startupRoute.path : undefined;
  const analyzerInitialMode =
    startupRoute?.view === "analyzer" &&
    (startupRoute.subview === "map" || startupRoute.subview === "duplicates" || startupRoute.subview === "storage")
      ? startupRoute.subview
      : undefined;
  const analyzerView = useMemo(
    () => (
      <Analyzer
        initialPath={analyzerInitialPath}
        initialMode={analyzerInitialMode}
        onNavigateToCleaner={navigateToCleanerWithTarget}
      />
    ),
    [analyzerInitialPath, analyzerInitialMode, navigateToCleanerWithTarget],
  );
  const cleanerView = useMemo(() => <Cleaner initialTargetId={cleanerTargetId} />, [cleanerTargetId]);
  const activityView = useMemo(() => <Activity />, []);
  const settingsView = useMemo(() => <Settings />, []);

  const getViewElement = (id: ViewId): ReactNode => {
    switch (id) {
      case "overview":
        return overviewView;
      case "uninstaller":
        return uninstallerView;
      case "analyzer":
        return analyzerView;
      case "cleaner":
        return cleanerView;
      case "activity":
        return activityView;
      case "settings":
        return settingsView;
    }
  };

  return (
    <div className="app" data-collapsed={collapsed}>
      <Sidebar
        view={view}
        collapsed={collapsed}
        onNavigate={navigate}
        onToggleCollapsed={() => setCollapsed((value) => !value)}
      />
      <main className="main" id="main-content" aria-label="Cyron content">
        {visited.map((id) => (
          <ViewSlot
            key={id}
            id={id}
            active={view === id}
            label={viewLabel(id)}
          >
            {getViewElement(id)}
          </ViewSlot>
        ))}
      </main>
      <HunterMode installedApps={apps} showFloatingTrigger={settings.showHunterWidget} />
    </div>
  );
}
