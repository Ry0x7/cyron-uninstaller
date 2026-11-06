import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useSettings, type ThemePreference } from "../../lib/settings";
import {
  Badge,
  Button,
  Field,
  KeyValue,
  Panel,
  Toggle,
  ViewHeader,
} from "../../components/ui";
import {
  IconCheck,
  IconExternal,
  IconLock,
  IconMonitor,
  IconMoon,
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
  const { settings, setTheme, setIncludeHidden, setExclusions, reset } = useSettings();

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
                  Reset restores the system theme, hidden-file scanning and an empty exclusion
                  list.
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
                <Badge tone="accent">
                  <IconLock size={11} />
                  Read-only
                </Badge>
                <span className="set-note">This build has no delete, uninstall or kill action.</span>
              </div>

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
                The Cleaner shows what a future cleaning phase could remove, with risk levels. It
                does not remove anything in this build.
              </p>
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
                  { label: "Stage", value: "Phase 2 · disk intelligence, read-only" },
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
