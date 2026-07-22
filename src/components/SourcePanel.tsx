import { CheckCircle, Database, WarningCircle, X } from "@phosphor-icons/react";
import { formatBytes, formatRelativeDate } from "../lib/format";
import type { AppStatus } from "../types";

interface SourcePanelProps {
  status: AppStatus;
  onClose: () => void;
  onRefresh: () => void;
}

export function SourcePanel({ status, onClose, onRefresh }: SourcePanelProps) {
  return (
    <section className="source-panel" aria-label="Index sources">
      <header>
        <div>
          <span className="panel-kicker">Local index</span>
          <h2>{status.indexedSessions.toLocaleString()} chats</h2>
        </div>
        <button className="icon-button" type="button" aria-label="Close sources" onClick={onClose}>
          <X weight="bold" />
        </button>
      </header>

      <div className="source-stats">
        <span><Database weight="bold" /> {status.indexedMessages.toLocaleString()} messages</span>
        <span>{formatBytes(status.sourceBytes)} source</span>
        <span>{status.lastIndexedAt ? formatRelativeDate(status.lastIndexedAt) : "Never indexed"}</span>
      </div>

      <div className="source-list">
        {status.roots.map((root) => (
          <div className="source-row" key={root.path}>
            {root.exists ? <CheckCircle weight="fill" /> : <WarningCircle weight="fill" />}
            <div>
              <span>{root.provider}</span>
              <code>{root.path}</code>
            </div>
          </div>
        ))}
      </div>

      <button className="action-button action-button--full" type="button" onClick={onRefresh} disabled={status.isIndexing}>
        {status.isIndexing ? "Indexing…" : "Refresh index"}
      </button>
    </section>
  );
}
