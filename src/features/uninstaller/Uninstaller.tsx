import { useMemo, useState, type KeyboardEvent } from "react";
import { api } from "../../lib/api";
import { formatBytes, formatCount, formatDate, truncateMiddle } from "../../lib/format";
import { useAsync } from "../../lib/hooks";
import type { InstalledApp } from "../../lib/types";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Panel,
  SearchField,
  Segmented,
  Select,
  Skeleton,
  ViewHeader,
} from "../../components/ui";
import { IconApps, IconLock, IconRefresh, IconShield } from "../../components/Icons";
import { AppDrawer } from "./AppDrawer";
import "./uninstaller.css";

type ScopeFilter = "all" | "machine" | "user";
type SortKey = "name" | "publisher" | "installed" | "size";

const SORT_OPTIONS: { value: SortKey; label: string }[] = [
  { value: "name", label: "Name (A–Z)" },
  { value: "publisher", label: "Publisher (A–Z)" },
  { value: "installed", label: "Newest install" },
  { value: "size", label: "Largest estimate" },
];

function compareNullableText(a: string | null, b: string | null): number {
  if (!a && !b) return 0;
  if (!a) return 1;
  if (!b) return -1;
  return a.localeCompare(b, undefined, { sensitivity: "base", numeric: true });
}

function compareNullableNumber(a: number | null, b: number | null): number {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b - a;
}

function installDateSortValue(value: string | null): number | null {
  if (!value) return null;
  const normalized = /^\\d{8}$/.test(value)
    ? `${value.slice(0, 4)}-${value.slice(4, 6)}-${value.slice(6, 8)}`
    : value;
  const timestamp = Date.parse(normalized);
  return Number.isFinite(timestamp) ? timestamp : null;
}

function normalizeSearch(value: string): string {
  return value.trim().normalize("NFD").replace(/[\\u0300-\\u036f]/g, "").toLocaleLowerCase();
}

