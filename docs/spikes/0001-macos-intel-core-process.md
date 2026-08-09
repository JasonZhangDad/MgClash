# macOS Intel Core Process Spike

Date: 2026-08-09  
Status: Passed for the scoped process-lifecycle check

## Scope

Validate on a real `x86_64` Mac that the shared Rust process boundary can:

- start official Xray and sing-box binaries;
- observe that each Core remains running;
- wait for a local proxy listener;
- stop and reap each Core process;
- return typed errors for duplicate operations, spawn failure, early exit, and timeout.

This spike does not complete Phase 0B. System Proxy save/restore, outbound proxy
traffic, stdout/stderr capture, architecture/hash enforcement, and crash recovery
remain separate tasks.

## Environment and artifacts

- Host: macOS 15.7.7, Intel `x86_64`.
- Xray: `26.3.27`, official `Xray-macos-64.zip`, `go1.26.1 darwin/amd64`.
- Xray archive SHA-256: `f5b0471d3459eff1b82e48af0aeac186abcc3298210070afbbbd8437a4e8b203`.
  It matched the adjacent official `.dgst` asset.
- sing-box: `1.13.18`, official `sing-box-1.13.18-darwin-amd64.tar.gz`,
  `go1.26.5 darwin/amd64`.
- sing-box archive SHA-256 recorded locally:
  `500f0decfc21f7cdb2aaa4fe193b7857a41b07c38ee3a0b15bd53e3c7af3671c`.
  The release did not publish a checksum asset.

The third-party binaries were downloaded to a temporary directory and are not
stored in this repository.

Official releases:

- https://github.com/XTLS/Xray-core/releases/tag/v26.3.27
- https://github.com/SagerNet/sing-box/releases/tag/v1.13.18

## Results

| Core | Fixture | Listener | Start | Stop |
|---|---|---:|---:|---:|
| Xray | `xray-local-http.json` | `127.0.0.1:18980` HTTP | Pass | Pass |
| sing-box | `sing-box-local-mixed.json` | `127.0.0.1:18981` mixed | Pass | Pass |

Both published executables were confirmed as native Mach-O `x86_64` binaries.
Their configurations passed each Core's built-in validation before the listener
smoke tests ran.

## Reproduction

After downloading and extracting the official macOS Intel artifacts:

```sh
env MAGIES_XRAY_BIN=/absolute/path/to/xray \
  cargo test -p magies-core-runtime --test macos_intel_core_smoke \
  xray_starts_a_local_http_listener_and_stops -- --ignored --nocapture

env MAGIES_SING_BOX_BIN=/absolute/path/to/sing-box \
  cargo test -p magies-core-runtime --test macos_intel_core_smoke \
  sing_box_starts_a_local_mixed_listener_and_stops -- --ignored --nocapture
```

The tests are ignored by default so normal CI never downloads or executes a
third-party Core. The standard lifecycle tests use the Rust test executable as a
controlled child process and run on every supported CI platform.

## Decision

Use `magies-core-runtime` as the Tauri-independent shared lifecycle boundary.
Keep Core discovery, architecture/hash verification, configuration generation,
and Core-specific arguments outside the generic process runner. This keeps the
runner limited to process state and typed operating-system failures.

The current stop operation is a forced process termination. Graceful shutdown,
log capture, health protocol, and the PRD's bounded crash recovery state machine
must be added only when their corresponding tasks begin.
