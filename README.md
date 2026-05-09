# Warpdrive

A fork of [Warp](https://github.com/warpdotdev/warp) that replaces Warp's cloud AI backend with [OpenCode](https://opencode.ai) as the sole AI engine. No Warp account required.

## What's Different

- **OpenCode is the only AI backend** — no Warp server, no account, no login
- **Auth bypass** — login screen skipped, AI features enabled for anonymous users
- **Cloud commands removed** — `/cloud-agent`, `/move-to-cloud`, `/host`, `/harness`, `/environment`, `/remote-control`, `/create-environment`, `/pr-comments`, `/usage`, `/cost`, `/continue-locally`
- **Slash commands rewired** — `/plan`, `/orchestrate`, `/compact` inject system instructions into OpenCode
- **System prompt** — OpenCode receives terminal context (CWD, shell, OS) on first message

## Prerequisites

- [OpenCode](https://opencode.ai) installed and on your `PATH`
- An AI provider configured in OpenCode (e.g. GitHub Copilot, Anthropic, OpenAI)
- macOS (Warp's only supported platform for building from source)

## Setup

1. Install OpenCode:
   ```bash
   # See https://opencode.ai for installation instructions
   brew install opencode  # or your preferred method
   ```

2. Configure your AI provider in OpenCode. For GitHub Copilot:
   ```bash
   opencode  # launches OpenCode, follow auth prompts
   ```
   The auth token is stored at `~/.local/share/opencode/auth.json`.

3. Build and run Warpdrive:
   ```bash
   ./script/bootstrap   # platform-specific setup (first time only)
   cargo run --bin warp-oss
   ```

## How It Works

```
User types in Warp agent chat
  → RequestParams built with model selection
  → generate_opencode_output() in backend_switch.rs
  → OpenCode sidecar spawned on port 14096
  → Synchronous prompt to OpenCode HTTP API
  → Response parts converted to Warp display events
  → Rendered in Warp UI
```

OpenCode manages the LLM, tool execution, and context. Warp handles the UI, terminal, and conversation state.

## Available Slash Commands

| Command | Description |
|---|---|
| `/agent`, `/new` | Start a new conversation |
| `/plan <task>` | Research and create a plan (no code yet) |
| `/orchestrate <task>` | Break task into parallel subtasks |
| `/compact` | Summarize conversation to free context |
| `/compact-and <prompt>` | Compact then send follow-up |
| `/model` | Switch the AI model |
| `/fork` | Fork conversation into new pane/tab |
| `/fork-and-compact` | Fork and summarize the fork |
| `/rewind` | Rewind to a previous point |
| `/queue <prompt>` | Queue prompt for after agent finishes |
| `/init` | Index codebase and generate AGENTS.md |
| `/index` | Index the codebase |
| `/skills` | Invoke a skill |
| `/open-file <path>` | Open file in Warp's editor |
| `/open-project-rules` | Open AGENTS.md |
| `/open-rules` | View global and project rules |
| `/open-mcp-servers` | Open MCP server settings |
| `/export-to-clipboard` | Export conversation as markdown |
| `/export-to-file` | Export conversation to a file |
| `/rename-tab`, `/set-tab-color` | Tab management |
| `/feedback` | Send feedback |
| `/changelog` | View changelog |

## Architecture

### Key Files

| File | Purpose |
|---|---|
| `app/src/ai/agent/backend_switch.rs` | Routes all AI requests to OpenCode |
| `app/src/ai/agent/opencode_adapter.rs` | Converts OpenCode responses to Warp events |
| `crates/opencode_client/src/client.rs` | HTTP client for OpenCode API |
| `crates/opencode_client/src/types.rs` | Request/response types |
| `crates/opencode_client/src/sidecar.rs` | Sidecar process management |
| `crates/opencode_client/src/events.rs` | SSE event parser |
| `crates/opencode_client/src/mapping.rs` | Tool call mapping (OpenCode → Warp) |

### Tests

```bash
# Unit tests
cargo test -p opencode_client
cargo test -p warp -- opencode_adapter

# Integration test (requires opencode on PATH)
cargo test -p opencode_client -- --ignored --nocapture
```

## Upstream

Based on [warp-oss](https://github.com/warpdotdev/warp). To pull upstream changes:

```bash
git remote add upstream https://github.com/warpdotdev/warp.git
git fetch upstream
git merge upstream/main  # resolve conflicts in backend_switch.rs, opencode_adapter.rs
```

## Licensing

Warp's UI framework (`warpui_core` and `warpui` crates) is MIT licensed. The rest is AGPL v3. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-AGPL](LICENSE-AGPL).
