import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  IndexSummary,
  ProviderFilter,
  SearchResult,
} from "../types";

export function getStatus() {
  return invoke<AppStatus>("get_status");
}

export function syncIndex() {
  return invoke<IndexSummary>("sync_index");
}

export function searchSessions(query: string, provider: ProviderFilter) {
  return invoke<SearchResult[]>("search_sessions", {
    query,
    provider,
    limit: 100,
  });
}
