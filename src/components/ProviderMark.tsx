import { Sparkle, TerminalWindow } from "@phosphor-icons/react";
import type { SearchResult } from "../types";

export function ProviderMark({ provider }: Pick<SearchResult, "provider">) {
  return (
    <span className={`provider-mark provider-mark--${provider}`} aria-label={provider}>
      {provider === "claude" ? <Sparkle weight="fill" /> : <TerminalWindow weight="bold" />}
    </span>
  );
}
