import { CaretRight } from "@phosphor-icons/react";
import { formatRelativeDate, middleTruncate, projectName } from "../lib/format";
import type { SearchResult } from "../types";
import { MarkedText } from "./MarkedText";
import { ProviderMark } from "./ProviderMark";

interface ResultListProps {
  results: SearchResult[];
  selectedIndex: number;
  query: string;
  isSearching: boolean;
  isIndexing: boolean;
  onSelect: (index: number) => void;
  onOpen: (result: SearchResult) => void;
}

export function ResultList({
  results,
  selectedIndex,
  query,
  isSearching,
  isIndexing,
  onSelect,
  onOpen,
}: ResultListProps) {
  if (isSearching && results.length === 0) {
    return (
      <div className="result-state" aria-live="polite">
        <span className="state-pulse" /> Searching
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className="result-state" aria-live="polite">
        <span className="state-title">{isIndexing ? "Building index" : query ? "No matches" : "No chats yet"}</span>
        <span className="state-copy">{isIndexing ? "Results appear when indexing finishes." : "Try another keyword or refresh the index."}</span>
      </div>
    );
  }

  return (
    <div className="result-list" role="listbox" aria-label="Chat search results">
      {results.map((result, index) => {
        const selected = selectedIndex === index;
        return (
          <button
            className="result-row"
            data-selected={selected}
            type="button"
            role="option"
            aria-selected={selected}
            key={result.sourcePath}
            onMouseEnter={() => onSelect(index)}
            onFocus={() => onSelect(index)}
            onClick={() => onSelect(index)}
            onDoubleClick={() => onOpen(result)}
          >
            <ProviderMark provider={result.provider} />
            <span className="result-main">
              <span className="result-heading">
                <span className="result-title">{result.title || projectName(result.cwd)}</span>
                <span className="result-time">{formatRelativeDate(result.updatedAt)}</span>
              </span>
              <span className="result-id">{middleTruncate(result.sessionId, 42)}</span>
              <span className="result-snippet">
                {result.snippet ? <MarkedText text={result.snippet} /> : result.cwd || result.sourcePath}
              </span>
              <span className="result-meta">
                <span>{result.provider}</span>
                {result.matchCount > 0 ? <span>{result.matchCount} hit{result.matchCount === 1 ? "" : "s"}</span> : null}
                {result.snippetRole ? <span>{result.snippetRole}</span> : null}
              </span>
            </span>
            <CaretRight className="result-caret" weight="bold" />
          </button>
        );
      })}
    </div>
  );
}
