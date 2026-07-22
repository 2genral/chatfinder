import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowsClockwise,
  Database,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { DetailPanel } from "./components/DetailPanel";
import { ResultList } from "./components/ResultList";
import { SourcePanel } from "./components/SourcePanel";
import { getStatus, launchTarget, searchSessions, syncIndex } from "./lib/api";
import type {
  AppStatus,
  IndexProgress,
  ProviderFilter,
  SearchResult,
} from "./types";
import "./App.css";

const EMPTY_STATUS: AppStatus = {
  indexedSessions: 0,
  indexedMessages: 0,
  sourceFiles: 0,
  sourceBytes: 0,
  lastIndexedAt: null,
  isIndexing: false,
  roots: [],
};

const FILTERS: Array<{ value: ProviderFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
];

function App() {
  const [query, setQuery] = useState("");
  const [provider, setProvider] = useState<ProviderFilter>("all");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [status, setStatus] = useState<AppStatus>(EMPTY_STATUS);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [showSources, setShowSources] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const searchSequence = useRef(0);
  const initialized = useRef(false);

  const selectedResult = results[selectedIndex] ?? null;
  const modifier = useMemo(
    () => (/Mac|iPhone|iPad/.test(navigator.userAgent) ? "⌘" : "Ctrl"),
    [],
  );

  const executeSearch = useCallback(async (nextQuery: string, nextProvider: ProviderFilter) => {
    const sequence = ++searchSequence.current;
    setIsSearching(true);
    try {
      const nextResults = await searchSessions(nextQuery, nextProvider);
      if (sequence !== searchSequence.current) return;
      setResults(nextResults);
      setSelectedIndex((current) => Math.min(current, Math.max(0, nextResults.length - 1)));
      setError(null);
    } catch (searchError) {
      if (sequence === searchSequence.current) {
        setError(String(searchError));
      }
    } finally {
      if (sequence === searchSequence.current) setIsSearching(false);
    }
  }, []);

  const refreshStatus = useCallback(async () => {
    const nextStatus = await getStatus();
    setStatus(nextStatus);
    return nextStatus;
  }, []);

  const runIndex = useCallback(async () => {
    setStatus((current) => ({ ...current, isIndexing: true }));
    setError(null);
    try {
      const summary = await syncIndex();
      const nextStatus = await refreshStatus();
      await executeSearch(query, provider);
      setNotice(`${summary.indexed} updated · ${nextStatus.indexedSessions} chats`);
    } catch (indexError) {
      const message = String(indexError);
      if (!message.toLowerCase().includes("already running")) setError(message);
      await refreshStatus().catch(() => undefined);
    }
  }, [executeSearch, provider, query, refreshStatus]);

  useEffect(() => {
    let unlisten: () => void = () => {};
    listen<IndexProgress>("index-progress", ({ payload }) => {
      setProgress(payload);
      setStatus((current) => ({ ...current, isIndexing: payload.phase !== "complete" }));
    }).then((cleanup) => {
      unlisten = cleanup;
    });
    return () => unlisten();
  }, []);

  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;
    void (async () => {
      try {
        const initialStatus = await refreshStatus();
        await executeSearch("", "all");
        if (initialStatus.indexedSessions === 0 && initialStatus.sourceFiles > 0) {
          await runIndex();
        }
      } catch (initialError) {
        setError(String(initialError));
      }
    })();
  }, [executeSearch, refreshStatus, runIndex]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void executeSearch(query, provider);
    }, 140);
    return () => window.clearTimeout(timeout);
  }, [executeSearch, provider, query]);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), 2200);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const copyValue = useCallback(async (value: string, label: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setNotice(label);
    } catch (copyError) {
      setError(String(copyError));
    }
  }, []);

  const revealResult = useCallback(async (result: SearchResult) => {
    try {
      await revealItemInDir(result.sourcePath);
    } catch (revealError) {
      setError(String(revealError));
    }
  }, []);

  const openTarget = useCallback(async (target: "chat" | "vscode", result: SearchResult) => {
    try {
      await launchTarget(target, result);
      const label = target === "vscode" ? "VS Code" : result.provider === "claude" ? "Claude" : "Codex";
      setNotice(`${label} opened`);
      setError(null);
    } catch (launchError) {
      setError(String(launchError));
    }
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey;

      if (command && event.key.toLowerCase() === "k") {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
        return;
      }
      if (command && event.key.toLowerCase() === "r") {
        event.preventDefault();
        if (!status.isIndexing) void runIndex();
        return;
      }
      if (command && event.shiftKey && event.key.toLowerCase() === "c" && selectedResult) {
        event.preventDefault();
        void copyValue(selectedResult.sessionId, "ID copied");
        return;
      }
      if (command && event.key.toLowerCase() === "o" && selectedResult) {
        event.preventDefault();
        void revealResult(selectedResult);
        return;
      }
      if (event.key === "ArrowDown" && results.length > 0) {
        event.preventDefault();
        setSelectedIndex((current) => Math.min(results.length - 1, current + 1));
        return;
      }
      if (event.key === "ArrowUp" && results.length > 0) {
        event.preventDefault();
        setSelectedIndex((current) => Math.max(0, current - 1));
        return;
      }
      if (event.key === "Enter" && selectedResult) {
        event.preventDefault();
        void openTarget("chat", selectedResult);
        return;
      }
      if (event.key === "Escape") {
        if (showSources) {
          setShowSources(false);
        } else if (query) {
          setQuery("");
        } else {
          inputRef.current?.blur();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [copyValue, openTarget, query, results.length, revealResult, runIndex, selectedResult, showSources, status.isIndexing]);

  const progressPercent = progress && progress.total > 0
    ? Math.round((progress.processed / progress.total) * 100)
    : 0;

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand" aria-label="Chatfinder">
          <span className="brand-glyph" aria-hidden="true"><i /><i /></span>
          <span>Chatfinder</span>
        </div>

        <label className="search-box">
          <MagnifyingGlass weight="bold" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
            placeholder="Search messages, IDs, paths"
            aria-label="Search chats"
            spellCheck={false}
            autoFocus
          />
          {query ? (
            <button type="button" className="search-clear" aria-label="Clear search" onClick={() => setQuery("")}>
              <X weight="bold" />
            </button>
          ) : <kbd>{modifier} K</kbd>}
        </label>

        <button
          className="index-button"
          type="button"
          data-active={showSources}
          onClick={() => setShowSources((current) => !current)}
          title="Index sources"
        >
          <Database weight="bold" />
          <span className="index-count">{status.indexedSessions.toLocaleString()}</span>
          <span className="status-dot" data-active={status.isIndexing} />
        </button>
      </header>

      <div className="filterbar">
        <div className="provider-filter" role="tablist" aria-label="Provider filter">
          {FILTERS.map((filter) => (
            <button
              type="button"
              role="tab"
              aria-selected={provider === filter.value}
              data-selected={provider === filter.value}
              key={filter.value}
              onClick={() => setProvider(filter.value)}
            >
              {filter.label}
            </button>
          ))}
        </div>
        <div className="result-summary">
          <span>{isSearching ? "Searching" : `${results.length} results`}</span>
          <button
            className="refresh-button"
            type="button"
            aria-label="Refresh index"
            title={`${modifier} R · Refresh index`}
            disabled={status.isIndexing}
            onClick={() => void runIndex()}
          >
            <ArrowsClockwise weight="bold" data-spinning={status.isIndexing} />
          </button>
        </div>
      </div>

      <section className="workspace">
        <div className="results-pane">
          <ResultList
            results={results}
            selectedIndex={selectedIndex}
            query={query}
            isSearching={isSearching}
            isIndexing={status.isIndexing}
            onSelect={setSelectedIndex}
            onOpen={(result) => void openTarget("chat", result)}
          />
        </div>
        <DetailPanel
          result={selectedResult}
          onCopy={copyValue}
          onOpenChat={(result) => void openTarget("chat", result)}
          onOpenVscode={(result) => void openTarget("vscode", result)}
          onReveal={(result) => void revealResult(result)}
        />
      </section>

      <footer className="statusbar">
        <div className="index-progress">
          {status.isIndexing ? (
            <>
              <span className="progress-track"><span style={{ width: `${progressPercent}%` }} /></span>
              <span>Indexing {progress?.processed ?? 0}/{progress?.total ?? status.sourceFiles}</span>
            </>
          ) : (
            <span>{status.sourceFiles} source files · local only</span>
          )}
        </div>
        <div className="shortcuts" aria-label="Keyboard shortcuts">
          <span><kbd>↑↓</kbd> Select</span>
          <span><kbd>↵</kbd> Open chat</span>
          <span><kbd>{modifier} ⇧ C</kbd> Copy ID</span>
          <span><kbd>{modifier} O</kbd> Open</span>
        </div>
      </footer>

      {showSources ? (
        <>
          <button className="panel-scrim" type="button" aria-label="Close sources" onClick={() => setShowSources(false)} />
          <SourcePanel status={status} onClose={() => setShowSources(false)} onRefresh={() => void runIndex()} />
        </>
      ) : null}

      {notice ? <div className="toast" role="status">{notice}</div> : null}
      {error ? (
        <button className="toast toast--error" type="button" onClick={() => setError(null)}>
          {error}
        </button>
      ) : null}
    </main>
  );
}

export default App;
