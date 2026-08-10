# G06 72-hour stability soak

## Scope

PRD section 50 lists "72 小时稳定性测试" as a V0.1 acceptance item, and the V1.1
addendum requires it per platform. This spike covers the repeatable harness that
produces that result, not the 72-hour result itself.

`crates/magies-session/tests/soak.rs` drives a real Core process through
`DesktopSession` for a configurable duration. Every 250 ms it probes the Core's
local SOCKS port over real TCP, and every eighth tick it forces a full recovery
cycle through `NetworkRecoveryPolicy` — stop System Proxy, stop Core, start
Core, re-enable — so a long run accumulates hundreds of stop/start cycles rather
than one idle process.

## What it asserts

The point of a soak is drift that a short test cannot show:

| Assertion | Drift it catches |
| --- | --- |
| `max_runtime_files == 1` | `session-<uuid>.json` accumulating in the runtime directory across reconnects |
| `failed_probes == 0` | the Core silently stopping without the session noticing |
| every forced recovery returns `Reconnected` | reconnect degrading after N cycles |
| `file_count() == 0` after `stop` | the last runtime config outliving the session |

`max_reconnect_attempts` is reported so a run where reconnects started needing
retries is visible even when it still passed.

## Self-contained by default

The default Core is `tests/fixtures/soak_core.rs`, compiled with bare `rustc` at
test time like the other fixtures. Unlike `fake_sing_box`, it parses the
generated config for its `listen_port` values and actually binds them, so the
session's real TCP health check and the real `TcpHealthProbe` run against a real
listener. That removes the two things that would otherwise make a 72-hour run
hard to schedule — a pinned binary and a reachable proxy server — while still
exercising the process lifecycle, health, and recovery paths.

Point `MAGIES_SOAK_CORE_BIN` at an official sing-box to soak against the real
Core; the generated config is byte-for-byte the one the app writes.

System Proxy is disabled for the duration. A 72-hour run must not hold the
host's proxy settings hostage; save/restore has its own tests and its own
`--ignored` real-system test.

## Running it

```sh
# verify the harness (20 s default)
cargo test -p magies-session --test soak -- --ignored --nocapture

# the PRD's 72 hours
MAGIES_SOAK_DURATION_SECS=259200 \
  cargo test -p magies-session --test soak -- --ignored --nocapture

# against the pinned Core
MAGIES_SOAK_CORE_BIN=/path/to/sing-box MAGIES_SOAK_DURATION_SECS=259200 \
  cargo test -p magies-session --test soak -- --ignored --nocapture
```

## Result so far

Real macOS Intel hardware (`x86_64-apple-darwin`, Core i7-9750H), fixture Core:

```text
# 20 s default
soak: ticks=77 healthy=77 failed=0 reconnects=9 max_reconnect_attempts=1 max_runtime_files=1

# MAGIES_SOAK_DURATION_SECS=300
soak: ticks=1140 healthy=1140 failed=0 reconnects=142 max_reconnect_attempts=1 max_runtime_files=1
soak duration: 300.21s
```

142 stop/start cycles without a single reconnect needing a retry, without a
probe failing, and without a runtime config surviving its session. That is the
shape a passing 72-hour run should have; it is not a substitute for one.

## Remaining work

- **The 72-hour run has not been performed.** Only short runs have. The V0.1
  acceptance item stays open until a 72-hour result exists per platform.
- The harness has only run on macOS Intel and only against the fixture Core; no
  run against a pinned sing-box has been done, and macOS Apple Silicon, Windows,
  and Linux are unverified.
- The harness does not sample RSS or file-descriptor counts. `CoreOutput` is an
  unbounded channel, so a caller that *holds* it without draining would grow
  without bound over 72 hours — the desktop app drops it, and the reader thread
  discards chunks once the receiver is gone, so this is not currently a leak.
  A future log viewer that retains `CoreOutput` must drain it.
