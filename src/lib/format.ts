const relativeFormatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

export function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

export function formatRelativeDate(value: string) {
  const date = new Date(value);
  const delta = date.getTime() - Date.now();
  const minute = 60_000;
  const hour = minute * 60;
  const day = hour * 24;

  if (Math.abs(delta) < hour) return relativeFormatter.format(Math.round(delta / minute), "minute");
  if (Math.abs(delta) < day) return relativeFormatter.format(Math.round(delta / hour), "hour");
  if (Math.abs(delta) < day * 14) return relativeFormatter.format(Math.round(delta / day), "day");
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(date);
}

export function projectName(path: string | null) {
  if (!path) return "Unknown project";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function middleTruncate(value: string, maxLength = 38) {
  if (value.length <= maxLength) return value;
  const side = Math.floor((maxLength - 1) / 2);
  return `${value.slice(0, side)}…${value.slice(-side)}`;
}
