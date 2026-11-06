import { useCallback, useEffect, useState, type ComponentType, type ReactNode } from "react";
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
import { Overview } from "./features/overview/Overview";
import { Settings } from "./features/settings/Settings";
import { Uninstaller } from "./features/uninstaller/Uninstaller";
import { useSettings, type ThemePreference } from "./lib/settings";
import type { ViewId } from "./lib/types";

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
          <span className="brand-sub">Read-only analysis</span>
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
            <strong>No deletions</strong>
Read-only scans. Files stay untouched.
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

export default function App() {
  const [view, setView] = useState<ViewId>("overview");
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [visited, setVisited] = useState<ViewId[]>(["overview"]);

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

  const renderView = (id: ViewId): ReactNode => {
    switch (id) {
      case "overview":
        return <Overview onNavigate={navigate} />;
      case "uninstaller":
        return <Uninstaller />;
      case "analyzer":
        return <Analyzer />;
      case "cleaner":
        return <Cleaner />;
      case "activity":
        return <Activity />;
      case "settings":
        return <Settings />;
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
          <section
            key={id}
            className="view-slot"
            hidden={view !== id}
            aria-label={viewLabel(id)}
          >
            {renderView(id)}
          </section>
        ))}
      </main>
    </div>
  );
}
