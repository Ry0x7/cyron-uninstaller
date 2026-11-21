import { memo, useCallback, useEffect, useMemo, useState, type ComponentType, type ReactNode } from "react";
import { api } from "./lib/api";
import { IconButton, cx } from "./components/ui";
import {
  IconApps,
  IconGauge,
  IconHistory,
  IconHome,
  IconPanel,
  IconSettings,
  IconStorage,
  type IconProps,
} from "./components/Icons";
import { HistoryRecovery } from "./features/history/HistoryRecovery";
import { HunterMode } from "./features/hunter/HunterMode";
import { HunterConfirmationModal } from "./features/hunter/HunterConfirmationModal";
import { ForceDeleteModal } from "./features/explorer/ForceDeleteModal";
import { FindLocksModal } from "./features/explorer/FindLocksModal";
import { KillProcessModal } from "./features/explorer/KillProcessModal";
import { Overview } from "./features/overview/Overview";
import { Performance } from "./features/performance/Performance";
import { Settings } from "./features/settings/Settings";
import { StorageCleanup } from "./features/storage/StorageCleanup";
import { Uninstaller } from "./features/uninstaller/Uninstaller";
import { loadInstalledApps } from "./lib/api";
import type { InstalledApp, StartupRoute, ViewId } from "./lib/types";

const COLLAPSE_KEY = "cyron.sidebar.collapsed";

type Destination = "home" | "applications" | "storage" | "performance" | "history" | "settings";
type NavDestination = Exclude<Destination, "settings">;

interface NavEntry {
  id: NavDestination;
  label: string;
  icon: ComponentType<IconProps>;
}

const PRIMARY_NAV: NavEntry[] = [
  { id: "home", label: "Home", icon: IconHome },
  { id: "applications", label: "Applications", icon: IconApps },
  { id: "storage", label: "Storage & Cleanup", icon: IconStorage },
  { id: "performance", label: "Performance", icon: IconGauge },
  { id: "history", label: "History & Recovery", icon: IconHistory },
];

const TITLES: Record<Destination, string> = {
  home: "Home",
  applications: "Applications",
  storage: "Storage & Cleanup",
  performance: "Performance",
  history: "History & Recovery",
  settings: "Settings",
};

function destinationForView(view: ViewId): Destination {
  switch (view) {
    case "overview":
      return "home";
    case "uninstaller":
      return "applications";
    case "cleaner":
    case "analyzer":
      return "storage";
    case "performance":
      return "performance";
    case "activity":
      return "history";
    case "settings":
      return "settings";
  }
}

function defaultView(destination: Destination): ViewId {
  switch (destination) {
    case "home":
      return "overview";
    case "applications":
      return "uninstaller";
    case "storage":
      return "cleaner";
    case "performance":
      return "performance";
    case "history":
      return "activity";
    case "settings":
      return "settings";
  }
}

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
        strokeWidth="2.4"
        strokeLinecap="round"
      />
      <circle cx="12" cy="12" r="2.2" fill="currentColor" />
    </svg>
  );
}

