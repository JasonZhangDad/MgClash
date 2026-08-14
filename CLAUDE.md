# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

MgClash is a cross-platform desktop proxy client (macOS 13+ x86_64/aarch64, Windows 10/11 x86_64,
Ubuntu 22.04+ x86_64) built as a Rust workspace plus a Tauri 2 + React shell. It drives external
proxy Cores (`sing-box`, `Xray`) as child processes; it does not implement the proxy protocols.

Product behavior is defined by `Magies_Proxy_PRD_V1.0.md` plus
`docs/PRD_V1.1_CROSS_PLATFORM_ADDENDUM.md`. **When they conflict, V1.1 wins.** Commit subjects and
spike docs reference PRD task IDs (`B04`, `E05`, `CP03`, …) — look them up in the PRD when a change
claims to implement one.

## Commands

```sh
# Rust (workspace root)
cargo test --workspace
cargo test -p magies-profiles --test sing_box_runtime_config          # one test file
cargo test -p magies-session --test desktop_session starts_core_before_system_proxy # one test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings                 # pedantic lints are CI errors
# Coverage gate. It skips the two files that only run inside a live Tauri app.
cargo llvm-cov --workspace --fail-under-lines 80 \
    --ignore-filename-regex 'src-tauri/src/(lib|tray)\.rs'

# Frontend / desktop shell (apps/desktop)
npm ci
npm test                        # vitest run
npm run test:coverage           # 80% branches/functions/lines/statements gate
npm run build                   # tsc --noEmit && vite build
npm run dev                     # vite only, port 1420
npm run tauri -- build --no-bundle

# unsigned release artifact for the host platform
scripts/package-unsigned.sh
```

### Smoke and integration tests behind `--ignored`

Anything that touches a real Core binary, a real TUN device, the OS credential store, or the host's
System Proxy is `#[ignore]`d and only runs explicitly. They need env vars pointing at *official*
pinned binaries:

| Var | Used by |
| --- | --- |
| `MAGIES_SING_BOX_TUN_BIN` | `magies-profiles --test tun_smoke` |
| `MAGIES_SING_BOX_DNS_BIN` | `magies-profiles --test dns_smoke` |
| `MAGIES_SING_BOX_CONFIG_BIN` | `magies-profiles --test sing_box_outbound_smoke`, `--test sing_box_runtime_e2e` |
| `MAGIES_SING_BOX_BIN` / `MAGIES_XRAY_BIN` | macOS Intel core smokes, `local_proxy_core_smoke`, the Xray smokes below; `MAGIES_XRAY_BIN` is also how the app locates Xray at runtime |
| `MAGIES_MACOS_NETWORK_SERVICE` | macOS System Proxy real test; also the app's macOS proxy adapter |
| `MAGIES_SING_BOX_SHA256` | the app's pinned Core digest (also compiled in at build time) |
| `MAGIES_XRAY_SHA256` | the app's pinned Xray digest. Unlike sing-box there is **no** build-time fallback: ADR 0003 records that no verified official Xray digest exists in this repo, so choosing Xray without this fails with `xray_unavailable` rather than running something unverified |
| `MAGIES_SOAK_CORE_BIN` / `MAGIES_SOAK_DURATION_SECS` | `magies-session --test soak` |

```sh
cargo test -p magies-profiles --test tun_smoke -- --ignored --nocapture
cargo test -p magies-storage --test secret_store platform_store_obeys_secret_store_contract -- --ignored

# the whole Xray config pipeline against a real binary; nothing in it has run yet
MAGIES_XRAY_BIN=/path/to/xray cargo test -p magies-profiles \
    --test xray_runtime_smoke --test xray_outbound_smoke -- --ignored --nocapture

# no pinned binary needed: reads the host's real default route / runs a fixture Core
cargo test -p magies-platform --test network_path -- --ignored --nocapture
cargo test -p magies-session --test soak -- --ignored --nocapture
MAGIES_SOAK_DURATION_SECS=259200 cargo test -p magies-session --test soak -- --ignored --nocapture
```

`.github/workflows/ci.yml` is the authoritative recipe: it downloads sing-box 1.13.18 and Wintun
0.14.1, verifies SHA-256 and (on Windows) the Authenticode signature, grants `CAP_NET_ADMIN` on
Linux, and wraps the Linux keyring test in `dbus-run-session` + `gnome-keyring-daemon`. Reproduce
those steps locally rather than inventing new ones.

