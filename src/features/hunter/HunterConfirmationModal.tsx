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
          Find an app by window
        </>
      }
      subtitle="Point at a desktop window to inspect the application behind it."
      onClose={onCancel}
      maxWidth={500}
      footer={
        <>
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" icon={<IconCrosshair size={14} />} onClick={onConfirm}>
            Start targeting
          </Button>
        </>
      }
    >
      <p className="hunter-intro">
        Hunter Mode follows the cursor across your desktop, outside Cyron, and matches the window to
        its process and installed application. You can lock a target and review it before any change.
      </p>

      <ul className="hunter-capabilities">
        <li>
          <span className="hunter-capability__icon hunter-capability__icon--accent">
            <IconTrash size={15} />
          </span>
          <div>
            <strong>Uninstall with a reviewed plan</strong>
            <span>Match the window to its registered uninstaller, then review every step.</span>
          </div>
        </li>
        <li>
          <span className="hunter-capability__icon hunter-capability__icon--danger">
            <IconStop size={15} />
          </span>
          <div>
            <strong>End an unresponsive process</strong>
            <span>The exact PID and process are shown, with a separate confirmation.</span>
          </div>
        </li>
        <li>
          <span className="hunter-capability__icon hunter-capability__icon--safe">
            <IconShield size={15} />
          </span>
          <div>
            <strong>Windows processes stay protected</strong>
            <span>Critical system processes cannot be ended from Hunter Mode.</span>
          </div>
        </li>
      </ul>

      <p className="hunter-exit">
        Press <kbd>Esc</kbd> at any time to exit. Cursor tracking stops immediately.
      </p>
    </Modal>
  );
}
