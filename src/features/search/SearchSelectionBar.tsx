import { Button, IconButton } from "../../components/ui";
import { IconClose, IconCopy, IconDownload } from "../../components/Icons";
import { formatBytes, formatCount } from "../../lib/format";

export interface SearchSelectionBarProps {
  count: number;
  bytes: number;
  busy: "csv" | "txt" | null;
  onCopy: () => void;
  onExportCsv: () => void;
  onExportTxt: () => void;
  onClear: () => void;
}

export function SearchSelectionBar({
  count,
  bytes,
  busy,
  onCopy,
  onExportCsv,
  onExportTxt,
  onClear,
}: SearchSelectionBarProps) {
  if (count === 0) return null;
  const disabled = busy !== null;
  return (
    <div className="search-page__selection" role="group" aria-label="Selected results actions">
      <span className="search-page__selection-count">
        <strong>{formatCount(count)}</strong> selected
        <span className="search-page__selection-bytes">{formatBytes(bytes)}</span>
      </span>
      <span className="search-page__selection-actions">
        <Button
          size="sm"
          variant="ghost"
          icon={<IconCopy size={13} />}
          disabled={disabled}
          onClick={onCopy}
        >
          Copy paths
        </Button>
        <Button
          size="sm"
          variant="ghost"
          icon={<IconDownload size={13} />}
          loading={busy === "csv"}
          disabled={disabled}
          aria-label="Export selected rows as CSV"
          onClick={onExportCsv}
        >
          CSV
        </Button>
        <Button
          size="sm"
          variant="ghost"
          icon={<IconDownload size={13} />}
          loading={busy === "txt"}
          disabled={disabled}
          aria-label="Export selected rows as TXT"
          onClick={onExportTxt}
        >
          TXT
        </Button>
        <IconButton label="Clear selection" onClick={onClear} disabled={disabled}>
          <IconClose size={13} />
        </IconButton>
      </span>
    </div>
  );
}