## Architecture

### Crate graph

```
magies-domain      validated newtypes (NodeName, ServerAddress, CredentialRef, ProxyNode, …)
magies-platform    OS/CPU matrix, unsigned-build capabilities, System Proxy adapters + recovery,
                   default-route fingerprint (network_path)
magies-storage     SecretStore trait; PlatformSecretStore (keyring) / MemorySecretStore
magies-routing     RouteProfile → ordered route JSON for sing-box and for Xray
magies-core-runtime process lifecycle: binary validation, adapters, spawn/poll/stop, output,
                   health, crash recovery, atomic runtime config file, TUN state machine
magies-profiles    URI parsers + ShareLinkParser dispatcher, subscriptions (SQLite), credential
                   codec, config generators, DiagnosticRedactor
magies-session     DesktopSession — orchestrates all of the above; NetworkRecoveryPolicy +
                   NetworkWatcher for network-change and sleep/wake recovery
apps/desktop       Tauri shell (thin) + React UI
```

Dependencies flow strictly downward. **The Rust layers must not depend on Tauri** (PRD constraint 3).
`apps/desktop/src-tauri` depends on every crate above but stays a thin command layer: DTO
conversion, Tauri state, and the two per-OS wiring modules (`core_control`, `platform_proxy`).
Logic belongs in the crates — if a command body grows past plumbing, it is in the wrong place.
This is also why the Rust coverage gate skips `src-tauri/src/lib.rs` and `src-tauri/src/tray.rs`:
both only run inside a live Tauri app, and a line-coverage number over plumbing measures the wrong
thing. Anything they call must live in a crate, where the gate does apply — a command body that
needs its own tests is a command body that should have been a crate function.
The UI never reads or writes Core JSON (constraint 4); it goes through Tauri commands.

### Config generation pipeline

There is one pipeline per Core, chosen by `CoreCapabilityMatrix` (PRD section 14) from the node's
protocol and the user's preference; `DesktopSessionProfile::with_core` carries the answer into the
session. **Never branch on the Core outside the matrix** — PRD 14.2 forbids scattered
`if core == xray` checks, and the UI only renders what the matrix concluded.

`SingBoxRuntimeConfigGenerator::generate` (`magies-profiles/src/sing_box_runtime_config.rs`)
assembles one sing-box JSON document from independently tested sub-generators:

- `SingBoxOutboundConfigGenerator` — the selected node's outbound (only emitted when the route
  actually references `proxy`)
- `LocalSocksConfigGenerator` / `LocalHttpConfigGenerator` — loopback inbounds
- `SingBoxTunConfigGenerator` — TUN inbound, plus prepended `sniff` / `hijack-dns` route actions
- `SingBoxDnsConfigGenerator`, `SingBoxRouteConfigGenerator`

`XrayRuntimeConfigGenerator::generate` (`magies-profiles/src/xray_runtime_config.rs`) is the
counterpart, built from `XrayOutboundConfigGenerator`, `XrayDnsConfigGenerator`, and
`XrayRouteConfigGenerator`. The two schemas differ in shape, not just field names — Xray has no
`final`, no server tags, and no TUN inbound — so the Xray generators translate rather than rename.
Places where meaning cannot survive the translation are pinned by tests; look for them before
assuming a field maps across.

Each sub-generator has its own unit test file *and* an `--ignored` smoke test that feeds the output
to a real `sing-box check` or `xray run -test`. Adding a config field means updating both. The Xray
smokes were last run against Xray 26.3.27 on macos-x86_64 and all passed; what they prove is that
the schema is accepted, not that traffic routes correctly.

### Session lifecycle (`magies-session`)

`DesktopSession::start` has a deliberate order that must be preserved:

1. load secret from `SecretStore` → `CredentialCodec::decode`
2. generate config → `AtomicRuntimeConfig::write` (temp file + rename, `session-<uuid>.json`)
3. start Core (`CoreSessionControl::start` → validate binary, `sing-box check`, spawn, TCP health)
4. **only then** enable System Proxy

Failure at step 4 rolls the Core back; if the rollback itself fails the session stays `active` so it
remains stoppable, and the error carries both causes (`ProxyEnableAndCoreRollback`). `stop` reverses
the order: restore System Proxy, then stop Core. TUN and System Proxy are mutually exclusive
(`ConflictingNetworkModes`).

