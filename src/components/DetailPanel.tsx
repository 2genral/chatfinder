import { Copy, FolderOpen, Path } from "@phosphor-icons/react";
import { formatBytes, formatRelativeDate, projectName } from "../lib/format";
import type { SearchResult } from "../types";
import { ProviderIcon, VscodeIcon } from "./BrandIcon";
import { MarkedText } from "./MarkedText";
import { ProviderMark } from "./ProviderMark";

interface DetailPanelProps {
  result: SearchResult | null;
  onCopy: (value: string, label: string) => void;
  onOpenChat: (result: SearchResult) => void;
  onOpenVscode: (result: SearchResult) => void;
  onReveal: (result: SearchResult) => void;
}

export function DetailPanel({ result, onCopy, onOpenChat, onOpenVscode, onReveal }: DetailPanelProps) {
  if (!result) {
    return (
      <aside className="detail-panel detail-panel--empty">
        <Path size={22} />
        <span>Select a chat</span>
      </aside>
    );
  }

  return (
    <aside className="detail-panel">
      <div className="detail-signal" />
      <div className="detail-header">
        <ProviderMark provider={result.provider} />
        <div>
          <span className="detail-kicker">{result.provider}</span>
          <h2>{result.title || projectName(result.cwd)}</h2>
        </div>
      </div>

      <div className="detail-actions">
        <button type="button" className={`action-button action-button--primary action-button--${result.provider}`} onClick={() => onOpenChat(result)}>
          <ProviderIcon provider={result.provider} /> Open {result.provider === "claude" ? "Claude" : "Codex"}
        </button>
        <button type="button" className="action-button" onClick={() => onOpenVscode(result)}>
          <VscodeIcon /> Open VS Code
        </button>
        <button type="button" className="icon-button" title="Reveal source file" aria-label="Reveal source file" onClick={() => onReveal(result)}>
          <FolderOpen weight="bold" />
        </button>
      </div>

      <div className="copy-actions" aria-label="Copy chat details">
        <button type="button" className="copy-button" onClick={() => onCopy(result.title || projectName(result.cwd), "Title copied")}>
          <Copy weight="bold" /> Title
        </button>
        <button type="button" className="copy-button" onClick={() => onCopy(result.sessionId, "ID copied")}>
          <Copy weight="bold" /> Chat ID
        </button>
        <button type="button" className="copy-button" onClick={() => onCopy(result.sourcePath, "Path copied")}>
          <Path weight="bold" /> Path
        </button>
      </div>

      <dl className="detail-grid">
        <div>
          <dt>Chat ID</dt>
          <dd className="mono selectable">{result.sessionId}</dd>
        </div>
        <div>
          <dt>Project</dt>
          <dd>{result.cwd || "—"}</dd>
        </div>
        <div>
          <dt>Updated</dt>
          <dd>{formatRelativeDate(result.updatedAt)}</dd>
        </div>
        <div>
          <dt>File size</dt>
          <dd>{formatBytes(result.fileSize)}</dd>
        </div>
        <div className="detail-grid--wide">
          <dt>Source</dt>
          <dd className="mono selectable">{result.sourcePath}</dd>
        </div>
      </dl>

      {result.snippet ? (
        <div className="match-block">
          <div className="match-label">
            <span>Best match</span>
            <span>{result.matchCount} total</span>
          </div>
          <p><MarkedText text={result.snippet} /></p>
        </div>
      ) : null}
    </aside>
  );
}
