# AGENTS.md

Guidance for AI coding agents working in this repository.

## Desktop automation

This repository uses [`agent-desktop`](https://github.com/vercel-labs/agent-desktop) to inspect and operate
desktop applications through native accessibility trees. It is a tool, not an agent: it has no model and makes
no decisions. The observe, act, and verify loop belongs to you.

OpenCode starts it as the `agent-desktop` MCP server from `opencode.json` (`agent-desktop mcp` over stdio).
Other harnesses can call the `agent-desktop` CLI directly. MCP tools are named `desktop_<command>`; for example,
`desktop_snapshot` is the `snapshot` command, with CLI flags passed as object fields.

### Before the first desktop action

1. `agent-desktop status` reports the platform, permissions, and supported surfaces.
2. `agent-desktop skills get desktop` is the core guide to the snapshot/ref loop and retries.
3. `agent-desktop skills get platform` covers OS-specific limits; on Windows it also explains ref quoting.

Use `agent-desktop <command> --help` for flags instead of guessing.

### Working rules

- Observe, act, verify. Take a `snapshot`, act on a qualified ref such as `@s8f3k2p9:e12`, then snapshot again
  to confirm the result. Refs expire when the UI changes; `STALE_REF` and `AMBIGUOUS_TARGET` mean take a fresh
  snapshot, not retry.
- Repeat an action only when the result's `disposition.retry` is `safe`. Any other value means the action may
  already have landed.
- Prefer headless semantic actions. Use `--headed` (physical input, focus, cursor) only when the user explicitly
  permits it. Never switch to headed mode to recover from a headless failure.
- On Windows, quote every ref in PowerShell: `agent-desktop click '@s8f3k2p9:e1'`. A bare `@token` is the
  splatting operator and the argument disappears before the binary sees it.
- If PowerShell blocks the npm shim, call `agent-desktop.cmd` or use the MCP tools, which do not run through
  PowerShell.

### Safety and privacy

- Snapshots, screenshots, clipboard contents, and notifications can contain private user data. Read only what
  the task needs, and do not paste raw trees, screenshots, or clipboard text into responses, logs, commits, or
  issues unless the user asks for that specific content.
- Ask before destructive or externally visible actions: closing or quitting apps, sending messages, submitting
  forms, deleting data, changing settings, making purchases, or controlling an app the user did not name.
- Do not operate another user's session, a locked or secure desktop, or elevated windows. On Windows,
  `agent-desktop` requires an interactive signed-in session (Windows 10 1809+ or Server 2019+).
- Leave the clipboard as you found it when you change it for a task.

### Installation

`agent-desktop` is installed globally, currently version `1.3.1`, which ships a native Windows x64 binary:

```powershell
npm install --global --allow-scripts=agent-desktop agent-desktop@1.3.1
agent-desktop.cmd status
```

The `--allow-scripts` flag is required on npm versions that gate install scripts; the postinstall step links the
native binary. Native sandboxes are not available on Windows.
