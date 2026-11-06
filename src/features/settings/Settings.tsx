import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { useSettings, type ThemePreference } from "../../lib/settings";
import type { ExplorerIntegrationStatus, StartupItem } from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  Field,
  KeyValue,
  Panel,
  Toggle,
  ViewHeader,
} from "../../components/ui";
import {
  IconCheck,
  IconCopy,
  IconExternal,
  IconLock,
  IconMonitor,
  IconMoon,
  IconRefresh,
  IconShield,
  IconSun,
  type IconProps,
} from "../../components/Icons";
import "./settings.css";

const THEME_OPTIONS: {
  value: ThemePreference;
  label: string;
  icon: (props: IconProps) => ReactNode;
}[] = [
  { value: "system", label: "System", icon: IconMonitor },
  { value: "light", label: "Light", icon: IconSun },
  { value: "dark", label: "Dark", icon: IconMoon },
];

const DRIVE_PATH = /^[a-zA-Z]:\\/;
const UNC_PATH = /^\\\\[^\\]+\\[^\\]+/;
const INVALID_PATH_CHARS = /[<>:"|?*\u0000-\u001f]/;
const MAX_EXCLUSIONS = 200;

interface ParsedExclusions {
  paths: string[];
  invalid: string[];
}

/** A path segment that navigates upward cannot be applied as a prefix. */
function hasTraversal(path: string): boolean {
  return path.split("\\").some((segment) => segment === "." || segment === "..");
}

/** Accepts one absolute Windows folder per line; comments and blanks are ignored. */
function parseExclusions(raw: string): ParsedExclusions {
  const paths: string[] = [];
  const invalid: string[] = [];
  const seen = new Set<string>();

  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const unquoted = trimmed.replace(/^"(.*)"$/, "$1").trim();
    const withBackslashes = unquoted.replace(/\//g, "\\");
    const normalized =
      withBackslashes.length > 3 ? withBackslashes.replace(/\\+$/, "") : withBackslashes;

    const withoutDrive = normalized.replace(/^[a-zA-Z]:/, "");
    const absolute = DRIVE_PATH.test(normalized) || UNC_PATH.test(normalized);
    if (
      !absolute ||
      hasTraversal(normalized) ||
      INVALID_PATH_CHARS.test(withoutDrive) ||
      paths.length >= MAX_EXCLUSIONS
    ) {
      invalid.push(trimmed);
      continue;
    }

    const key = normalized.toLowerCase();
    if (seen.has(key)) {
      invalid.push(trimmed);
      continue;
    }
    seen.add(key);
    paths.push(normalized);
  }

  return { paths, invalid };
}

export function Settings() {
  const { settings, setTheme, setIncludeHidden, setExclusions, setReadOnly, reset } =
    useSettings();

  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia("(prefers-color-scheme: dark)").matches,
  );
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  const [exclusionText, setExclusionText] = useState(settings.exclusions.join("\n"));
  const [resetDone, setResetDone] = useState(false);
  const lastSynced = useRef(settings.exclusions.join("\n"));
  const parsed = useMemo(() => parseExclusions(exclusionText), [exclusionText]);
  const savedExclusions = settings.exclusions.join("\n");
  const parsedText = parsed.paths.join("\n");
  const pending = parsedText !== savedExclusions;

  // Pull in external changes such as Reset to defaults.
  useEffect(() => {
    if (savedExclusions !== lastSynced.current) {
      lastSynced.current = savedExclusions;
      setExclusionText(savedExclusions);
    }
  }, [savedExclusions]);

  const [explorerStatus, setExplorerStatus] = useState<ExplorerIntegrationStatus | null>(null);
  const [explorerLoading, setExplorerLoading] = useState(false);
  const [explorerMsg, setExplorerMsg] = useState<{ tone: "safe" | "caution" | "danger"; text: string } | null>(null);

  const loadExplorerStatus = () => {
    api.getExplorerIntegrationStatus()
      .then(setExplorerStatus)
      .catch(() => {});
  };

  useEffect(() => {
    loadExplorerStatus();
  }, []);

  const handleToggleExplorer = async (enable: boolean) => {
    setExplorerLoading(true);
    setExplorerMsg(null);
    try {
      await api.setExplorerIntegration(enable);
      setExplorerMsg({
        tone: "safe",
        text: enable
          ? "Windows Explorer right-click integration registered in HKCU."
          : "Windows Explorer right-click integration cleanly unregistered.",
      });
      loadExplorerStatus();
    } catch (err) {
      setExplorerMsg({ tone: "danger", text: errorMessage(err) });
    } finally {
      setExplorerLoading(false);
    }
  };

  const handleRepairExplorer = async () => {
    setExplorerLoading(true);
    setExplorerMsg(null);
    try {
      await api.repairExplorerIntegration();
      setExplorerMsg({
        tone: "safe",
        text: "Windows Explorer integration repaired and pointed to the current binary path.",
      });
      loadExplorerStatus();
    } catch (err) {
      setExplorerMsg({ tone: "danger", text: errorMessage(err) });
    } finally {
      setExplorerLoading(false);
    }
  };

  // Persist the parsed list shortly after typing stops.
  useEffect(() => {
    const timer = window.setTimeout(() => {
      if (parsedText !== savedExclusions) {
        lastSynced.current = parsedText;
        setExclusions(parsed.paths);
      }
    }, 450);
    return () => window.clearTimeout(timer);
  }, [parsed, parsedText, savedExclusions, setExclusions]);

  // Blur commits immediately, so closing the window right after editing keeps
  // the exclusion list instead of dropping the pending debounce.
  const commitExclusions = () => {
    if (parsedText !== savedExclusions) {
      lastSynced.current = parsedText;
      setExclusions(parsed.paths);
    }
  };

  useEffect(() => {
    if (!resetDone) return;
    const timer = window.setTimeout(() => setResetDone(false), 3000);
    return () => window.clearTimeout(timer);
  }, [resetDone]);

  const handleReset = () => {
    reset();
    setExclusionText("");
    lastSynced.current = "";
    setResetDone(true);
  };

  const [startupItems, setStartupItems] = useState<StartupItem[]>([]);
  const [startupLoading, setStartupLoading] = useState(false);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [togglingId, setTogglingId] = useState<string | null>(null);

  const loadStartup = () => {
    setStartupLoading(true);
    setStartupError(null);
    api.listStartupItems()
      .then((items) => {
        setStartupItems(items);
        setStartupLoading(false);
      })
      .catch((err) => {
        setStartupError(errorMessage(err));
        setStartupLoading(false);
      });
  };

  const handleToggleStartup = async (item: StartupItem) => {
    if (item.isProtected || !item.canToggle) return;
    const targetState = !item.enabled;
    const actionLabel = targetState ? "enable" : "disable";
    if (!window.confirm(`Are you sure you want to ${actionLabel} "${item.name}" from starting automatically?`)) {
      return;
    }

    setTogglingId(item.id);
    setStartupError(null);
    try {
      const updated = await api.setStartupItemEnabled(item.id, targetState);
      setStartupItems((prev) =>
        prev.map((i) => (i.id === updated.id ? updated : i)),
      );
    } catch (err) {
      setStartupError(errorMessage(err));
    } finally {
      setTogglingId(null);
    }
  };

  useEffect(() => {
    loadStartup();
  }, []);

  const resolved = settings.theme === "system" ? (systemDark ? "dark" : "light") : settings.theme;
  const themeSource = settings.theme === "system" ? "Following system preference" : "Chosen manually";

  return (
    <>
      <ViewHeader
        title="Settings"
        description="Appearance, scan scope and safety. Preferences are stored locally in this Windows profile."
      />

      <div className="view-body">
        <div className="set-grid">
          <div className="set-col">
            <Panel title="Appearance" subtitle="Applies immediately to this window.">
              <div className="set-theme" role="group" aria-label="Theme">
                {THEME_OPTIONS.map((option) => {
                  const ThemeIcon = option.icon;
                  return (
                    <button
                      key={option.value}
                      type="button"
                      className="set-theme__item"
                      aria-pressed={settings.theme === option.value}
                      onClick={() => setTheme(option.value)}
                    >
                      <ThemeIcon size={16} />
                      <span>{option.label}</span>
                    </button>
                  );
                })}
              </div>
              <p className="set-theme__current">
                <span className="set-theme__dot" aria-hidden="true" />
                {resolved === "dark" ? "Dark" : "Light"} theme
                <span className="set-theme__source">· {themeSource}</span>
              </p>
            </Panel>

            <Panel
              title="Scanning"
              subtitle="These choices are applied the next time you start a disk scan."
            >
              <Toggle
                checked={settings.includeHidden}
                onChange={setIncludeHidden}
                label="Include hidden and system files"
                description="Hidden folders often hold the largest developer caches. Turning this off skips files and folders marked hidden or system, so the reported size can be lower than what the drive holds."
              />

              <hr className="hairline" />

              <Field
                label="Excluded folders"
                htmlFor="cyron-exclusions"
                hint="One absolute folder per line, up to 200. Folders at or below these paths are skipped the next time you start a scan. Wildcards and relative paths are rejected."
              >
                <textarea
                  id="cyron-exclusions"
                  className="textarea set-exclusions"
                  value={exclusionText}
                  spellCheck={false}
                  placeholder={"e.g. C:\\Users\\Me\\Projects\\large-repo\ne.g. D:\\Virtual Machines"}
                  onChange={(event) => setExclusionText(event.target.value)}
                  onBlur={commitExclusions}
                />
              </Field>

              <div className="set-exclusions__meta" aria-live="polite">
                <span>
                  {resetDone
                    ? "Defaults restored. Changes apply to the next scan."
                    : pending
                      ? "Saving exclusion list…"
                      : settings.exclusions.length === 0
                        ? "No folders excluded"
                        : `${settings.exclusions.length} ${
                            settings.exclusions.length === 1 ? "folder" : "folders"
                          } excluded · applies to the next scan`}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    // Commit immediately instead of waiting for the 450 ms debounce,
                    // so closing the window right after clicking cannot keep stale exclusions.
                    if (exclusionText.length === 0) return;
                    lastSynced.current = "";
                    setExclusionText("");
                    setExclusions([]);
                    setResetDone(false);
                  }}
                >
                  Clear all
                </Button>
              </div>

              {parsed.invalid.length > 0 && (
                <div className="set-invalid" role="status">
                  <strong>
                    {parsed.invalid.length === 1
                      ? "1 line was ignored"
                      : `${parsed.invalid.length} lines were ignored`}
                    :
                  </strong>{" "}
                  each path must be an absolute drive or UNC folder, without wildcards or
                  traversal segments (maximum {MAX_EXCLUSIONS}).
                  <ul className="set-invalid__list">
                    {parsed.invalid.slice(0, 4).map((line, index) => (
                      <li key={`${index}-${line}`}>
                        <code>{line}</code>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              <div className="set-actions">
                <p className="set-note">
                  Reset restores the system theme, hidden-file scanning, an empty exclusion list
                  and read-only cleaning.
                </p>
                <Button variant="secondary" size="sm" onClick={handleReset}>
                  Reset to defaults
                </Button>
              </div>
            </Panel>
          </div>

          <div className="set-col">
            <Panel title="Safety">
              <div className="set-status">
                <Badge tone={settings.readOnly ? "accent" : "caution"}>
                  <IconLock size={11} />
                  {settings.readOnly ? "Read-only" : "Cleaning enabled"}
                </Badge>
                <span className="set-note">
                  {settings.readOnly
                    ? "Cyron can measure and preview, but the backend refuses to move any file."
                    : "Cyron may move eligible cache files to quarantine after you confirm a preview."}
                </span>
              </div>

              <Toggle
                checked={!settings.readOnly}
                onChange={(enabled) => setReadOnly(!enabled)}
                label="Cleaning enabled"
                description="Off by default. Cyron still previews every cleanup and asks you to confirm. Eligible files move to a 7-day quarantine you can restore; protected data is refused by the backend, not merely hidden. Cyron never asks for administrator rights."
              />

              <hr className="hairline" />

              <h3 className="set-subhead">Cyron never touches</h3>
              <ul className="set-guards">
                {[
                  "Conversation histories and chat logs from AI tools",
                  "Local AI models, checkpoints and downloaded weights",
                  "Credentials, tokens and tool settings",
                  "Your source repositories and project folders",
                  "Docker volumes and WSL disks",
                  "Windows system and program files",
                ].map((item) => (
                  <li className="set-guard" key={item}>
                    <IconCheck size={14} />
                    <span>{item}</span>
                  </li>
                ))}
              </ul>

              <p className="set-note set-note--spaced">
                The Cleaner previews every cleanup before anything moves, quarantines what it does
                move, and refuses protected locations in the backend even if a request asks for
                them. Application uninstallation, process management, and residual cleanup are review-first
                and require server-verified confirmation plans.
              </p>
            </Panel>

            <Panel title="Windows Explorer Integration">
              <div className="set-status">
                {explorerStatus?.isRegistered && !explorerStatus?.needsRepair ? (
                  <Badge tone="safe">Registered in user shell (HKCU)</Badge>
                ) : explorerStatus?.needsRepair ? (
                  <Badge tone="caution">Needs repair / Path mismatch</Badge>
                ) : (
                  <Badge tone="neutral">Not registered</Badge>
                )}
              </div>

              <p className="set-note set-note--spaced">
                Optional native Windows right-click menu integration for folders, drives, and executables.
                Registered exclusively in <code>HKEY_CURRENT_USER\Software\Classes</code> without requiring
                administrator privileges.
              </p>

              <ul className="set-guards">
                <li className="set-guard">
                  <IconCheck size={14} />
                  <span><strong>Folders &amp; Drives:</strong> Analyze, Find Duplicates, View Storage Breakdown</span>
                </li>
                <li className="set-guard">
                  <IconCheck size={14} />
                  <span><strong>Executables:</strong> Review Uninstallation, Find Related Files</span>
                </li>
                <li className="set-guard">
                  <IconCheck size={14} />
                  <span><strong>Safe execution:</strong> Never triggers destructive actions directly from Explorer</span>
                </li>
              </ul>

              {explorerMsg && (
                <div style={{ marginTop: 12 }}>
                  <Banner tone={explorerMsg.tone}>{explorerMsg.text}</Banner>
                </div>
              )}

              <div className="set-actions" style={{ marginTop: 16 }}>
                {explorerStatus?.isRegistered ? (
                  <div className="row" style={{ gap: 8 }}>
                    <Button
                      variant="secondary"
                      size="sm"
                      loading={explorerLoading}
                      onClick={() => handleToggleExplorer(false)}
                    >
                      Disable integration
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      loading={explorerLoading}
                      onClick={handleRepairExplorer}
                    >
                      Repair / Re-register
                    </Button>
                  </div>
                ) : (
                  <Button
                    variant="primary"
                    size="sm"
                    loading={explorerLoading}
                    onClick={() => handleToggleExplorer(true)}
                  >
                    Enable Explorer integration
                  </Button>
                )}
              </div>
            </Panel>

            <Panel
              title="Startup Applications"
              subtitle="Programs registered to launch automatically when Windows boots."
              actions={
                <Button
                  size="sm"
                  variant="ghost"
                  icon={<IconRefresh size={13} />}
                  loading={startupLoading}
                  onClick={loadStartup}
                >
                  Refresh
                </Button>
              }
            >
              {startupError && (
                <Banner tone="caution">
                  {startupError}
                </Banner>
              )}
              {startupItems.length === 0 ? (
                <p className="set-note">
                  {startupLoading ? "Reading startup entries…" : "No startup applications detected."}
                </p>
              ) : (
                <div className="set-startup-list">
                  {startupItems.map((item) => (
                    <div className="set-startup-item" key={item.id}>
                      <div className="set-startup-info">
                        <div className="set-startup-name">
                          <span className="set-startup-title">{item.name}</span>
                          <span className="set-startup-badges">
                            <Badge tone="neutral">{item.location}</Badge>
                            <Badge tone={item.enabled ? "safe" : "neutral"}>
                              {item.enabled ? "Enabled" : "Disabled"}
                            </Badge>
                            {item.isProtected && <Badge tone="caution">System Protected</Badge>}
                          </span>
                        </div>
                        <span className="set-startup-cmd" title={item.command}>
                          {item.command}
                        </span>
                      </div>
                      <div className="set-startup-actions">
                        {item.canToggle && (
                          <Button
                            size="sm"
                            variant={item.enabled ? "secondary" : "primary"}
                            loading={togglingId === item.id}
                            onClick={() => handleToggleStartup(item)}
                          >
                            {item.enabled ? "Disable" : "Enable"}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="ghost"
                          icon={<IconCopy size={12} />}
                          title="Copy command line"
                          onClick={() => navigator.clipboard.writeText(item.command)}
                        >
                          Copy
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </Panel>

            <Panel title="Data">
              <div className="set-status">
                <Badge tone="safe">
                  <IconShield size={11} />
                  Local only
                </Badge>
              </div>
              <p className="set-note set-note--spaced">
                Scans run on this machine and results stay in memory. Cyron sends nothing over the
                network and contains no telemetry. Settings and scan history live in this Windows
                profile so they survive a restart.
              </p>
            </Panel>

            <Panel title="About">
              <KeyValue
                items={[
                  { label: "Name", value: "Cyron" },
                  { label: "Version", value: "0.1.0" },
                  { label: "Stage", value: "Release Candidate 1 (RC1)" },
                  { label: "Stack", value: "Tauri v2 · Rust · React · TypeScript" },
                  {
                    label: "Repository",
                    value: (
                      <a
                        className="set-link"
                        href="https://github.com/Ry0x7/Cyron-uninstaller"
                        target="_blank"
                        rel="noreferrer noopener"
                      >
                        Ry0x7/Cyron-uninstaller
                        <IconExternal size={12} />
                      </a>
                    ),
                  },
                ]}
              />
              <p className="set-note set-note--spaced">
                Application sizes come from installer metadata when the publisher recorded it.
                Measure a folder from the Uninstaller details to get the real on-disk size.
              </p>
            </Panel>
          </div>
        </div>
      </div>
    </>
  );
}
