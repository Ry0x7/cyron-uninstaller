import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  IconButton,
  Panel,
  ProgressBar,
  Segmented,
  Spinner,
  ViewHeader,
  cx,
} from "../../components/ui";
import {
  IconAlert,
  IconChevronDown,
  IconChevronUp,
  IconClose,
  IconCopy,
  IconDownload,
  IconFile,
  IconFolder,
  IconFolderOpen,
  IconRefresh,
  IconSearch,
  IconStop,
} from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import {
  formatBytes,
  formatCount,
  formatDateTime,
  formatDuration,
  relativeTime,
  truncateMiddle,
} from "../../lib/format";
import type {
  SearchExportFormat,
  SearchFilter,
  SearchIndexItem,
  SearchIndexPage,
  SearchIndexStatus,
  SearchQueryOptions,
  SearchSortField,
} from "../../lib/types";
import {
  checkedBytes,
  checkedPaths,
  copyPaths,
  selectAll,
  selectRange,
  toggleChecked,
} from "./selection";
import { SearchSelectionBar } from "./SearchSelectionBar";
import "./search.css";

const PAGE_SIZE = 100;
const QUERY_LIMIT = 512;
const DEBOUNCE_MS = 140;
const STATUS_POLL_MS = 700;
const RECENTS_KEY = "cyron.search.recents";
const RECENTS_LIMIT = 8;

const FILTERS: { value: SearchFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "files", label: "Files" },
  { value: "folders", label: "Folders" },
  { value: "audio", label: "Audio" },
  { value: "video", label: "Video" },
  { value: "image", label: "Images" },
  { value: "document", label: "Documents" },
  { value: "executable", label: "Executables" },
  { value: "compressed", label: "Compressed" },
];

const COLUMNS: { field: SearchSortField; label: string; className: string }[] = [
  { field: "name", label: "Name", className: "search-page__col-name" },
  { field: "path", label: "Path", className: "search-page__col-path" },
  { field: "size", label: "Size", className: "search-page__col-size" },
  { field: "modified", label: "Modified", className: "search-page__col-date" },
];

const EXAMPLES: { label: string; query: string }[] = [
  { label: "PDFs from this week", query: "ext:pdf dm:thisweek" },
  { label: "Files over 1 GB", query: "size:>1gb" },
  { label: "node_modules folders", query: "folder: node_modules" },
  { label: "Design files", query: "ext:psd,ai,sketch" },
];

const SYNTAX: { code: string; description: string }[] = [
  { code: "alpha beta", description: "Both terms must match (AND is the default)." },
  { code: "alpha | beta", description: "Either term matches." },
  { code: "!alpha", description: "Exclude names that contain alpha." },
  { code: '"exact phrase"', description: "Match the phrase as typed." },
  { code: "* and ?", description: "Wildcards for any run of characters, or one character." },
  { code: "ext:pdf,png", description: "Match one of several file extensions." },
  { code: "file: / folder:", description: "Only files, or only folders." },
  { code: "size:>1gb", description: "Compare size; ranges use 1mb..10mb, units b, kb, mb, gb, tb." },
  { code: "dm:thisweek", description: "Modified date: today, yesterday, thisweek, lastmonth, thisyear, 2026-01-31, ranges with .., comparisons with > or <." },
  { code: "dc: / da:", description: "Created and accessed dates, same values as dm:." },
  { code: "type:video", description: "Built-in kinds: audio, video, image, document, executable, compressed." },
  { code: "path:src", description: "Match the full path instead of only the name." },
  { code: "attrib:h", description: "Files with an attribute such as h, s, r, a or d; prefix - to exclude it." },
  { code: "dupe:", description: "Keep only names that appear more than once in the matches." },
  { code: "regex:^log-\\d+", description: "Treat the rest of the term as a regular expression." },
];

type MatchOptions = {
  matchCase: boolean;
  wholeWord: boolean;
  matchPath: boolean;
  regex: boolean;
};

const TOGGLES: { key: keyof MatchOptions; label: string; title: string }[] = [
  { key: "matchCase", label: "Match case", title: "Match letter case exactly" },
  { key: "wholeWord", label: "Whole word", title: "Match complete words only" },
  { key: "matchPath", label: "Match path", title: "Match the full path as well as the name" },
  { key: "regex", label: "Regex", title: "Treat terms as regular expressions" },
];

