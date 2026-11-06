import { memo } from "react";
import { Drawer, KeyValue } from "../../components/ui";
import { formatBytes, formatCount, formatDate } from "../../lib/format";
import type { ScanNode } from "../../lib/types";

interface ItemPropertiesDialogProps {
  item: ScanNode | null;
  onClose: () => void;
  onReveal?: (path: string) => void;
}

export const ItemPropertiesDialog = memo(function ItemPropertiesDialog({
  item,
  onClose,
  onReveal,
}: ItemPropertiesDialogProps) {
  if (!item) return null;

  const kvItems = [
    { label: "Full Path", value: item.path || "Synthetic Group" },
    { label: "Type", value: item.kind === "dir" ? "Directory / Folder" : "File" },
    { label: "Logical Size", value: formatBytes(item.size) },
    { label: "Size on Disk (Allocated)", value: formatBytes(item.allocated) },
    ...(item.kind === "dir"
      ? [
          { label: "Contained Files", value: formatCount(item.fileCount) },
          { label: "Subdirectories", value: formatCount(item.dirCount) },
        ]
      : []),
    {
      label: "Last Modified",
      value: item.modified ? formatDate(item.modified) : "Not available",
    },
  ];

  return (
    <Drawer
      open={Boolean(item)}
      title={item.name}
      subtitle={item.kind === "dir" ? "Folder Properties" : "File Properties"}
      onClose={onClose}
    >
      <div className="stack" style={{ gap: 16 }}>
        <KeyValue items={kvItems} />
        {item.path && onReveal && (
          <div style={{ marginTop: 8 }}>
            <button
              type="button"
              className="btn btn--sm btn--ghost"
              onClick={() => onReveal(item.path)}
            >
              Reveal in File Explorer
            </button>
          </div>
        )}
      </div>
    </Drawer>
  );
});
