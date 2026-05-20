# Upstream Sync Strategy

## Overview

Warpdrive is a fork of [warp-terminal/warp](https://github.com/warpdotdev/warp). Upstream moves fast (~100+ commits/week) and most changes are irrelevant (cloud, billing, auth, orchestration). We cherry-pick selectively.

## Principles

1. **Never merge upstream wholesale.** A full merge brings hundreds of irrelevant changes and guaranteed conflicts in our patched files.
2. **Cherry-pick by theme.** Group related commits into small sync branches.
3. **Sync weekly.** Letting it drift too far makes conflict resolution harder.
4. **Our patches win.** If a conflict touches `backend_switch.rs`, `opencode_adapter.rs`, or any file where we replaced cloud logic, our version is authoritative.

## Paths we care about

| Path | Why |
|---|---|
| `crates/terminal` | Core terminal emulation |
| `crates/pty` | PTY handling |
| `crates/shell_integration` | Shell hooks, prompts, HISTSIZE |
| `crates/warpui` / `crates/warpui_core` | UI framework |
| `app/src/terminal` | Terminal rendering, scrollback |
| `app/src/input` | Keyboard handling, IME |
| `app/src/app` | Window management, tabs |
| `app/src/workspace` | Panes, splits, layout |

## Paths we ignore

- `**/cloud/**`, `**/billing/**`, `**/auth/**` — ripped out
- `**/orchestrat**` — cloud orchestration
- `**/environment**` — remote environments
- `**/harness**` — cloud agent harnesses
- `**/telemetry**` — we don't phone home
- Anything mentioning `REV-` (revenue tickets)

## Workflow

### 1. Fetch and review

```bash
git fetch upstream
./script/upstream-review
```

The script shows commits touching our relevant paths, excluding cloud noise. Review the list and note commit hashes worth taking.

### 2. Create a sync branch

Name it by theme and date:

```bash
git checkout -b sync/terminal-fixes-2026-05-20
```

### 3. Cherry-pick

```bash
git cherry-pick <hash1> <hash2> <hash3>
```

If a commit doesn't apply cleanly:
- **Conflict in our patched files** (`backend_switch.rs`, `opencode_adapter.rs`, sidecar code) → keep ours, skip the upstream hunk.
- **Conflict in shared code** (terminal, UI) → resolve by reading the upstream PR description (linked in commit message as `#NNNNN`).
- **Too messy** → skip it. Not every upstream fix is worth the integration cost.

### 4. Build and test

```bash
cargo build --bin warp-oss
cargo test -p opencode_client
cargo test -p warp -- opencode_adapter
```

### 5. Merge to master

```bash
git checkout master
git merge sync/terminal-fixes-2026-05-20
git branch -d sync/terminal-fixes-2026-05-20
```

## Conflict resolution rules

| File / area | Rule |
|---|---|
| `backend_switch.rs` | Always keep ours |
| `opencode_adapter.rs` | Always keep ours |
| `crates/opencode_client/` | Always keep ours (upstream doesn't have this) |
| Feature flag cleanup commits | Take them — less dead code is good |
| New slash commands | Skip — we manage commands ourselves |
| UI/terminal fixes | Take upstream's version, then verify it compiles |

## Script reference

- `./script/upstream-review` — list cherry-pick candidates
- `./script/upstream-review --all` — show all upstream commits (unfiltered)
