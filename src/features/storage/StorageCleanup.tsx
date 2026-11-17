import { useEffect, useState } from "react";
import { Badge, PageTabs, ViewHeader } from "../../components/ui";
import { Cleaner } from "../cleaner/Cleaner";
import { Analyzer } from "../analyzer/Analyzer";

type StorageMode = "cleanup" | "disk";

export function StorageCleanup({
  mode,
  onModeChange,
  initialAnalyzerPath,
  initialAnalyzerMode,
  cleanerTargetId,
  onNavigateToCleaner,
  onOpenRecovery,
}: {
  mode: StorageMode;
  onModeChange: (mode: StorageMode) => void;
  initialAnalyzerPath?: string;
  initialAnalyzerMode?: "map" | "duplicates" | "storage";
  cleanerTargetId?: string | null;
  onNavigateToCleaner: (targetId?: string) => void;
  onOpenRecovery: () => void;
}) {
  const [mounted, setMounted] = useState<Set<StorageMode>>(() => new Set([mode]));

  useEffect(() => {
    setMounted((current) => (current.has(mode) ? current : new Set([...current, mode])));
  }, [mode]);

  return (
    <div>
      <ViewHeader
        title="Storage & Cleanup"
        description="See where space goes and reclaim what can be rebuilt. Files move to quarantine after a preview; source code, credentials, models and personal data stay untouched."
        meta={<Badge tone="neutral">Review before cleanup</Badge>}
        actions={
          <PageTabs<StorageMode>
            ariaLabel="Storage and cleanup sections"
            value={mode}
            onChange={onModeChange}
            options={[
              { value: "cleanup", label: "Cleanup" },
              { value: "disk", label: "Disk space" },
            ]}
          />
        }
      />

      <div className="view-body">
        {mounted.has("cleanup") && (
          <div hidden={mode !== "cleanup"}>
            <Cleaner
              embedded
              initialTargetId={cleanerTargetId}
              onOpenRecovery={onOpenRecovery}
            />
          </div>
        )}

        {mounted.has("disk") && (
          <div hidden={mode !== "disk"}>
            <Analyzer
              embedded
              initialPath={initialAnalyzerPath}
              initialMode={initialAnalyzerMode}
              onNavigateToCleaner={onNavigateToCleaner}
            />
          </div>
        )}
      </div>
    </div>
  );
}
