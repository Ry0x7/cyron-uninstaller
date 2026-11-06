import { CATEGORY_LABEL, type CategoryKey } from "../../lib/categories";
import {
  activeFilterCount,
  defaultFilters,
  type AnalyzerAge,
  type AnalyzerFilters as Filters,
  type AnalyzerSortField,
} from "../../lib/analyzerFilters";
import { Button } from "../../components/ui";
import { formatBytes, formatCount } from "../../lib/format";

const MIN_SIZES: { value: number; label: string }[] = [
  { value: 0, label: "Any size" },
  { value: 1024 * 1024, label: "≥ 1 MB" },
  { value: 10 * 1024 * 1024, label: "≥ 10 MB" },
  { value: 100 * 1024 * 1024, label: "≥ 100 MB" },
  { value: 1024 * 1024 * 1024, label: "≥ 1 GB" },
];

const AGES: { value: AnalyzerAge; label: string }[] = [
  { value: "any", label: "Any age" },
  { value: "recent", label: "< 30 days" },
  { value: "mid", label: "30–365 days" },
  { value: "old", label: "> 1 year" },
];

const SORTS: { value: AnalyzerSortField; label: string }[] = [
  { value: "size", label: "Size" },
  { value: "name", label: "Name" },
  { value: "files", label: "Files" },
  { value: "modified", label: "Modified" },
];

const CATEGORIES: CategoryKey[] = [
  "code",
  "image",
  "video",
  "audio",
  "archive",
  "document",
  "app",
  "other",
];

interface AnalyzerFilterBarProps {
  filters: Filters;
  onChange: (filters: Filters) => void;
  visible: { bytes: number; files: number };
  total: { bytes: number; files: number };
  method: string;
  methodNote: string | null;
  treeOpen: boolean;
  onToggleTree: () => void;
}

export function AnalyzerFilterBar({
  filters,
  onChange,
  visible,
  total,
  method,
  methodNote,
  treeOpen,
  onToggleTree,
}: AnalyzerFilterBarProps) {
  const active = activeFilterCount(filters);
  const update = <Key extends keyof Filters>(key: Key, value: Filters[Key]) =>
    onChange({ ...filters, [key]: value });

  return (
    <div className="az-toolbar">
      <div className="az-toolbar__main">
        <label className="az-field">
          <span className="az-field__label">Min size</span>
          <span className="select">
            <select
              aria-label="Minimum file size"
              value={filters.minSize}
              onChange={(event) => update("minSize", Number(event.target.value))}
            >
              {MIN_SIZES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </span>
        </label>

        <label className="az-field">
          <span className="az-field__label">Type</span>
          <span className="select">
            <select
              aria-label="File category"
              value={filters.category}
              onChange={(event) => update("category", event.target.value as CategoryKey | "all")}
            >
              <option value="all">All types</option>
              {CATEGORIES.map((category) => (
                <option key={category} value={category}>
                  {CATEGORY_LABEL[category]}
                </option>
              ))}
            </select>
          </span>
        </label>

        <label className="az-field">
          <span className="az-field__label">Age</span>
          <span className="select">
            <select
              aria-label="File age"
              value={filters.age}
              onChange={(event) => update("age", event.target.value as AnalyzerAge)}
            >
              {AGES.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </span>
        </label>

        <label className="az-field az-field--ext">
          <span className="az-field__label">Extension</span>
          <input
            className="input az-field__input"
            value={filters.extension}
            onChange={(event) => update("extension", event.target.value.replace(/^\./, ""))}
            placeholder="e.g. mp4"
            aria-label="File extension"
            spellCheck={false}
            autoComplete="off"
          />
        </label>

        <label className="az-field">
          <span className="az-field__label">Sort</span>
          <span className="az-field__sort">
            <span className="select">
              <select
                aria-label="Sort field"
                value={filters.sortField}
                onChange={(event) => update("sortField", event.target.value as AnalyzerSortField)}
              >
                {SORTS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </span>
            <button
              type="button"
              className="az-sort-dir"
              aria-label={filters.sortDir === "desc" ? "Sort descending" : "Sort ascending"}
              title={filters.sortDir === "desc" ? "Descending" : "Ascending"}
              onClick={() => update("sortDir", filters.sortDir === "desc" ? "asc" : "desc")}
            >
              {filters.sortDir === "desc" ? "↓" : "↑"}
            </button>
          </span>
        </label>

        <Button
          size="sm"
          variant="ghost"
          className="az-toolbar__clear"
          disabled={active === 0}
          onClick={() => onChange({ ...defaultFilters, sortField: filters.sortField, sortDir: filters.sortDir })}
        >
          {active > 0 ? `Clear ${active}` : "Clear"}
        </Button>
      </div>

      <div className="az-toolbar__meta">
        <button
          type="button"
          className="az-toolbar__tree-toggle"
          aria-pressed={treeOpen}
          onClick={onToggleTree}
        >
          {treeOpen ? "Hide tree" : "Show tree"}
        </button>
        <span
          className={`az-method az-method--${method === "mft" ? "mft" : "directory"}`}
          title={methodNote ?? undefined}
        >
          {method === "mft" ? "MFT" : "Directory walk"}
        </span>
        {methodNote && (
          <span className="az-toolbar__note" title={methodNote}>
            {methodNote}
          </span>
        )}
        <span className="az-toolbar__count">
          {active > 0
            ? `${formatCount(visible.files)} of ${formatCount(total.files)} files · ${formatBytes(visible.bytes)} of ${formatBytes(total.bytes)}`
            : `${formatCount(total.files)} files · ${formatBytes(total.bytes)}`}
        </span>
      </div>

      {active > 0 && (
        <p className="az-toolbar__hint">
          Filters apply to the folder tree and the Largest/Types lists. The treemap always shows
          unfiltered measured sizes.
        </p>
      )}
    </div>
  );
}
