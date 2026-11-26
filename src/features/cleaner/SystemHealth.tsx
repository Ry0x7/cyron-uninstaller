import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Banner, Button, EmptyState, KeyValue, Modal, Panel, SectionHeader, Skeleton } from "../../components/ui";
import { IconCheck, IconRefresh, IconShield } from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import { formatCount, relativeTime } from "../../lib/format";
import type {
  BrokenEntryPlan,
  DeadEntryScanReport,
  DeadRegistryEntry,
  DeadStartupEntry,
  DeadStartupPlan,
} from "../../lib/types";

type Status = "loading" | "ready" | "error";

type Removal =
  | { kind: "startup"; entry: DeadStartupEntry; plan: DeadStartupPlan }
  | { kind: "registry"; entry: DeadRegistryEntry; plan: BrokenEntryPlan };

function startupTitle(entry: DeadStartupEntry): string {
  return `${entry.registryKeyPath}\\${entry.valueName}`;
}

function registryTitle(entry: DeadRegistryEntry): string {
  return entry.displayName;
}

export function SystemHealth({ readOnly }: { readOnly: boolean | null }) {
  const [report, setReport] = useState<DeadEntryScanReport | null>(null);
  const [status, setStatus] = useState<Status>("loading");
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [busyId, setBusyId] = useState<string | null>(null);
  const [removal, setRemoval] = useState<Removal | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  const mounted = useRef(true);
  const request = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const scan = useCallback(async (initial: boolean) => {
    const id = ++request.current;
    if (initial) setStatus("loading");
    else setRefreshing(true);
    setActionError(null);
    try {
      const data = await api.scanDeadEntries();
      if (!mounted.current || request.current !== id) return;
      setReport(data);
      setStatus("ready");
      setError(null);
    } catch (caught) {
      if (!mounted.current || request.current !== id) return;
      setError(errorMessage(caught));
      setStatus((prev) => (prev === "ready" ? "ready" : "error"));
    } finally {
      if (mounted.current && request.current === id) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    void scan(true);
  }, [scan]);

  const startRemoval = async (entry: DeadStartupEntry | DeadRegistryEntry) => {
    setBusyId("valueName" in entry ? entry.id : entry.appId);
    setActionError(null);
    setResult(null);
    try {
      if ("valueName" in entry) {
        const plan = await api.prepareDeadStartupRemoval(entry.id);
        if (mounted.current) setRemoval({ kind: "startup", entry, plan });
      } else {
        const plan = await api.prepareBrokenInventoryCleanup(entry.appId);
        if (mounted.current) setRemoval({ kind: "registry", entry, plan });
      }
    } catch (caught) {
      if (mounted.current) setActionError(errorMessage(caught));
    } finally {
      if (mounted.current) setBusyId(null);
    }
  };

  const confirmRemoval = async () => {
    if (!removal) return;
    setConfirming(true);
    setActionError(null);
    try {
      if (removal.kind === "startup") {
        const output = await api.executeDeadStartupRemoval(
          removal.plan.planId,
          removal.plan.location,
        );
        if (!mounted.current) return;
        setResult(
          `Removed “${output.name}”. The startup value was exported to ${output.registryBackupPath} and the batch is in quarantine.`,
        );
        setReport((prev) =>
          prev
            ? { ...prev, startup: prev.startup.filter((item) => item.id !== removal.entry.id) }
            : prev,
        );
      } else {
        const output = await api.executeBrokenInventoryCleanup(
          removal.plan.planId,
          removal.plan.registryKeyPath,
        );
        if (!mounted.current) return;
        if (output.deletedRegistryKeys === 0) {
          setActionError(
            output.errors.join(" ") ||
              "The registration was not deleted. It may have changed since the scan.",
          );
          setRemoval(null);
          return;
        }
        const backup = output.registryBackupPath
          ? ` The key tree was exported to ${output.registryBackupPath} and the batch is in quarantine.`
          : "";
        const errors =
          output.errors.length > 0 ? ` Not everything completed: ${output.errors.join(" ")}` : "";
        setResult(`Removed “${removal.entry.displayName}” from the uninstall registry.${backup}${errors}`);
        setReport((prev) =>
          prev
            ? {
                ...prev,
                registry: prev.registry.filter(
                  (item) => item.registryKeyPath !== removal.entry.registryKeyPath,
                ),
              }
            : prev,
        );
      }
      setRemoval(null);
    } catch (caught) {
      if (!mounted.current) return;
      setActionError(errorMessage(caught));
      setRemoval(null);
    } finally {
      if (mounted.current) setConfirming(false);
    }
  };

  const startupCount = report?.startup.length ?? 0;
  const registryCount = report?.registry.length ?? 0;
  const total = startupCount + registryCount;
  const removalBlocked = readOnly !== false;

  const actions = (
    <>
      <span className="small faint">
        {report ? `Scanned ${relativeTime(report.scannedAt)}` : "Not scanned yet"}
      </span>
      <Button
        size="sm"
        icon={<IconRefresh size={14} />}
        loading={refreshing || (status === "loading" && !report)}
        disabled={status === "loading" && !report}
        onClick={() => void scan(false)}
      >
        Scan registry
      </Button>
    </>
  );

  return (
    <>
      <SectionHeader
        title="System health"
        description="Find startup commands whose program is gone and uninstall registrations whose uninstaller and install folder are both missing. Cyron re-checks each entry before removal and exports the .reg backup first."
        actions={actions}
      />

      {actionError && (
        <Banner tone="danger">
          <strong>Nothing was removed.</strong> {actionError}
        </Banner>
      )}

      {result && (
        <Banner
          tone="safe"
          actions={
            <Button size="sm" variant="ghost" onClick={() => setResult(null)}>
              Dismiss
            </Button>
          }
        >
          {result}
        </Banner>
      )}

      {status === "error" && (
        <Banner
          tone="danger"
          actions={
            <Button size="sm" loading={refreshing} onClick={() => void scan(false)}>
              Retry
            </Button>
          }
        >
          <strong>Could not scan the registry.</strong> {error}
        </Banner>
      )}

      {report && report.warnings.length > 0 && (
        <Banner tone="caution">
          <strong>{report.warnings.length === 1 ? "One registry hive" : `${report.warnings.length} registry hives`}</strong>{" "}
          could not be read completely. Entries in those hives are not listed.
          <details className="cl-warning-details">
            <summary>Review the blocked hives</summary>
            <ul className="cl-warning-list">
              {report.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          </details>
        </Banner>
      )}

      {status === "loading" && !report ? (
        <div className="cl-health-list">
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} height={54} />
          ))}
        </div>
      ) : status !== "error" || report ? (
        <>
          <div className="cl-health-summary">
            <span>
              <strong>{formatCount(startupCount)}</strong> dead startup{" "}
              {startupCount === 1 ? "entry" : "entries"}
            </span>
            <span>
              <strong>{formatCount(registryCount)}</strong> dead uninstall{" "}
              {registryCount === 1 ? "registration" : "registrations"}
            </span>
            <span className="faint">
              {total === 0 ? "Nothing to clean up" : `${formatCount(total)} found`}
            </span>
          </div>

          <Panel
            title="Startup entries"
            subtitle="Registry Run and RunOnce values whose executable no longer exists. Startup-folder shortcuts are not touched."
            flush
          >
            {startupCount === 0 ? (
              <EmptyState
                compact
                icon={IconShield}
                title="No dead startup entries"
                description="Every registry startup command points at a program that still exists."
              />
            ) : (
              <ul className="cl-health-list">
                {report?.startup.map((entry) => {
                  const disabled = removalBlocked || !entry.canRemove || busyId === entry.id;
                  return (
                    <li className="cl-health-row" key={entry.id}>
                      <div className="cl-health-main">
                        <div className="cl-health-title">
                          <strong>{entry.name}</strong>
                          {entry.isProtected && <Badge tone="neutral">Protected</Badge>}
                          {!entry.isProtected && entry.requiresElevation && (
                            <Badge tone="caution">Needs elevation</Badge>
                          )}
                        </div>
                        <p className="cl-health-reason">{entry.reason}</p>
                        <p className="cl-health-path" title={entry.command}>
                          {entry.command}
                        </p>
                        <p className="cl-health-meta">
                          {entry.location} · {startupTitle(entry)}
                        </p>
                      </div>
                      <div className="cl-health-actions">
                        <Button
                          size="sm"
                          variant="ghost"
                          loading={busyId === entry.id}
                          disabled={disabled}
                          aria-label={`Remove ${entry.name}`}
                          title={
                            entry.isProtected
                              ? "Windows security startup entries cannot be removed."
                              : entry.requiresElevation && !entry.canRemove
                                ? "Restart Cyron as administrator to remove this entry."
                                : removalBlocked
                                  ? "Enable cleaning to remove entries."
                                  : undefined
                          }
                          onClick={() => void startRemoval(entry)}
                        >
                          Remove
                        </Button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </Panel>

          <Panel
            title="Uninstall registrations"
            subtitle="Installed-app entries whose uninstaller and install folder are both gone. Cyron also protects system components and shared vendor keys."
            flush
          >
            {registryCount === 0 ? (
              <EmptyState
                compact
                icon={IconCheck}
                title="No dead uninstall registrations"
                description="Every registered application still has an uninstaller or an install folder."
              />
            ) : (
              <ul className="cl-health-list">
                {report?.registry.map((entry) => {
                  const disabled = removalBlocked || !entry.canRemove || busyId === entry.appId;
                  return (
                    <li className="cl-health-row" key={entry.registryKeyPath}>
                      <div className="cl-health-main">
                        <div className="cl-health-title">
                          <strong>{registryTitle(entry)}</strong>
                          {entry.displayVersion && <span className="cl-health-version">{entry.displayVersion}</span>}
                          {entry.isProtected && <Badge tone="neutral">Protected</Badge>}
                          {!entry.isProtected && entry.requiresElevation && (
                            <Badge tone="caution">Needs elevation</Badge>
                          )}
                        </div>
                        <p className="cl-health-reason">{entry.reason}</p>
                        <p className="cl-health-path" title={entry.registryKeyPath}>
                          {entry.registryKeyPath}
                        </p>
                        <p className="cl-health-meta">
                          {entry.publisher || "Unknown publisher"}
                          {entry.installLocation ? ` · ${entry.installLocation}` : " · no install folder recorded"}
                        </p>
                      </div>
                      <div className="cl-health-actions">
                        <Button
                          size="sm"
                          variant="ghost"
                          loading={busyId === entry.appId}
                          disabled={disabled}
                          aria-label={`Remove ${entry.displayName}`}
                          title={
                            entry.requiresElevation && !entry.canRemove
                              ? "Restart Cyron as administrator to remove this registration."
                              : removalBlocked
                                ? "Enable cleaning to remove registrations."
                                : undefined
                          }
                          onClick={() => void startRemoval(entry)}
                        >
                          Remove
                        </Button>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </Panel>
        </>
      ) : null}

      <Modal
        open={removal !== null}
        title={removal?.kind === "startup" ? "Remove dead startup entry?" : "Remove dead registration?"}
        subtitle="Cyron re-checks the live entry first and exports a verified .reg backup before deleting. This is a single-use confirmation."
        onClose={() => {
          if (!confirming) setRemoval(null);
        }}
        footer={
          <>
            <Button variant="ghost" disabled={confirming} onClick={() => setRemoval(null)}>
              Cancel
            </Button>
            <Button variant="danger" loading={confirming} onClick={() => void confirmRemoval()}>
              {removal?.kind === "startup" ? "Remove startup entry" : "Remove registration"}
            </Button>
          </>
        }
      >
        {removal?.kind === "startup" && (
          <>
            <p className="cl-health-confirm">
              <strong>{removal.entry.name}</strong> points at a program that no longer exists.
            </p>
            <KeyValue
              items={[
                { label: "Command", value: removal.entry.command },
                { label: "Location", value: removal.entry.location },
                { label: "Registry value", value: startupTitle(removal.entry) },
                { label: "Reason", value: removal.entry.reason },
              ]}
            />
          </>
        )}
        {removal?.kind === "registry" && (
          <>
            <p className="cl-health-confirm">
              <strong>{removal.entry.displayName}</strong> has no working uninstaller and no install
              folder.
            </p>
            <KeyValue
              items={[
                { label: "Registry key", value: removal.entry.registryKeyPath },
                { label: "Publisher", value: removal.entry.publisher || "Unknown" },
                { label: "Version", value: removal.entry.displayVersion || "Unknown" },
                { label: "Reason", value: removal.entry.reason },
              ]}
            />
          </>
        )}
      </Modal>
    </>
  );
}
