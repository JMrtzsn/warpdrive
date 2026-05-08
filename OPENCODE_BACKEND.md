# OpenCode Backend Integration

## What This Is

A protocol bridge that replaces Warp's "oz agent" backend with [OpenCode](https://opencode.ai) as the agent brain, while keeping Warp's native conversation UI. OpenCode runs as a localhost sidecar, and its SSE events are translated into `warp_multi_agent_api` proto types so the existing UI renders everything without modification.

## Architecture

```
User prompt → Warp UI → backend_switch → OpenCode sidecar (port 14096)
                                        ↓
                              SSE events streamed back
                                        ↓
                              opencode_adapter translates to
                              warp_multi_agent_api::ResponseEvent
                                        ↓
                              Warp UI renders natively
```

When disabled (default), `backend_switch` routes to the original `generate_multi_agent_output` — zero behavior change.

## Key Files

| File | Purpose |
|---|---|
| `crates/opencode_client/` | HTTP client, SSE subscriber, sidecar lifecycle, tool mapping |
| `app/src/ai/agent/opencode_adapter.rs` | Translates `OpenCodeEvent` → `ResponseEvent` |
| `app/src/ai/agent/backend_switch.rs` | Feature-flag router (env var or compile flag) |
| `app/src/ai/blocklist/controller/response_stream.rs` | Call site (swapped to `generate_agent_output`) |

## Activation

Either:
- `WARP_USE_OPENCODE=1` env var at runtime
- `--features opencode_backend` at compile time

Requires `opencode` binary in `$PATH`.

## Keeping This Fork Up to Date

```bash
git fetch origin master
git rebase origin/master
cargo check -p warp --lib
cargo test -p opencode_client
git push --force-with-lease
```

### Likely Conflict Points

1. **`response_stream.rs`** — highest risk. If upstream changes `generate_multi_agent_output`'s signature or call site, update `backend_switch::generate_agent_output` to match.
2. **`Cargo.toml` / `Cargo.lock`** — workspace dependency changes. Re-add `opencode_client` to workspace members and deps if lost.
3. **`app/src/ai/agent/mod.rs`** — module declarations. Re-add `mod backend_switch; mod opencode_adapter;` if lost.

Your new files (`opencode_client/`, `opencode_adapter.rs`, `backend_switch.rs`) won't conflict since they're additive.

## Proto Type Reference

The adapter maps to types from `warp_multi_agent_api` (git dep, rev `02997b8f`, prost-generated):

- Tool calls: `api::message::tool_call::{RunShellCommand, ReadFiles, ApplyFileDiffs, Grep, FileGlobV2, Server}`
- Tool results: `api::message::tool_call_result::Result::Server(ServerResult { serialized_result })`
- Stream control: `api::StreamStarted`, `api::StreamFinished`
- OpenCode sidecar port: `14096` (avoids conflict with user's opencode on default `4096`)

## TODO

- [ ] End-to-end test with actual OpenCode binary
- [ ] Verify streaming text, tool calls, and tool results render correctly in conversation UI
- [ ] Handle OpenCode permission prompts (auto-approve or surface in Warp UI)
- [ ] Map remaining OpenCode tools (todo, task) if needed