`DesktopSession` is generic over `SecretStore`, `CoreSessionControl`, and `SystemProxySessionControl`
so tests inject fakes. `SingBoxCoreControl` is the real `CoreSessionControl` impl. An active session
retains the profile it started from (`active_profile`) so recovery can restart it without the caller
reassembling it.

### Recovery (`magies-session`)

`NetworkRecoveryPolicy` implements PRD section 29: debounce → check Core health → reconnect *only*
when the probe fails, bounded to `MAX_RECOVERY_ATTEMPTS`. Two properties are load-bearing and have
their own tests — **a user-requested `stop` is never undone by recovery**, and **an exhausted burst
is not terminal** (the profile is retained so a later event retries; waking with no network yet must
not kill the session permanently). `NWPathMonitor` is unusable here because `unsafe_code` is
`forbid`ed workspace-wide, so `NetworkWatcher` derives events from polled default-route
fingerprints plus wall-clock gaps. See ADR-adjacent spike `docs/spikes/0021`.

### Platform adapters

`magies-platform` isolates every OS-specific behavior behind a trait with per-OS implementations
(`macos_system_proxy.rs` via `networksetup`/SystemConfiguration, `windows_system_proxy.rs` via the
registry, `linux_system_proxy.rs` via GSettings/`gio`). `SystemProxyRecoveryManager` snapshots the
user's pre-existing proxy state into a `JsonRecoveryStore` before mutating it, and can detect and
repair a leftover snapshot at startup (`inspect_startup` / `recover` / `dismiss`).

Unsupported capabilities must fail with a typed error **before** startup and must not be shown as
available in the UI — e.g. `TargetPlatform::unsigned_tun_availability` returns
`UnavailableInUnsignedBuild` on macOS, which the UI surfaces as a disabled TUN toggle. All release
artifacts are currently unsigned; see ADR 0001/0002 for the consequences.

## Conventions

- **Rust 2024 edition, toolchain pinned to 1.97.1** (`rust-toolchain.toml`). `unsafe_code` is
  `forbid`ed workspace-wide; `clippy::pedantic` is on and CI denies warnings.
- **Typed errors everywhere.** `thiserror` enums per module, `#[source]` chaining, and generic error
  parameters when a type wraps injected adapters (`DesktopSessionError<C, P>`). No `anyhow`, no
  string errors. `expect` is only used for invariants the type system already guarantees, with the
  reason spelled out.
- **Validated newtypes over primitives.** Construct through fallible constructors (`NodeName::new`,
  `ProxyEndpoint::new`, `TunProfile::new`) that return domain errors; serde uses
  `try_from = "String"` so deserialization goes through the same validation.
- **Tests live in `tests/`, not inline.** Integration tests exercise the public API; only a handful
  of modules have `mod tests`. `magies-core-runtime/tests/fixtures/*.rs` are standalone programs
  compiled with `rustc` at test time (`common::compile_fixture`) to act as fake Cores — that is how
  process lifecycle, output streaming, health, and crash recovery are tested without a real binary.
- **Secrets never touch the domain model.** `ProxyNode` holds a `CredentialRef`; the actual secret
  lives in the OS keyring as a `SecretValue` (zeroized on drop, `Debug` prints `[REDACTED]`), and is
  serialized through `CredentialCodec`.
- **Pinned external versions are load-bearing**: sing-box 1.13.18, Wintun 0.14.1, `tauri =2.11.5`,
  `keyring =3.6.3`, `rusqlite =0.35.0`. SHA-256 digests for the downloads live in ADR 0002 and CI.
- **Branch + commit style**: one branch per PRD task (`feat/tun-state-machine`), Conventional
  Commits with a one-line subject and empty body (`feat(session): control sing-box lifecycle`).
  Commits are small and single-purpose.
- **Docs**: an ADR in `docs/adr/` for locked-in architectural decisions, a numbered spike report in
  `docs/spikes/` for capability investigations (scope, shared boundary, test result, remaining
  work). PRD and ADR 0001 are in Chinese; code, comments, and newer docs are in English.
  Every spike ends with a **Remaining work** section naming what was *not* verified — keep that
  habit; an unverified claim is worse than an acknowledged gap.
- **Release artifacts are unsigned** and named `mgclash-<version>-<os>-<cpu>-unsigned.<ext>` with a
  `.sha256` sidecar (ADR 0003). The artifact does not bundle sing-box yet: verified official macOS
  digests do not exist in this repo, and inventing one would defeat the pin.
