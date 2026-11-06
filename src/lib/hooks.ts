import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "./api";

export type AsyncStatus = "idle" | "loading" | "ready" | "error";

export interface AsyncResult<T> {
  status: AsyncStatus;
  data: T | null;
  error: string | null;
  reload: () => void;
}

/** Runs an async loader on mount and whenever deps change, discarding stale results. */
export function useAsync<T>(
  loader: () => Promise<T>,
  deps: readonly unknown[],
): AsyncResult<T> {
  const loaderRef = useRef(loader);
  loaderRef.current = loader;
  const requestRef = useRef(0);
  const [state, setState] = useState<Omit<AsyncResult<T>, "reload">>({
    status: "loading",
    data: null,
    error: null,
  });
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    const request = requestRef.current + 1;
    requestRef.current = request;
    const isCurrent = () => requestRef.current === request;

    setState((previous) => ({ ...previous, status: "loading", error: null }));
    // Defer invocation so synchronous throws follow the same error path as a
    // rejected promise instead of escaping the effect and leaving `loading` stuck.
    Promise.resolve().then(() => loaderRef.current()).then(
      (data) => {
        if (isCurrent()) setState({ status: "ready", data, error: null });
      },
      (error: unknown) => {
        if (isCurrent()) {
          setState({ status: "error", data: null, error: errorMessage(error) });
        }
      },
    );

    return () => {
      // Invalidate this request so a late result cannot overwrite newer state or
      // call setState after the component unmounted.
      if (requestRef.current === request) requestRef.current = request + 1;
    };
    // The caller owns the dependency list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, nonce]);

  const reload = useCallback(() => setNonce((value) => value + 1), []);
  return { ...state, reload };
}

export function useDebouncedValue<T>(value: T, delay = 200): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delay);
    return () => window.clearTimeout(timer);
  }, [value, delay]);
  return debounced;
}

/** Closes on Escape while `open` is true. */
export function useEscape(open: boolean, onClose: () => void): void {
  const handler = useRef(onClose);
  handler.current = onClose;
  useEffect(() => {
    if (!open) return;
    const listener = (event: KeyboardEvent) => {
      if (event.key === "Escape") handler.current();
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [open]);
}
