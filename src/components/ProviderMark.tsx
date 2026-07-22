import type { SearchResult } from "../types";
import { ProviderIcon } from "./BrandIcon";

export function ProviderMark({ provider }: Pick<SearchResult, "provider">) {
  return (
    <span className={`provider-mark provider-mark--${provider}`} aria-label={provider}>
      <ProviderIcon provider={provider} />
    </span>
  );
}
