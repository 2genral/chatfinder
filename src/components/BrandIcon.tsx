import { FileCode, OpenAiLogo } from "@phosphor-icons/react";
import { siClaudecode } from "simple-icons";
import type { SearchResult } from "../types";

export function ProviderIcon({ provider }: Pick<SearchResult, "provider">) {
  if (provider === "claude") {
    return (
      <svg className="brand-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d={siClaudecode.path} />
      </svg>
    );
  }

  return <OpenAiLogo className="brand-icon" weight="fill" aria-hidden="true" />;
}

export function VscodeIcon() {
  return <FileCode className="brand-icon brand-icon--vscode" weight="fill" aria-hidden="true" />;
}
