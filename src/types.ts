export type ProviderFilter = "all" | "claude" | "codex";

export interface SourceRoot {
  provider: Exclude<ProviderFilter, "all">;
  path: string;
  exists: boolean;
}

export interface AppStatus {
  indexedSessions: number;
  indexedMessages: number;
  sourceFiles: number;
  sourceBytes: number;
  lastIndexedAt: string | null;
  isIndexing: boolean;
  roots: SourceRoot[];
}

export interface IndexProgress {
  phase: "indexing" | "complete";
  processed: number;
  total: number;
  indexed: number;
  skipped: number;
  currentPath: string | null;
}

export interface IndexSummary {
  discovered: number;
  indexed: number;
  skipped: number;
  removed: number;
  messages: number;
}

export interface SearchResult {
  provider: Exclude<ProviderFilter, "all">;
  sessionId: string;
  sourcePath: string;
  cwd: string | null;
  title: string | null;
  startedAt: string | null;
  updatedAt: string;
  fileSize: number;
  matchCount: number;
  snippet: string | null;
  snippetRole: string | null;
}
