# Warpdrive

Warp terminal, de-clouded. No account, no telemetry, no phone-home.

Warpdrive is a fork of [Warp](https://github.com/warpdotdev/warp) that severs every
network egress path: telemetry, crash reporting, auto-update, account/login, and all
cloud-backed features. You get Warp's terminal, editor, and UI — running fully on your
machine, talking to nobody.

## What "de-clouded" means here

The fork does not rip out the cloud code (it is load-bearing for parts of the terminal).
Instead it makes the build structurally incapable of phoning home:

| Vector | How it's neutralized |
|---|---|
| Authenticated cloud calls (server API, sync, sessions, AI) | `skip_login` is a **default feature**, so `AuthSession::get_or_refresh_access_token` unconditionally fails — every authenticated request dies at the single chokepoint |
| Telemetry (Rudderstack) | `telemetry_config: None` → empty destination; send is additionally gated behind `is_release_bundle()` |
| Crash reporting (Sentry) | Not compiled into the `warpdrive` binary (`crash_reporting` is not in the default feature set) |
| Auto-update | `autoupdate_config: None`; unreachable for the `Oss` channel |

The kill switch lives in `app/Cargo.toml` (the `skip_login` entry in `default`) and is
enforced in `crates/warp_server_client/src/auth/session.rs`.

## Install (macOS, Apple Silicon)

```bash
brew install --cask JMrtzsn/warpdrive/warpdrive
```

## Build from source

```bash
git clone git@github.com:JMrtzsn/warpdrive.git
cd warpdrive
./script/bootstrap          # first time only — platform toolchain setup
cargo run --bin warpdrive
```

To produce a distributable `.app`:

```bash
./script/build-release      # outputs Warpdrive.app + a zip
```

## Releases

Pushing a `vX.Y.Z` tag triggers `.github/workflows/release.yml`, which builds
`Warpdrive-macos-arm64.zip` and creates a **draft** GitHub release. Publishing that
release fires `.github/workflows/update-homebrew-cask.yml`, which updates the cask in
the `JMrtzsn/homebrew-warpdrive` tap.

## Relationship to upstream Warp

Warpdrive tracks [warpdotdev/warp](https://github.com/warpdotdev/warp) as `upstream`.
The terminal, editor, and UI are upstream's work. Warpdrive's changes are deliberately
small and additive (a default feature flag plus rebranding/release tooling) so the fork
stays mergeable with upstream.

## Licensing

Warp's UI framework (the `warpui_core` and `warpui` crates) is licensed under the
[MIT license](LICENSE-MIT). The rest of the code is licensed under the
[AGPL v3](LICENSE-AGPL).
