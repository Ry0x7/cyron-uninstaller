import { useEffect, useState } from "react";
import { PageTabs, ViewHeader } from "../../components/ui";
import { Activity } from "../activity/Activity";
import { Recovery } from "./Recovery";
import type { ViewId } from "../../lib/types";
import "./history.css";

type HistoryTab = "activity" | "recovery";

export function HistoryRecovery({
  onNavigate,
  recoveryRequest = 0,
}: {
  onNavigate: (view: ViewId) => void;
  recoveryRequest?: number;
}) {
  const [tab, setTab] = useState<HistoryTab>("activity");

  useEffect(() => {
    if (recoveryRequest > 0) setTab("recovery");
  }, [recoveryRequest]);

  useEffect(() => {
    const openRecovery = () => setTab("recovery");
    window.addEventListener("cyron:open-recovery", openRecovery);
    return () => window.removeEventListener("cyron:open-recovery", openRecovery);
  }, []);

  return (
    <div>
      <ViewHeader
        title="History & Recovery"
        description="See what Cyron changed on this PC, restore files from quarantine, and review past cleanup outcomes. Records stay local."
        actions={
          <PageTabs<HistoryTab>
            ariaLabel="History and recovery sections"
            value={tab}
            onChange={setTab}
            options={[
              { value: "activity", label: "Activity" },
              { value: "recovery", label: "Recovery" },
            ]}
          />
        }
      />

      <div className="view-body">
        <div hidden={tab !== "activity"}>
          <Activity embedded onNavigate={onNavigate} />
        </div>
        {tab === "recovery" && <Recovery />}
      </div>
    </div>
  );
}
