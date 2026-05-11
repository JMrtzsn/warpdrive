# Warpdrive

Warp terminal with local AI. No account, no cloud, no telemetry.

Replaces Warp's proprietary AI backend with [OpenCode](https://opencode.ai). Your prompts stay on your machine (or go to whichever LLM provider you configure in OpenCode).

## Requirements

- macOS (only platform supported for building from source)
- [OpenCode](https://opencode.ai) installed and on `PATH`
- An AI provider configured in OpenCode (GitHub Copilot, Anthropic, OpenAI, etc.)

## Install

```bash
git clone git@github.com:JMrtzsn/warpdrive.git
cd warpdrive
./script/bootstrap    # first time only
cargo run --bin warp-oss
```

OpenCode starts automatically as a sidecar process on port 14096.

## What changed from upstream Warp

| Area | Change |
|---|---|
| AI backend | All requests route to OpenCode. Warp server code is dead. |
| Auth | Login screen bypassed. No account needed. |
| Slash commands | 11 cloud-only commands removed. `/plan`, `/orchestrate`, `/compact` rewired to inject system instructions into OpenCode. |
| Input handling | All `AIAgentInput` variants (summarize, init rules, action results, clone, skills, etc.) produce text for OpenCode. |
| System prompt | First message includes CWD, shell, OS, and architecture. |

### Removed commands

`/cloud-agent`, `/move-to-cloud`, `/continue-locally`, `/host`, `/harness`, `/environment`, `/remote-control`, `/create-environment`, `/pr-comments`, `/usage`, `/cost`

### Available commands

`/agent` `/new` `/plan` `/orchestrate` `/compact` `/compact-and` `/model` `/fork` `/fork-and-compact` `/rewind` `/queue` `/init` `/index` `/skills` `/open-file` `/open-project-rules` `/open-rules` `/open-mcp-servers` `/export-to-clipboard` `/export-to-file` `/rename-tab` `/set-tab-color` `/feedback` `/changelog` `/summarize` `/new-project` `/clone`

## Tests

```bash
cargo test -p opencode_client              # client unit tests
cargo test -p warp -- opencode_adapter     # adapter unit tests
cargo test -p opencode_client -- --ignored  # integration test (needs opencode running)
```

## Syncing with upstream

```bash
git remote add upstream https://github.com/warpdotdev/warp.git
git fetch upstream
git merge upstream/main
```

Conflicts typically in `backend_switch.rs` and `opencode_adapter.rs`.

## Key files

| File | What it does |
|---|---|
| `app/src/ai/agent/backend_switch.rs` | Entry point — routes all AI to OpenCode |
| `app/src/ai/agent/opencode_adapter.rs` | Builds prompts, converts responses to Warp events |
| `app/src/ai/agent/opencode_adapter_tests.rs` | 27 unit tests |
| `crates/opencode_client/src/client.rs` | HTTP client for OpenCode API |
| `crates/opencode_client/src/sidecar.rs` | Sidecar lifecycle management |
| `crates/opencode_client/src/events.rs` | SSE event parser |

## License

Warp's UI framework (`warpui_core`, `warpui`) is MIT. Everything else is AGPL v3. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-AGPL](LICENSE-AGPL).