function readRecents(): string[] {
  try {
    const raw = window.localStorage.getItem(RECENTS_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((value): value is string => typeof value === "string" && value.trim().length > 0)
      .slice(0, RECENTS_LIMIT);
  } catch {
    return [];
  }
}

function resultSummary(page: SearchIndexPage): string {
  if (page.items.length === 0) return "No results";
  const first = page.offset + 1;
  const last = page.offset + page.items.length;
  return `${formatCount(first)}–${formatCount(last)} of ${formatCount(page.total)}`;
}

function statusCounts(status: SearchIndexStatus): string {
  return `${formatCount(status.entries)} entries · ${formatCount(status.files)} files · ${formatCount(
    status.folders,
  )} folders · ${formatBytes(status.totalSize)}`;
}

function SyntaxReference() {
  return (
    <details className="search-page__syntax">
      <summary>
        <span>Search syntax</span>
        <IconChevronDown size={14} className="search-page__syntax-chevron" />
      </summary>
      <dl className="search-page__syntax-grid">
        {SYNTAX.map((row) => (
          <div className="search-page__syntax-row" key={row.code}>
            <dt>
              <code>{row.code}</code>
            </dt>
            <dd>{row.description}</dd>
          </div>
        ))}
      </dl>
      <p className="search-page__syntax-note">
        Combine terms with spaces (AND) or <code>|</code> (OR), and wrap phrases in quotes. Filters
        can sit beside a name, for example <code>report ext:pdf dm:lastmonth</code>.
      </p>
    </details>
  );
}

export function Search() {
  const [status, setStatus] = useState<SearchIndexStatus | null>(null);
  const [statusLoading, setStatusLoading] = useState(true);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [indexBusy, setIndexBusy] = useState(false);

  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [searchNonce, setSearchNonce] = useState(0);
  const [options, setOptions] = useState<MatchOptions>({
    matchCase: false,
    wholeWord: false,
    matchPath: false,
    regex: false,
  });
  const [filter, setFilter] = useState<SearchFilter>("all");
  const [sort, setSort] = useState<{ field: SearchSortField; descending: boolean }>({
    field: "name",
    descending: false,
  });

  const [page, setPage] = useState<SearchIndexPage | null>(null);
  const [searching, setSearching] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [revealing, setRevealing] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [checked, setChecked] = useState<Set<string>>(() => new Set());
  const [exporting, setExporting] = useState<SearchExportFormat | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [recents, setRecents] = useState<string[]>(readRecents);

  const mountedRef = useRef(true);
  const requestRef = useRef(0);
  const revealingRef = useRef(false);
  const noticeTimer = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const headerCheckRef = useRef<HTMLInputElement>(null);
  const anchorRef = useRef<string | null>(null);
  const exportingRef = useRef(false);

  const loadStatus = useCallback(async () => {
    try {
      const next = await api.searchIndexStatus();
      if (!mountedRef.current) return;
      setStatus(next);
      setStatusError(null);
    } catch (error) {
      if (mountedRef.current) setStatusError(errorMessage(error));
    } finally {
      if (mountedRef.current) setStatusLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void loadStatus();
    return () => {
      mountedRef.current = false;
      requestRef.current += 1;
      if (noticeTimer.current !== null) window.clearTimeout(noticeTimer.current);
    };
  }, [loadStatus]);

  const pollable = status?.state === "loading" || status?.state === "building";
  useEffect(() => {
    if (!pollable) return;
    const timer = window.setInterval(() => void loadStatus(), STATUS_POLL_MS);
    return () => window.clearInterval(timer);
  }, [pollable, loadStatus]);

  // `ready` is true while a rebuild runs only if a previous index is still
  // searchable; an initial build has no entries yet, so its controls stay off.
  const searchable = status?.ready === true;
  const staleIndex =
    status?.indexedAt != null && Date.now() / 1000 - status.indexedAt > 24 * 60 * 60;

  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      requestRef.current += 1;
      setSubmitted("");
      setPage(null);
      setSelected(null);
      setChecked(new Set());
      anchorRef.current = null;
      setSearching(false);
      setLoadingMore(false);
      setSearchError(null);
      return;
    }
    if (trimmed === submitted) return;
    const timer = window.setTimeout(() => {
      setSubmitted(trimmed);
      setSelected(null);
    }, DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query, submitted]);

  useEffect(() => {
    if (!submitted || !searchable) {
      setSearching(false);
      return;
    }
    const token = requestRef.current + 1;
    requestRef.current = token;
    setSearching(true);
    setChecked(new Set());
    anchorRef.current = null;
    setSearchError(null);
    setRevealError(null);
    const request: SearchQueryOptions = {
      query: submitted,
      ...options,
      filter,
      sort: sort.field,
      descending: sort.descending,
      offset: 0,
      limit: PAGE_SIZE,
    };
    api
      .querySearchIndex(request)
      .then((next) => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setPage(next);
        setSelected(next.items[0]?.fullPath ?? null);
      })
      .catch((error) => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setSearchError(errorMessage(error));
      })
      .finally(() => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setSearching(false);
      });
  }, [
    submitted,
    searchNonce,
    searchable,
    options.matchCase,
    options.wholeWord,
    options.matchPath,
    options.regex,
    filter,
    sort.field,
    sort.descending,
  ]);

  useEffect(() => {
    if (!selected) return;
    document
      .querySelector<HTMLElement>('[data-search-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const remember = useCallback((term: string) => {
    setRecents((current) => {
      const next = [term, ...current.filter((value) => value !== term)].slice(0, RECENTS_LIMIT);
      try {
        window.localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      } catch {
        // A disabled or full localStorage must not block searching.
      }
      return next;
    });
  }, []);

  const clearRecents = () => {
    setRecents([]);
    try {
      window.localStorage.removeItem(RECENTS_KEY);
    } catch {
      // Ignore storage failures; the in-memory list is already cleared.
    }
  };

  const submit = useCallback(
    (term?: string) => {
      const trimmed = (term ?? query).trim();
      if (!trimmed || !searchable) return;
      setQuery(trimmed);
      setSubmitted(trimmed);
      setSearchNonce((value) => value + 1);
      setSelected(null);
      remember(trimmed);
      inputRef.current?.focus();
    },
    [query, searchable, remember],
  );

  const clearQuery = () => {
    requestRef.current += 1;
    setQuery("");
    setSubmitted("");
    setPage(null);
    setSelected(null);
    setChecked(new Set());
    anchorRef.current = null;
    setSearching(false);
    setSearchError(null);
    setRevealError(null);
    inputRef.current?.focus();
  };

  const loadMore = async () => {
    if (!page || !page.hasMore || page.items.length === 0 || loadingMore || searching) return;
    const token = requestRef.current;
    setLoadingMore(true);
    setSearchError(null);
    try {
      const next = await api.querySearchIndex({
        query: submitted,
        ...options,
        filter,
        sort: sort.field,
        descending: sort.descending,
        offset: page.items.length,
        limit: PAGE_SIZE,
      });
      if (!mountedRef.current || requestRef.current !== token) return;
      setPage((current) =>
        current
          ? {
              ...next,
              offset: current.offset,
              items: [...current.items, ...next.items],
            }
          : next,
      );
    } catch (error) {
      if (mountedRef.current && requestRef.current === token) {
        setSearchError(errorMessage(error));
      }
    } finally {
      if (mountedRef.current) setLoadingMore(false);
    }
  };

  const reveal = async (item: SearchIndexItem) => {
    if (revealingRef.current) return;
    revealingRef.current = true;
    setRevealError(null);
    setRevealing(item.fullPath);
    try {
      await api.revealPath(item.fullPath);
    } catch (error) {
      if (mountedRef.current) setRevealError(errorMessage(error));
    } finally {
      revealingRef.current = false;
      if (mountedRef.current) setRevealing(null);
    }
  };

  const copyPath = async (item: SearchIndexItem) => {
    setRevealError(null);
    try {
      await navigator.clipboard.writeText(item.fullPath);
      if (!mountedRef.current) return;
      setNotice("Path copied to the clipboard.");
      if (noticeTimer.current !== null) window.clearTimeout(noticeTimer.current);
      noticeTimer.current = window.setTimeout(() => {
        if (mountedRef.current) setNotice(null);
      }, 2200);
    } catch {
      if (mountedRef.current) {
        setRevealError("Windows did not allow Cyron to write to the clipboard.");
      }
    }
  };

  const startBuild = async () => {
    setIndexBusy(true);
    setStatusError(null);
    try {
      const next = await api.buildSearchIndex();
      if (!mountedRef.current) return;
      requestRef.current += 1;
      setStatus(next);
      setSubmitted("");
      setPage(null);
      setSelected(null);
      setChecked(new Set());
      anchorRef.current = null;
      setSearching(false);
      setLoadingMore(false);
      setSearchError(null);
    } catch (error) {
      if (mountedRef.current) setStatusError(errorMessage(error));
    } finally {
      if (mountedRef.current) setIndexBusy(false);
    }
  };

  const cancelBuild = async () => {
    setIndexBusy(true);
    try {
      const next = await api.cancelSearchIndexBuild();
      if (!mountedRef.current) return;
      setStatus(next);
    } catch (error) {
      if (mountedRef.current) setStatusError(errorMessage(error));
    } finally {
      if (mountedRef.current) setIndexBusy(false);
    }
    void loadStatus();
  };

  const changeFilter = (next: SearchFilter) => {
    setFilter(next);
    setSelected(null);
  };

  const changeSort = (field: SearchSortField) => {
    setSort((current) =>
      current.field === field
        ? { field, descending: !current.descending }
        : { field, descending: false },
    );
    setSelected(null);
  };

  const moveSelection = (delta: number, extend = false) => {
    const items = page?.items ?? [];
    if (items.length === 0) return;
    const index = items.findIndex((item) => item.fullPath === selected);
    const nextIndex = index < 0 ? 0 : Math.min(Math.max(index + delta, 0), items.length - 1);
    const nextPath = items[nextIndex].fullPath;
    setSelected(nextPath);
    if (extend) {
      const anchor = anchorRef.current ?? items[Math.max(index, 0)].fullPath;
      setChecked(selectRange(items.map((item) => item.fullPath), anchor, nextPath));
      anchorRef.current = anchor;
    }
  };

  const handleTableKeys = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveSelection(1, event.shiftKey);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveSelection(-1, event.shiftKey);
    } else if (event.key === "Enter") {
      const item = page?.items.find((entry) => entry.fullPath === selected);
      if (item) {
        event.preventDefault();
        void reveal(item);
      }
    } else if (event.key === " ") {
      // Space toggles the current row; a focused checkbox already handles its
      // own Space press, so leave inputs and buttons to the browser.
      const target = event.target as HTMLElement;
      if (target.closest("input, button, a")) return;
      const item = page?.items.find((entry) => entry.fullPath === selected);
      if (item) {
        event.preventDefault();
        toggleRowChecked(item, !checked.has(item.fullPath));
      }
    }
  };

  const loadedItems = page?.items ?? [];
  const loadedPaths = loadedItems.map((item) => item.fullPath);
  const checkedCount = checked.size;
  const allLoadedChecked =
    loadedItems.length > 0 && loadedItems.every((item) => checked.has(item.fullPath));

  useEffect(() => {
    if (headerCheckRef.current) {
      headerCheckRef.current.indeterminate = checkedCount > 0 && !allLoadedChecked;
    }
  }, [checkedCount, allLoadedChecked]);

  const showNotice = useCallback((message: string) => {
    setNotice(message);
    if (noticeTimer.current !== null) window.clearTimeout(noticeTimer.current);
    noticeTimer.current = window.setTimeout(() => {
      if (mountedRef.current) setNotice(null);
    }, 2600);
  }, []);

  const toggleRowChecked = (item: SearchIndexItem, value: boolean) => {
    setSelected(item.fullPath);
    setChecked((current) => toggleChecked(current, item.fullPath, value));
    anchorRef.current = item.fullPath;
  };

  const toggleAllLoaded = () => {
    if (allLoadedChecked) {
      setChecked(new Set());
      anchorRef.current = null;
    } else {
      setChecked(selectAll(loadedPaths));
      anchorRef.current = loadedPaths[0] ?? null;
    }
  };

  const clearChecked = () => {
    setChecked(new Set());
    anchorRef.current = null;
  };

  const selectRow = (event: MouseEvent<HTMLTableRowElement>, item: SearchIndexItem) => {
    setSelected(item.fullPath);
    if (event.shiftKey && anchorRef.current) {
      setChecked(selectRange(loadedPaths, anchorRef.current, item.fullPath));
    } else if (event.ctrlKey || event.metaKey) {
      setChecked((current) => toggleChecked(current, item.fullPath));
    }
    if (!event.shiftKey) anchorRef.current = item.fullPath;
  };

  const copySelectedPaths = async () => {
    const paths = checkedCount > 0 ? checkedPaths(loadedItems, checked) : selected ? [selected] : [];
    if (paths.length === 0) return;
    setRevealError(null);
    try {
      await navigator.clipboard.writeText(copyPaths(paths));
      if (!mountedRef.current) return;
      showNotice(paths.length === 1 ? "Path copied to the clipboard." : `${paths.length} paths copied.`);
    } catch {
      if (mountedRef.current) {
        setRevealError("Windows did not allow Cyron to write to the clipboard.");
      }
    }
  };

  const exportResults = async (format: SearchExportFormat, useSelection = false) => {
    if (!submitted || !page || exportingRef.current) return;
    exportingRef.current = true;
    const selectedPaths =
      useSelection && checkedCount > 0 ? checkedPaths(loadedItems, checked) : undefined;
    const defaultPath = `cyron-search-${new Date().toISOString().slice(0, 10)}.${format}`;
    setRevealError(null);
    try {
      const destination = await save({
        defaultPath,
        filters: [
          {
            name: format === "csv" ? "CSV spreadsheet" : "Plain text",
            extensions: [format],
          },
        ],
      });
      if (!destination || !mountedRef.current) return;
      setExporting(format);
      const request: SearchQueryOptions = {
        query: submitted,
        ...options,
        filter,
        sort: sort.field,
        descending: sort.descending,
        offset: 0,
        limit: PAGE_SIZE,
      };
      const report = await api.exportSearchResults(
        request,
        format,
        destination,
        selectedPaths,
      );
      if (!mountedRef.current) return;
      const skipped = report.skipped > 0 ? ` · ${formatCount(report.skipped)} skipped` : "";
      const truncated = report.truncated ? " · capped at 1,000,000 rows" : "";
      showNotice(`Exported ${formatCount(report.rows)} rows to ${truncateMiddle(report.path, 56)}${skipped}${truncated}`);
    } catch (error) {
      if (mountedRef.current) setRevealError(errorMessage(error));
    } finally {
      exportingRef.current = false;
      if (mountedRef.current) setExporting(null);
    }
  };

  const handleTableShortcuts = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      clearChecked();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      setChecked(selectAll(loadedPaths));
      anchorRef.current = loadedPaths[0] ?? null;
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c") {
      event.preventDefault();
      void copySelectedPaths();
    }
  };

  const rebuildLabel = status?.state === "empty" ? "Build index" : "Rebuild index";

  return (
    <div>
      <ViewHeader
        title="Search"
        description="Instant filename search across every fixed drive, powered by Cyron's own local index. No Everything install and no network: the index stores names, paths, sizes and dates, never file contents."
        meta={<Badge tone="safe">Local index</Badge>}
        actions={
          <Button
            icon={<IconRefresh size={14} />}
            loading={indexBusy}
            disabled={!status || statusLoading || status.state === "loading" || status.building}
            onClick={() => void startBuild()}
          >
            {rebuildLabel}
          </Button>
        }
      />

      <div className="view-body">
        {statusLoading && !status && (
          <Panel>
            <div className="search-page__checking" role="status">
              <Spinner size={18} />
              <div>
                <strong>Starting the local index</strong>
                <p>Cyron is checking for a saved index. Nothing leaves this PC.</p>
              </div>
            </div>
          </Panel>
        )}

        {statusError && !status && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={() => void loadStatus()}>
                Retry
              </Button>
            }
          >
            <strong>Cyron could not load the search index.</strong> {statusError}
          </Banner>
        )}

        {status?.state === "empty" && (
          <Panel>
            <EmptyState
              icon={IconSearch}
              title="Build Cyron's local file index"
              description="Cyron walks every fixed drive once and stores file and folder names, paths, sizes and timestamps in a local index. Search is then instant and stays offline; file contents are never read and nothing on disk is changed."
              action={
                <Button
                  variant="primary"
                  icon={<IconSearch size={14} />}
                  loading={indexBusy}
                  onClick={() => void startBuild()}
                >
                  Build index
                </Button>
              }
            />
            <ul className="search-page__scope">
              <li>
                <strong>Fixed drives only</strong>
                <span>Removable and network drives are skipped so indexing never blocks on them.</span>
              </li>
              <li>
                <strong>Metadata, not contents</strong>
                <span>The index holds names, sizes and dates. Cyron never opens the files it finds.</span>
              </li>
              <li>
                <strong>Saved between sessions</strong>
                <span>Later launches reuse the index and keep it current with live file-system changes.</span>
              </li>
            </ul>
          </Panel>
        )}

        {status?.state === "loading" && (
          <Panel>
            <div className="search-page__checking" role="status">
              <Spinner size={18} />
              <div>
                <strong>Loading the saved index</strong>
                <p>{status.message || "The index is read from this PC and will be searchable in a moment."}</p>
              </div>
            </div>
          </Panel>
        )}

        {status?.state === "building" && (
          <Panel
            className="search-page__build"
            title={status.entries > 0 ? "Refreshing the local index" : "Indexing your drives"}
            subtitle={status.message}
            actions={
              <Button
                size="sm"
                icon={<IconStop size={13} />}
                loading={indexBusy}
                onClick={() => void cancelBuild()}
              >
                Cancel
              </Button>
            }
          >
            <ProgressBar label="Indexing fixed drives" />
            <div className="search-page__build-meta">
              <span>
                <strong>{formatCount(status.progress?.entries ?? status.entries)}</strong> entries
                indexed so far
              </span>
              {status.progress?.volume && (
                <Badge tone="neutral">{status.progress.volume}</Badge>
              )}
            </div>
            {status.progress?.currentDir && (
              <p className="search-page__build-path" title={status.progress.currentDir}>
                {truncateMiddle(status.progress.currentDir, 96)}
              </p>
            )}
            {status.volumes.length > 0 && (
              <div className="search-page__volumes">
                {status.volumes.map((volume) => (
                  <span className="search-page__volume" key={volume.mount}>
                    <span className="search-page__volume-mount">{volume.mount}</span>
                    <Badge
                      tone={
                        volume.state === "error"
                          ? "danger"
                          : volume.state === "done"
                            ? "safe"
                            : volume.state === "indexing"
                              ? "accent"
                              : "neutral"
                      }
                    >
                      {volume.state}
                    </Badge>
                  </span>
                ))}
              </div>
            )}
            <p className="search-page__build-note">
              {status.entries > 0
                ? "Search keeps using the previous index while this one builds; the new results appear when it finishes."
                : "The first build has to finish before search is available. It only reads names, sizes and dates."}
            </p>
          </Panel>
        )}

        {status?.state === "error" && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={() => void startBuild()} loading={indexBusy}>
                Build index
              </Button>
            }
          >
            <strong>The search index is unavailable.</strong> {status.message}
          </Banner>
        )}

        {status?.state === "ready" && (
          <div className="search-page__index-strip">
            <div className="search-page__index-main">
              <Badge tone="safe">Index ready</Badge>
              {status.watching && <Badge tone="accent">Live</Badge>}
              {staleIndex && <Badge tone="caution">May be stale</Badge>}
              <span>{statusCounts(status)}</span>
            </div>
            <span className="search-page__index-meta">
              Indexed {relativeTime(status.indexedAt)}
              {status.buildMs !== null ? ` · built in ${formatDuration(status.buildMs)}` : ""}
              {staleIndex ? " · rebuild to include changes made while Cyron was closed" : ""}
            </span>
          </div>
        )}

        {status && status.warnings.length > 0 && (
          <Banner tone="caution">
            <strong>Some folders could not be indexed.</strong>
            <ul className="search-page__warning-list">
              {status.warnings.slice(0, 4).map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
            {status.warnings.length > 4 && (
              <span>{formatCount(status.warnings.length - 4)} more warnings.</span>
            )}
          </Banner>
        )}

        {statusError && status && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={() => void loadStatus()}>
                Retry
              </Button>
            }
          >
            <strong>Index status could not be refreshed.</strong> {statusError}
          </Banner>
        )}

        {searchable && (
          <>
            <Panel className="search-page__console">
              <form
                className="search-page__form"
                role="search"
                onSubmit={(event) => {
                  event.preventDefault();
                  submit();
                }}
              >
                <div className="search-page__input-row">
                  <div className="search search-page__input">
                    <IconSearch size={15} className="search__icon" />
                    <input
                      ref={inputRef}
                      className="search__input"
                      type="search"
                      value={query}
                      maxLength={QUERY_LIMIT}
                      autoFocus
                      aria-label="Search indexed file and folder names"
                      placeholder="Search names, e.g. report ext:pdf dm:thisweek"
                      onChange={(event) => setQuery(event.target.value)}
                    />
                    {query && (
                      <IconButton
                        label="Clear search"
                        className="search__clear"
                        onClick={clearQuery}
                      >
                        <IconClose size={13} />
                      </IconButton>
                    )}
                  </div>
                  <Button
                    type="submit"
                    variant="primary"
                    disabled={!query.trim()}
                    loading={searching}
                  >
                    Search
                  </Button>
                </div>

                <div className="search-page__controls">
                  <Segmented
                    options={FILTERS}
                    value={filter}
                    onChange={changeFilter}
                    ariaLabel="Filter results by kind"
                    size="sm"
                  />
                  <div
                    className="search-page__toggles"
                    role="group"
                    aria-label="Search matching options"
                  >
                    {TOGGLES.map((toggle) => (
                      <button
                        key={toggle.key}
                        type="button"
                        className="search-page__toggle"
                        aria-pressed={options[toggle.key]}
                        title={toggle.title}
                        onClick={() =>
                          setOptions((current) => ({
                            ...current,
                            [toggle.key]: !current[toggle.key],
                          }))
                        }
                      >
                        {toggle.label}
                      </button>
                    ))}
                  </div>
                </div>
              </form>

              {recents.length > 0 && (
                <div className="search-page__recents">
                  <span className="search-page__recents-label">Recent</span>
                  {recents.map((term) => (
                    <button
                      key={term}
                      type="button"
                      className="search-page__recent"
                      title={term}
                      onClick={() => submit(term)}
                    >
                      {truncateMiddle(term, 36)}
                    </button>
                  ))}
                  <button
                    type="button"
                    className="search-page__recents-clear"
                    onClick={clearRecents}
                  >
                    Clear
                  </button>
                </div>
              )}
            </Panel>

            {searchError && (
              <Banner
                tone="danger"
                actions={
                  <Button
                    size="sm"
                    onClick={() => setSearchNonce((value) => value + 1)}
                    disabled={searching}
                  >
                    Retry
                  </Button>
                }
              >
                <strong>The search failed.</strong> {searchError}
              </Banner>
            )}

            {revealError && (
              <Banner tone="danger">
                <strong>That action did not complete.</strong> {revealError}
              </Banner>
            )}

            {notice && (
              <div className="search-page__notice" role="status">
                {notice}
              </div>
            )}

            {!submitted && (
              <Panel>
                <EmptyState
                  icon={IconSearch}
                  title="Search every indexed drive"
                  description="Type a file or folder name. Cyron matches names as you type, and Everything-style filters such as ext:pdf, size:>1gb or dm:thisweek narrow the results instantly."
                />
                <div className="search-page__examples">
                  <span className="search-page__examples-label">Try</span>
                  {EXAMPLES.map((example) => (
                    <button
                      key={example.query}
                      type="button"
                      className="search-page__example"
                      onClick={() => submit(example.query)}
                    >
                      {example.label}
                      <code>{example.query}</code>
                    </button>
                  ))}
                </div>
                <SyntaxReference />
              </Panel>
            )}

            {searching && !page && (
              <Panel>
                <div className="search-page__searching" role="status">
                  <Spinner size={18} />
                  <span>Searching the local index…</span>
                </div>
              </Panel>
            )}

            {searching && page && (
              <div className="search-page__updating" role="status">
                <Spinner size={13} />
                <span>Updating results…</span>
              </div>
            )}

            {!searching && submitted && page && page.items.length === 0 && !searchError && (
              <Panel>
                <EmptyState
                  icon={IconSearch}
                  title={`No matches for “${submitted}”`}
                  description="Try a shorter name, check the spelling, or use a filter such as ext:pdf or path:src. A folder that could not be read is listed as an index warning."
                />
                <SyntaxReference />
              </Panel>
            )}

            {page && page.items.length > 0 && (
              <Panel
                className="search-page__results"
                flush
                title={
                  <span className="search-page__results-title">
                    Results <span>{resultSummary(page)}</span>
                  </span>
                }
                subtitle={
                  page.elapsedMs > 0
                    ? `Answered from the local index in ${page.elapsedMs} ms.`
                    : undefined
                }
              >
                {checkedCount > 0 && (
                  <SearchSelectionBar
                    count={checkedCount}
                    bytes={checkedBytes(loadedItems, checked)}
                    busy={exporting}
                    onCopy={() => void copySelectedPaths()}
                    onExportCsv={() => void exportResults("csv", true)}
                    onExportTxt={() => void exportResults("txt", true)}
                    onClear={clearChecked}
                  />
                )}
                <div
                  className="search-page__table-wrap"
                  aria-busy={searching || undefined}
                  tabIndex={0}
                  onKeyDown={(event) => {
                    handleTableKeys(event);
                    handleTableShortcuts(event);
                  }}
                >
                  <table className="search-page__table" aria-label="Indexed files and folders">
                    <thead>
                      <tr>
                        <th className="search-page__col-check">
                          <input
                            ref={headerCheckRef}
                            type="checkbox"
                            className="search-page__check"
                            checked={allLoadedChecked}
                            aria-label="Select all loaded results"
                            onChange={toggleAllLoaded}
                          />
                        </th>
                        {COLUMNS.map((column) => {
                          const active = sort.field === column.field;
                          return (
                            <th
                              key={column.field}
                              className={column.className}
                              aria-sort={
                                active ? (sort.descending ? "descending" : "ascending") : "none"
                              }
                            >
                              <button
                                type="button"
                                className="search-page__sort"
                                onClick={() => changeSort(column.field)}
                                title={`Sort by ${column.label.toLowerCase()}`}
                              >
                                {column.label}
                                <span
                                  className={cx("search-page__sort-icon", active && "is-active")}
                                  aria-hidden="true"
                                >
                                  {active && sort.descending ? (
                                    <IconChevronDown size={12} />
                                  ) : (
                                    <IconChevronUp size={12} />
                                  )}
                                </span>
                              </button>
                            </th>
                          );
                        })}
                        <th className="search-page__col-actions">
                          <span className="search-page__sr-only">Actions</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {page.items.map((item) => {
                        const isSelected = item.fullPath === selected;
                        return (
                          <tr
                            key={item.fullPath}
                            className={cx(
                              "search-page__row",
                              isSelected && "is-selected",
                              checked.has(item.fullPath) && "is-checked",
                            )}
                            data-search-selected={isSelected ? "true" : undefined}
                            aria-current={isSelected ? "true" : undefined}
                            onClick={(event) => selectRow(event, item)}
                            onDoubleClick={() => void reveal(item)}
                          >
                            <td className="search-page__col-check">
                              <input
                                type="checkbox"
                                className="search-page__check"
                                checked={checked.has(item.fullPath)}
                                aria-label={`Select ${item.name}`}
                                onClick={(event) => event.stopPropagation()}
                                onChange={(event) => toggleRowChecked(item, event.target.checked)}
                              />
                            </td>
                            <td className="search-page__col-name">
                              <span className="search-page__name-cell">
                                <span
                                  className={cx(
                                    "search-page__kind",
                                    item.isFolder && "search-page__kind--folder",
                                  )}
                                  aria-hidden="true"
                                >
                                  {item.isFolder ? (
                                    <IconFolder size={16} />
                                  ) : (
                                    <IconFile size={16} />
                                  )}
                                </span>
                                <span className="search-page__name" title={item.fullPath}>
                                  {item.name}
                                </span>
                              </span>
                            </td>
                            <td className="search-page__col-path">
                              <span className="search-page__path" title={item.parentPath}>
                                {item.parentPath}
                              </span>
                            </td>
                            <td className="search-page__col-size">
                              {item.isFolder ? "Folder" : formatBytes(item.size)}
                            </td>
                            <td className="search-page__col-date">
                              {formatDateTime(item.modified)}
                            </td>
                            <td className="search-page__col-actions">
                              <span className="search-page__row-actions">
                                <IconButton
                                  label={`Reveal ${item.name} in File Explorer`}
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    void reveal(item);
                                  }}
                                  disabled={revealing === item.fullPath}
                                >
                                  {revealing === item.fullPath ? (
                                    <Spinner size={13} />
                                  ) : (
                                    <IconFolderOpen size={14} />
                                  )}
                                </IconButton>
                                <IconButton
                                  label={`Copy the full path of ${item.name}`}
                                  onClick={(event) => {
                                    event.stopPropagation();
                                    void copyPath(item);
                                  }}
                                >
                                  <IconCopy size={14} />
                                </IconButton>
                              </span>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                <footer className="search-page__footer">
                  <span className="search-page__footer-count">
                    <strong>{formatCount(page.total)}</strong>{" "}
                    {page.total === 1 ? "result" : "results"} · {formatCount(page.files)} files ·{" "}
                    {formatCount(page.folders)} folders · {formatBytes(page.totalSize)}
                  </span>
                  <span className="search-page__footer-meta">
                    {formatCount(page.items.length)} loaded · {page.elapsedMs} ms
                  </span>
                  <span className="search-page__footer-actions">
                    <Button
                      size="sm"
                      variant="ghost"
                      icon={<IconDownload size={13} />}
                      loading={exporting === "csv"}
                      disabled={exporting !== null}
                      aria-label="Export all matching rows as CSV"
                      onClick={() => void exportResults("csv")}
                    >
                      CSV
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      icon={<IconDownload size={13} />}
                      loading={exporting === "txt"}
                      disabled={exporting !== null}
                      aria-label="Export all matching rows as TXT"
                      onClick={() => void exportResults("txt")}
                    >
                      TXT
                    </Button>
                  </span>
                  {page.hasMore && (
                    <Button size="sm" onClick={() => void loadMore()} loading={loadingMore}>
                      Load more
                    </Button>
                  )}
                </footer>

                {page.truncated && (
                  <p className="search-page__truncated">
                    <IconAlert size={13} /> The index stopped at its entry cap, so results may be
                    incomplete. Rebuild after narrowing the indexed drives.
                  </p>
                )}
              </Panel>
            )}
          </>
        )}
      </div>
    </div>
  );
}
