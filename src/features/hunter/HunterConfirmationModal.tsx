import { Button, Modal } from "../../components/ui";
import { IconCrosshair, IconShield, IconStop, IconTrash } from "../../components/Icons";

interface HunterConfirmationModalProps {
  open: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function HunterConfirmationModal({
  open,
  onConfirm,
  onCancel,
}: HunterConfirmationModalProps) {
  return (
    <Modal
      open={open}
      title={
        <>
          <IconCrosshair size={16} />
          Start Hunter Mode
        </>
      }
      subtitle="Desktop-wide window inspection and targeted application management."
      onClose={onCancel}
      maxWidth={490}
      footer={
        <div className="row" style={{ gap: 8, justifyContent: "flex-end", width: "100%" }}>
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            variant="primary"
            icon={<IconCrosshair size={14} />}
            onClick={onConfirm}
          >
            Start Hunter Mode
          </Button>
        </div>
      }
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <p style={{ fontSize: 13, color: "var(--text)", lineHeight: 1.5, margin: 0 }}>
          Hunter Mode tracks your cursor across all open desktop windows outside Cyron to inspect
          the underlying application, verified executable path, and process ID.
        </p>

        <div className="exp-modal-card">
          <span style={{ fontSize: 11, fontWeight: 600, color: "var(--text-strong)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
            Available Actions
          </span>
          <div style={{ display: "flex", flexDirection: "column", gap: 8, fontSize: 12, color: "var(--text-muted)" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <IconTrash size={14} style={{ color: "var(--accent)" }} />
              <span><strong>Guided Uninstallation:</strong> Match active window to registered uninstallers.</span>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <IconStop size={14} style={{ color: "var(--danger)" }} />
              <span><strong>Process Termination:</strong> Safely kill frozen, unresponsive, or background processes.</span>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <IconShield size={14} style={{ color: "var(--safe)" }} />
              <span><strong>Safety Protected:</strong> Windows core processes cannot be accidentally terminated.</span>
            </div>
          </div>
        </div>

        <div style={{ fontSize: 11.5, color: "var(--text-muted)", padding: "4px 0" }}>
          <strong>How to exit:</strong> Press <kbd style={{ padding: "1px 5px", background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 3, fontFamily: "var(--font-mono)" }}>Esc</kbd> at any time or click <strong>Exit Hunter Mode</strong> in the sidebar. Cursor tracking will terminate immediately.
        </div>
      </div>
    </Modal>
  );
}