export function Uninstaller() {
  const inventory = useAsync(() => api.listInstalledApps(), []);
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<ScopeFilter>("all");
  const [sort, setSort] = useState<SortKey>("name");
  const [selected, setSelected] = useState<InstalledApp | null>(null);

  const apps = inventory.data?.apps ?? [];
  const warnings = [...new Set(inventory.data?.warnings ?? [])];

  const counts = useMemo(() => {
    const needle = normalizeSearch(query);
    const matching = needle
      ? apps.filter((app) =>
          [app.name, app.publisher, app.installLocation, app.version].some(
            (value) => value !== null && normalizeSearch(value).includes(needle),
          ),
        )
      : apps;
    let machine = 0;
    let user = 0;
    for (const app of matching) {
      if (app.scope === "machine") machine += 1;
      else user += 1;
    }
    return { all: matching.length, machine, user };
  }, [apps, query]);

  const visible = useMemo(() => {
    const needle = normalizeSearch(query);
    const filtered = apps.filter((app) => {
      if (scope !== "all" && app.scope !== scope) return false;
      if (!needle) return true;
      return [app.name, app.publisher, app.installLocation, app.version]
        .some((value) => value !== null && normalizeSearch(value).includes(needle));
    });

    return filtered.sort((a, b) => {
      switch (sort) {
        case "publisher":
          return compareNullableText(a.publisher, b.publisher) || compareNullableText(a.name, b.name);
        case "installed": {
          const aDate = installDateSortValue(a.installDate);
          const bDate = installDateSortValue(b.installDate);
          if (aDate === null && bDate !== null) return 1;
          if (bDate === null && aDate !== null) return -1;
          return (aDate === null || bDate === null ? 0 : bDate - aDate) || compareNullableText(a.name, b.name);
        }
        case "size":
          return compareNullableNumber(a.estimatedSize, b.estimatedSize) || compareNullableText(a.name, b.name);
        default:
          return compareNullableText(a.name, b.name);
      }
    });
  }, [apps, query, scope, sort]);

  const loading = inventory.status === "loading" && apps.length === 0;

  const openRow = (app: InstalledApp) => setSelected(app);
  const onRowKeyDown = (event: KeyboardEvent<HTMLTableRowElement>, app: InstalledApp) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openRow(app);
    }
  };

  return (
    <>
      <ViewHeader
        title="Uninstaller"
        description="Everything Windows reports as installed, read from the machine and user uninstall registry keys."
        actions={
          <>
            <Badge tone="safe">Read-only</Badge>
            <Button
              icon={<IconRefresh size={15} />}
              loading={inventory.status === "loading"}
              onClick={inventory.reload}
            >
              Refresh
            </Button>
          </>
        }
      />

      <div className="view-body">
        <Banner tone="accent">
          <strong>Inventory only.</strong> Standard, force and batch uninstalling plus
          leftover scanning arrive in Phase 4. Nothing on this screen removes software.
        </Banner>

        {warnings.length > 0 && (
          <Banner tone="caution">
            Some registry keys could not be read: {warnings.slice(0, 3).join(" · ")}
            {warnings.length > 3 ? ` and ${warnings.length - 3} more` : ""}.
          </Banner>
        )}

        {inventory.status === "error" && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={inventory.reload}>
                Try again
              </Button>
            }
          >
            {inventory.error}
          </Banner>
        )}

        <Panel
          title="Installed applications"
          subtitle="Publisher, version, install date and size exactly as reported by each installer."
          flush
        >
          <div className="un-toolbar">
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder="Search name, publisher or path"
              ariaLabel="Search installed applications"
              className="un-search"
            />
            <Segmented
              ariaLabel="Filter by install scope"
              value={scope}
              onChange={setScope}
              options={[
                { value: "all", label: "All", count: counts.all },
                { value: "machine", label: "Machine", count: counts.machine },
                { value: "user", label: "User", count: counts.user },
              ]}
            />
            <div className="un-toolbar__end">
              <span className="un-count" aria-live="polite">
                {loading ? "Reading registry…" : `${formatCount(visible.length)} shown`}
              </span>
              <Select
                ariaLabel="Sort applications"
                value={sort}
                onChange={(value) => setSort(value as SortKey)}
                options={SORT_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
              />
            </div>
          </div>

          {loading ? (
            <div className="un-loading" aria-busy="true" aria-label="Loading installed applications">
              {Array.from({ length: 8 }, (_, index) => (
                <div className="un-loading__row" key={index}>
                  <Skeleton width="38%" />
                  <Skeleton width="20%" />
                  <Skeleton width="10%" />
                  <Skeleton width="14%" />
                  <Skeleton width="8%" />
                </div>
              ))}
            </div>
          ) : visible.length === 0 ? (
            <EmptyState
              icon={IconApps}
              title={
                inventory.status === "error" && apps.length === 0
                  ? "Inventory unavailable"
                  : apps.length === 0
                    ? "No applications found"
                    : "No matching applications"
              }
              description={
                inventory.status === "error" && apps.length === 0
                  ? "The registry could not be read, so this is not an empty inventory. Retry, or check the message above if the problem continues."
                  : apps.length === 0
                    ? "Cyron could not read any uninstall entries from the registry. This can happen on a locked-down machine."
                    : "Try a different search term or switch the scope filter back to All."
              }
              action={
                apps.length === 0 ? (
                  <Button onClick={inventory.reload}>Scan registry again</Button>
                ) : (
                  <Button
                    onClick={() => {
                      setQuery("");
                      setScope("all");
                    }}
                  >
                    Clear filters
                  </Button>
                )
              }
            />
          ) : (
            <div className="table-wrap un-table">
              <table className="table un-grid">
                <colgroup>
                  <col style={{ width: "33%" }} />
                  <col style={{ width: "20%" }} />
                  <col style={{ width: "10%" }} />
                  <col style={{ width: "12%" }} />
                  <col style={{ width: "11%" }} />
                  <col style={{ width: "14%" }} />
                </colgroup>
                <thead>
                  <tr>
                    <th scope="col">Application</th>
                    <th scope="col">Publisher</th>
                    <th scope="col">Version</th>
                    <th scope="col">Installed</th>
                    <th scope="col" className="numeric">
                      Size
                    </th>
                    <th scope="col">Scope</th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((app) => (
                    <tr
                      key={app.id}
                      className="un-row"
                      data-clickable="true"
                      tabIndex={0}
                      aria-selected={selected?.id === app.id}
                      aria-label={`View details for ${app.name}`}
                      onClick={() => openRow(app)}
                      onKeyDown={(event) => onRowKeyDown(event, app)}
                    >
                      <td>
                        <div className="cell-name">
                          <span className="cell-name__title un-name" title={app.name}>
                            {app.isProtected && (
                              <IconLock size={11} className="un-lock" aria-label="System component" />
                            )}
                            {app.name}
                          </span>
                          <span className="cell-name__meta">
                            {app.installLocation
                              ? truncateMiddle(app.installLocation, 58)
                              : "No install location reported"}
                          </span>
                        </div>
                      </td>
                      <td className="un-publisher">{app.publisher ?? "—"}</td>
                      <td className="mono un-version">{app.version ?? "—"}</td>
                      <td className="un-date">{formatDate(app.installDate)}</td>
                      <td
                        className="numeric"
                        title={
                          app.estimatedSize === null
                            ? "This installer did not record an EstimatedSize value"
                            : "EstimatedSize reported by the installer in the registry"
                        }
                      >
                        {app.estimatedSize === null ? "—" : formatBytes(app.estimatedSize)}
                      </td>
                      <td>
                        <div className="un-scope">
                          <Badge tone="neutral">
                            {app.scope === "machine" ? "Machine" : "User"}
                          </Badge>
                          <span className="un-arch">{app.architecture === "x64" ? "64-bit" : "32-bit"}</span>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Panel>

        {visible.length > 0 && (
          <p className="un-footnote">
            <IconShield size={13} /> Sizes are installer estimates from the registry, not scanned
            folder sizes. Open an application to measure its install folder for real.
          </p>
        )}

      </div>

      <AppDrawer app={selected} onClose={() => setSelected(null)} />
    </>
  );
}
