import { useCallback, useEffect, useRef, useState } from "react";
import {
  Badge,
  Banner,
  Button,
  EmptyState,
  Panel,
  Spinner,
  ViewHeader,
} from "../../components/ui";
import {
  IconClose,
  IconFile,
  IconFolder,
  IconRefresh,
  IconSearch,
} from "../../components/Icons";
import { api, errorMessage } from "../../lib/api";
import { formatBytes, formatCount, formatDateTime } from "../../lib/format";
import type { EverythingItem, EverythingPage, EverythingStatus } from "../../lib/types";
import "./search.css";

const PAGE_SIZE = 100;
const QUERY_LIMIT = 256;
const DEBOUNCE_MS = 280;

type StatusPhase = "loading" | "ready" | "error";

function resultSummary(page: EverythingPage): string {
  if (page.items.length === 0) return "No results";
  const first = page.offset + 1;
  const last = page.offset + page.items.length;
  return `${formatCount(first)}–${formatCount(last)} of ${formatCount(page.total)}`;
}

export function Search() {
  const [status, setStatus] = useState<EverythingStatus | null>(null);
  const [statusPhase, setStatusPhase] = useState<StatusPhase>("loading");
  const [statusError, setStatusError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [searchNonce, setSearchNonce] = useState(0);
  const [page, setPage] = useState<EverythingPage | null>(null);
  const [searching, setSearching] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [revealError, setRevealError] = useState<string | null>(null);
  const [revealing, setRevealing] = useState<string | null>(null);

  const mountedRef = useRef(true);
  const requestRef = useRef(0);
  const revealingRef = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const loadStatus = useCallback(async () => {
    setStatusPhase("loading");
    setStatusError(null);
    try {
      const next = await api.everythingStatus();
      if (!mountedRef.current) return;
      setStatus(next);
      setStatusPhase("ready");
    } catch (error) {
      if (!mountedRef.current) return;
      setStatus(null);
      setStatusError(errorMessage(error));
      setStatusPhase("error");
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void loadStatus();
    return () => {
      mountedRef.current = false;
      requestRef.current += 1;
    };
  }, [loadStatus]);

  const runSearch = useCallback((term: string) => {
    const token = requestRef.current + 1;
    requestRef.current = token;
    setSearching(true);
    setLoadingMore(false);
    setSearchError(null);
    setRevealError(null);
    setPage(null);

    api
      .searchEverything(term, 0, PAGE_SIZE)
      .then((next) => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setPage(next);
      })
      .catch((error) => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setPage(null);
        setSearchError(errorMessage(error));
      })
      .finally(() => {
        if (!mountedRef.current || requestRef.current !== token) return;
        setSearching(false);
      });
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      requestRef.current += 1;
      setSubmitted("");
      setPage(null);
      setSearching(false);
      setLoadingMore(false);
      setSearchError(null);
      return;
    }
    const timer = window.setTimeout(() => setSubmitted(trimmed), DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    if (!submitted || !status?.available) return;
    runSearch(submitted);
  }, [submitted, searchNonce, status?.available, runSearch]);

  const submit = () => {
    const trimmed = query.trim();
    if (!trimmed || !status?.available || searching) return;
    setQuery(trimmed);
    setSubmitted(trimmed);
    setSearchNonce((value) => value + 1);
  };

  const clearQuery = () => {
    requestRef.current += 1;
    setQuery("");
    setSubmitted("");
    setPage(null);
    setSearching(false);
    setLoadingMore(false);
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
      const next = await api.searchEverything(submitted, page.items.length, PAGE_SIZE);
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

  const reveal = async (item: EverythingItem) => {
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

  return (
    <div>
      <ViewHeader
        title="Search"
        description="Find files and folders across every drive indexed by Everything. Queries stay on this PC: Cyron reads the local index over Windows IPC and never adds, changes or deletes anything."
        meta={<Badge tone="neutral">Read-only</Badge>}
        actions={
          <Button
            icon={<IconRefresh size={14} />}
            loading={statusPhase === "loading"}
            onClick={() => void loadStatus()}
          >
            Check Everything
          </Button>
        }
      />

      <div className="view-body">
        {statusPhase === "loading" && !status && (
          <Panel>
            <div className="search-page__checking" role="status">
              <Spinner size={18} />
              <div>
                <strong>Checking the local Everything index</strong>
                <p>Cyron only reads Everything's index; it does not start or configure it.</p>
              </div>
            </div>
          </Panel>
        )}

        {statusPhase === "error" && (
          <Banner
            tone="danger"
            actions={
              <Button size="sm" onClick={() => void loadStatus()}>
                Retry
              </Button>
            }
          >
            <strong>Cyron could not check Everything.</strong> {statusError}
          </Banner>
        )}

        {status && !status.available && (
          <Panel
            title="Search needs Everything"
            subtitle="Cyron does not install, start or configure third-party software."
          >
            <EmptyState
              compact
              icon={IconSearch}
              title="Everything 1.4.1 or newer is not available"
              description={status.message}
              action={
                <Button onClick={() => void loadStatus()} loading={statusPhase === "loading"}>
                  Check again
                </Button>
              }
            />
            <ol className="search-page__steps">
              <li>
                <strong>Install Everything 1.4.1 or newer</strong>
                <span>
                  Download it from voidtools.com. Cyron does not bundle the indexer.
                </span>
              </li>
              <li>
                <strong>Keep Everything running</strong>
                <span>
                  Its search index is served while the Everything client is open, including when
                  its window is closed to the tray.
                </span>
              </li>
              <li>
                <strong>Let the index finish loading</strong>
                <span>
                  The first index build can take a while. Cyron never triggers a rebuild or changes
                  Everything settings.
                </span>
              </li>
            </ol>
          </Panel>
        )}

        {status?.available && (
          <>
            <Panel className="search-page__panel">
              <form
                className="search-page__form"
                role="search"
                onSubmit={(event) => {
                  event.preventDefault();
                  submit();
                }}
              >
                <div className="search search-page__input">
                  <IconSearch size={15} className="search__icon" />
                  <input
                    ref={inputRef}
                    className="search__input"
                    type="search"
                    value={query}
                    maxLength={QUERY_LIMIT}
                    autoFocus
                    aria-label="Search files and folders in the Everything index"
                    placeholder="Search names or paths, e.g. report ext:pdf"
                    onChange={(event) => setQuery(event.target.value)}
                  />
                  {query && (
                    <button
                      type="button"
                      className="icon-btn search__clear"
                      aria-label="Clear search"
                      onClick={clearQuery}
                    >
                      <IconClose size={13} />
                    </button>
                  )}
                </div>
                <Button
                  type="submit"
                  variant="primary"
                  disabled={!query.trim() || searching}
                  loading={searching}
                >
                  Search
                </Button>
              </form>

              <div className="search-page__status">
                <div className="search-page__status-badges">
                  <Badge tone="accent">
                    {status.version ? `Everything ${status.version}` : "Everything"}
                  </Badge>
                  <Badge tone={status.dbLoaded ? "safe" : "caution"}>
                    {status.dbLoaded ? "Index loaded" : "Index loading"}
                  </Badge>
                  {status.dbBusy && <Badge tone="caution">Index updating</Badge>}
                </div>
                <p className="search-page__status-note">{status.message}</p>
                {status.dbBusy && (
                  <p className="search-page__status-note">
                    Results can be incomplete while Everything updates its index.
                  </p>
                )}
              </div>
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
                <strong>Could not reveal that item.</strong> {revealError}
              </Banner>
            )}

            {!submitted && (
              <Panel>
                <EmptyState
                  icon={IconSearch}
                  title="Search the Everything index"
                  description="Type a file or folder name. Everything matches filenames by default; its syntax such as ext:pdf, folder: or a path fragment narrows the result."
                />
              </Panel>
            )}

            {searching && (
              <Panel>
                <div className="search-page__searching" role="status">
                  <Spinner size={18} />
                  <span>Searching Everything…</span>
                </div>
              </Panel>
            )}

            {!searching && page && page.items.length === 0 && !searchError && (
              <Panel>
                <EmptyState
                  icon={IconSearch}
                  title={`No matches for “${submitted}”`}
                  description="Try a shorter name, check the spelling, or use Everything syntax such as ext:pdf or a folder path. A drive may still be indexing."
                />
              </Panel>
            )}

            {!searching && page && page.items.length > 0 && (
              <Panel
                title={
                  <span className="search-page__result-title">
                    Results <span>{resultSummary(page)}</span>
                  </span>
                }
                subtitle={
                  page.elapsedMs > 0 ? `Answered by the local index in ${page.elapsedMs} ms.` : undefined
                }
                flush
              >
                <ul className="search-page__results" aria-label="Everything search results">
                  {page.items.map((item) => (
                    <li key={`${item.fullPath}:${item.isFolder ? "folder" : "file"}`}>
                      <button
                        type="button"
                        className="search-page__result"
                        onClick={() => void reveal(item)}
                        disabled={revealing === item.fullPath}
                        title={item.fullPath}
                      >
                        <span className="search-page__result-icon" aria-hidden="true">
                          {item.isFolder ? <IconFolder size={17} /> : <IconFile size={17} />}
                        </span>
                        <span className="search-page__result-main">
                          <span className="search-page__result-name">{item.name}</span>
                          <span className="search-page__result-path">
                            {item.path || item.fullPath}
                          </span>
                        </span>
                        <span className="search-page__result-size">
                          {item.isFolder ? "Folder" : formatBytes(item.size)}
                        </span>
                        <span className="search-page__result-date">
                          {formatDateTime(item.modified)}
                        </span>
                        <span className="search-page__result-action" aria-hidden="true">
                          {revealing === item.fullPath ? <Spinner size={13} /> : "Reveal"}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>

                <div className="search-page__more">
                  <span>
                    {formatCount(page.items.length)} of {formatCount(page.total)} shown
                  </span>
                  {page.hasMore && (
                    <Button onClick={() => void loadMore()} loading={loadingMore} disabled={searching}>
                      Load more results
                    </Button>
                  )}
                </div>
              </Panel>
            )}
          </>
        )}
      </div>
    </div>
  );
}
