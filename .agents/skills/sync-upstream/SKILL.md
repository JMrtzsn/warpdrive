---
name: sync-upstream
description: Sync the warpdrive fork with upstream warpdotdev/warp, resolving merge conflicts according to the fork's policy and verifying the de-cloud kill switch survived. Use when pulling upstream changes, updating the fork, or resolving a sync branch's conflicts.
---

# sync-upstream

Merge the latest `warpdotdev/warp` (`upstream`) into warpdrive, resolve conflicts
per the fork's policy, and verify the result still builds with the de-cloud kill
switch intact.

## Context: what warpdrive is

warpdrive is a thin fork of Warp. The fork's entire divergence is small and
deliberate:

1. **`skip_login` is a default feature** (`app/Cargo.toml`) — the de-cloud kill
   switch. It makes every authenticated request fail at
   `crates/warp_server_client/src/auth/session.rs`, so the build cannot phone home.
2. **The oss binary is rebranded** `warp-oss` -> `warpdrive` (binary name, bundle
   metadata in `app/Cargo.toml`, identity in `script/macos/bundle`).
3. **Release/CI tooling** is warpdrive's own (`release.yml`,
   `update-homebrew-cask.yml`, `script/build-release`, `script/update-homebrew-cask`).
4. **README/docs** describe the de-clouded fork.

Everything else is upstream Warp and should track upstream.

## Procedure

### 1. Run the sync script

```bash
./script/sync-upstream
```

It fetches upstream, creates a `sync/upstream-<date>` branch off the current
branch, and attempts the merge. It never pushes and never destructively resets.

- **Clean merge** -> jump to step 3 (Verify).
- **Conflicts** -> the script lists the conflicted files and stops. Continue to step 2.

To only check how far behind upstream you are: `./script/sync-upstream --status`.

### 2. Resolve conflicts (fork policy)

Resolve each conflicted file using these rules. The principle: **our small set of
fork-specific files win; everything else takes upstream.**

| File / area | Resolution |
|---|---|
| `app/Cargo.toml` | KEEP OURS for the fork-specific bits: `"skip_login",` must remain in `default`; `default-run`, the `[[bin]]` name, and `[package.metadata.bundle.bin.warpdrive]` must stay `warpdrive`. For unrelated dependency/feature changes upstream made elsewhere in the file, TAKE upstream's. This usually means a hand-merge, not "ours" wholesale. |
| `script/macos/bundle` | KEEP OURS in the `oss` channel branch (`WARP_BIN="warpdrive"`, `BUNDLE_ID="dev.warpdrive.Warpdrive"`, app name, scheme, CLI path). TAKE upstream elsewhere. |
| `README.md`, other top-level `*.md` docs | KEEP OURS (de-clouded framing). |
| `.github/workflows/**` | Run `./script/sync-upstream --sanitize-workflows` before committing. Only `ci.yml`, `release.yml`, and `update-homebrew-cask.yml` are allowed. |
| `script/build-release`, `script/update-homebrew-cask` | KEEP OURS (warpdrive-specific). |
| `crates/**`, `app/src/**` (terminal, editor, UI, input, etc.) | TAKE UPSTREAM. These are Warp's core; the fork does not modify them. After taking upstream, the build must still pass. |

When in doubt about a core source file, prefer upstream and rely on the build +
tests to catch breakage. Do not hand-edit Warp's terminal/UI logic during a sync.

For each resolved file:

```bash
git add <file>
```

Then complete the merge:

```bash
./script/sync-upstream --sanitize-workflows
git commit --no-edit
```

### 3. Verify (mandatory gate)

Two checks must pass before the sync is considered good.

**a. The kill switch survived.** This is non-negotiable — the fork's whole point.

```bash
grep -n '"skip_login",' app/Cargo.toml
```

This MUST print a match inside the `default = [ ... ]` list. If it does not, the
merge dropped the kill switch — re-add it before proceeding.

**b. warpdrive compiles.**

```bash
cargo check -p warp --bin warpdrive
```

If compilation fails, fix the breakage (usually a small API drift from upstream)
and re-run until green. See the `fix-errors` skill for help.

### 4. Land the sync

Only after both checks pass:

```bash
git checkout <base-branch>          # the branch you started from, e.g. master
git merge --ff-only sync/upstream-<date>
git push origin <base-branch>
git branch -d sync/upstream-<date>
```

## Aborting

To discard a sync attempt entirely:

```bash
git merge --abort
git checkout <base-branch>
git branch -D sync/upstream-<date>
```

## Notes

- Upstream moves fast (~100 commits/week) and most changes are core Warp code that
  merges cleanly. Conflicts almost always land in the four fork-specific areas
  above (`app/Cargo.toml`, `script/macos/bundle`, docs, workflows).
- Never resolve a conflict in a way that removes `skip_login` from default features
  or renames the binary back to `warp-oss`.
- This is a local operation. There is intentionally no GitHub Action for it:
  pushing upstream's workflow-file changes requires a `workflow`-scoped token, and
  conflict resolution needs judgment — both are better handled locally.
