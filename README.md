# Chatfinder

Fast, local search for Claude Code and Codex conversation history.

Chatfinder discovers the default chat folders on macOS, Windows, and Linux, extracts searchable conversation text into an incremental SQLite FTS index, and keeps the original JSONL files untouched.

## Features

- Searches message text, tool text, chat IDs, project paths, and source paths
- Supports active and archived Claude Code and Codex sessions
- Incremental indexing based on file size and modification time
- Skips oversized attachment and binary-like JSONL records
- Reveals the original chat file in Finder, Explorer, or the Linux file manager
- Uses the operating system WebView through Tauri; the macOS app bundle is about 12 MB
- Stores the index locally and makes no network requests

## Shortcuts

| Action | macOS | Windows / Linux |
| --- | --- | --- |
| Focus search | `⌘ K` | `Ctrl K` |
| Move selection | `↑` / `↓` | `↑` / `↓` |
| Reveal selected file | `Enter` or `⌘ O` | `Enter` or `Ctrl O` |
| Copy chat ID | `⌘ Shift C` | `Ctrl Shift C` |
| Refresh index | `⌘ R` | `Ctrl R` |

## Sources

Chatfinder automatically checks these folders under the current user profile:

```text
.claude/projects
.codex/sessions
.codex/archived_sessions
```

The first index can take time for large histories. Later refreshes only process changed files. The local index can be removed safely; Chatfinder rebuilds it from the original JSONL files.

## Development

Prerequisites:

- Node.js 22+
- Rust stable
- Platform dependencies listed in the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
npm install
npm run tauri dev
```

Validation:

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Native bundle:

```bash
npm run tauri build
```

## Architecture

| Layer | Technology | Responsibility |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Native window, system WebView, file reveal |
| Search engine | Rust + SQLite FTS5 | Discovery, parsing, incremental indexing, ranking |
| Interface | React 19 + TypeScript | Keyboard-first search and result inspection |
| Build | Vite 7 | Small production frontend bundle |

## Privacy

All parsing and search happen on-device. Chatfinder does not upload conversation content, telemetry, or index data.

## License

[MIT](LICENSE)