function Sidebar({
  destination,
  collapsed,
  onNavigate,
  onToggleCollapsed,
}: {
  destination: Destination;
  collapsed: boolean;
  onNavigate: (destination: Destination) => void;
  onToggleCollapsed: () => void;
}) {
  return (
    <aside className="sidebar" aria-label="Cyron sections">
      <button
        type="button"
        className="sidebar__brand"
        onClick={() => onNavigate("home")}
        aria-label="Cyron home"
      >
        <span className="brand-mark">
          <BrandMark />
        </span>
        <span className="brand-text">
          <span className="brand-name">Cyron</span>
        </span>
      </button>

      <nav className="sidebar__nav" aria-label="Primary">
        {PRIMARY_NAV.map((entry) => {
          const Icon = entry.icon;
          const active = destination === entry.id;
          return (
            <button
              key={entry.id}
              type="button"
              className={cx("nav__item", active && "nav__item--active")}
              aria-current={active ? "page" : undefined}
              title={collapsed ? entry.label : undefined}
              onClick={() => onNavigate(entry.id)}
            >
              <span className="nav__icon">
                <Icon size={18} />
              </span>
              <span className="nav__label">{entry.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar__foot">
        <button
          type="button"
          className={cx("nav__item", destination === "settings" && "nav__item--active")}
          aria-current={destination === "settings" ? "page" : undefined}
          title={collapsed ? "Settings" : undefined}
          onClick={() => onNavigate("settings")}
        >
          <span className="nav__icon">
            <IconSettings size={18} />
          </span>
          <span className="nav__label">Settings</span>
        </button>

        <div className="sidebar__tools">
          <span className="sidebar__version">v0.2.1</span>
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
  id: Destination;
  active: boolean;
  label: string;
  children: ReactNode;
}

const ViewSlot = memo(function ViewSlot({ active, label, children }: ViewSlotProps) {
  return (
    <section className="view-slot" hidden={!active} aria-label={label}>
      {children}
    </section>
  );
});

export default function App() {
  const [view, setView] = useState<ViewId>("overview");
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const [visited, setVisited] = useState<Destination[]>(["home"]);
  const [startupRoute, setStartupRoute] = useState<StartupRoute | null>(null);
  const [cleanerTargetId, setCleanerTargetId] = useState<string | null>(null);
  const [recoveryRequest, setRecoveryRequest] = useState(0);
  const [apps, setApps] = useState<InstalledApp[]>([]);

  const [isHunterActive, setIsHunterActive] = useState(false);
  const [hunterConfirmOpen, setHunterConfirmOpen] = useState(false);

  const [forceDeleteTarget, setForceDeleteTarget] = useState<string | null>(null);
  const [findLocksTarget, setFindLocksTarget] = useState<string | null>(null);
  const [killProcessTarget, setKillProcessTarget] = useState<string | null>(null);

  useEffect(() => {
    loadInstalledApps()
      .then((inventory) => {
        if (inventory?.apps) setApps(inventory.apps);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const destination = destinationForView(view);
    document.title = destination === "home" ? "Cyron" : `${TITLES[destination]} · Cyron`;
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
    const destination = destinationForView(next);
    setVisited((current) => (current.includes(destination) ? current : [...current, destination]));
  }, []);

  const navigateDestination = useCallback(
    (destination: Destination) => {
      if (destination === "storage") {
        navigate(view === "analyzer" ? "analyzer" : "cleaner");
        return;
      }
      if (destination === "settings") {
        navigate("settings");
        return;
      }
      navigate(defaultView(destination));
    },
    [navigate, view],
  );

  useEffect(() => {
    api
      .getStartupRoute()
      .then((route) => {
        if (route) {
          setStartupRoute(route);
          if (route.action === "force-delete" && route.path) {
            setForceDeleteTarget(route.path);
          } else if (route.action === "find-locks" && route.path) {
            setFindLocksTarget(route.path);
          } else if (route.action === "kill-process" && route.path) {
            setKillProcessTarget(route.path);
          } else {
            navigate(route.view as ViewId);
          }
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
  const uninstallerInitialAppId =
    startupRoute?.view === "uninstaller" ? startupRoute.targetAppId : undefined;
  const uninstallerInitialQuery =
    startupRoute?.view === "uninstaller" ? startupRoute.path : undefined;
  const uninstallerView = useMemo(
    () => (
      <Uninstaller
        initialAppId={uninstallerInitialAppId}
        initialQuery={uninstallerInitialQuery}
        onFindWindow={() => setHunterConfirmOpen(true)}
      />
    ),
    [uninstallerInitialAppId, uninstallerInitialQuery],
  );

  const analyzerInitialPath = startupRoute?.view === "analyzer" ? startupRoute.path : undefined;
  const analyzerInitialMode =
    startupRoute?.view === "analyzer" &&
    (startupRoute.subview === "map" ||
      startupRoute.subview === "duplicates" ||
      startupRoute.subview === "storage")
      ? startupRoute.subview
      : undefined;

  const storageView = useMemo(
    () => (
      <StorageCleanup
        mode={view === "analyzer" ? "disk" : "cleanup"}
        onModeChange={(mode) => navigate(mode === "disk" ? "analyzer" : "cleaner")}
        initialAnalyzerPath={analyzerInitialPath ?? undefined}
        initialAnalyzerMode={analyzerInitialMode}
        cleanerTargetId={cleanerTargetId ?? undefined}
        onNavigateToCleaner={navigateToCleanerWithTarget}
        onOpenRecovery={() => {
          setRecoveryRequest((value) => value + 1);
          navigate("activity");
        }}
      />
    ),
    [
      view,
      navigate,
      analyzerInitialPath,
      analyzerInitialMode,
      cleanerTargetId,
      navigateToCleanerWithTarget,
    ],
  );

  const performanceView = useMemo(() => <Performance onNavigate={navigate} />, [navigate]);
  const historyView = useMemo(
    () => <HistoryRecovery onNavigate={navigate} recoveryRequest={recoveryRequest} />,
    [navigate, recoveryRequest],
  );
  const settingsView = useMemo(() => <Settings />, []);

  const activeDestination = destinationForView(view);

  const getViewElement = (destination: Destination): ReactNode => {
    switch (destination) {
      case "home":
        return overviewView;
      case "applications":
        return uninstallerView;
      case "storage":
        return storageView;
      case "performance":
        return performanceView;
      case "history":
        return historyView;
      case "settings":
        return settingsView;
    }
  };

  return (
    <div className="app" data-collapsed={collapsed}>
      <Sidebar
        destination={activeDestination}
        collapsed={collapsed}
        onNavigate={navigateDestination}
        onToggleCollapsed={() => setCollapsed((value) => !value)}
      />
      <main className="main" id="main-content" aria-label="Cyron content">
        {visited.map((destination) => (
          <ViewSlot
            key={destination}
            id={destination}
            active={activeDestination === destination}
            label={TITLES[destination]}
          >
            {getViewElement(destination)}
          </ViewSlot>
        ))}
      </main>

      <HunterMode
        isActive={isHunterActive}
        onExit={() => setIsHunterActive(false)}
        installedApps={apps}
      />

      <HunterConfirmationModal
        open={hunterConfirmOpen}
        onConfirm={() => {
          setHunterConfirmOpen(false);
          setIsHunterActive(true);
        }}
        onCancel={() => setHunterConfirmOpen(false)}
      />

      <ForceDeleteModal
        open={Boolean(forceDeleteTarget)}
        targetPath={forceDeleteTarget}
        onClose={() => setForceDeleteTarget(null)}
      />

      <FindLocksModal
        open={Boolean(findLocksTarget)}
        targetPath={findLocksTarget}
        onClose={() => setFindLocksTarget(null)}
      />

      <KillProcessModal
        open={Boolean(killProcessTarget)}
        exePath={killProcessTarget}
        onClose={() => setKillProcessTarget(null)}
      />
    </div>
  );
}
