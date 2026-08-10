# G01–G03 Network change, sleep/wake, and auto reconnect

## Scope

PRD section 29 fixes the recovery pipeline: a path change is debounced for
500–1500 ms, Core health is checked, and the session is reconnected *only when
necessary* — "禁止每次轻微 `NWPath` 变化都重启 Core". This spike covers how the
three platforms produce those events and how the reconnect stays bounded.

## Why not `NWPathMonitor`

The PRD names `NWPathMonitor`, which is macOS-only and reachable only through
Objective-C/C FFI. This workspace sets `unsafe_code = "forbid"` at the root, and
the Windows (`NotifyRouteChange2`) and Linux (netlink `RTMGRP_IPV4_ROUTE`)
equivalents have the same problem. No safe, cross-platform binding was adopted.

The shared boundary is therefore an **opaque path fingerprint**, following the
System Proxy adapters' existing "read-only command behind a trait" pattern:

| Platform | Command |
| --- | --- |
| macOS | `route -n get default` |
| Windows | `route print -4 0.0.0.0` |
| Linux | `ip route show default` |

`NetworkPathReader::fingerprint` normalizes whitespace and returns `None` when
the command fails or exits non-zero. Callers only compare consecutive values, so
the text is never parsed. Wi-Fi↔Ethernet, hotspot switches, and DHCP lease
changes all move the default route and therefore the fingerprint.

A failed read is deliberately not a change: a flaky read must never restart a
healthy Core.

## Sleep/wake without an OS API

`NetworkWatcher` needs no platform code for G02. The driver ticks it every
`PATH_TICK`; when the wall clock between two ticks jumps by at least
`SLEEP_THRESHOLD` (or moves backwards), the timer stopped tracking real time,
which is what a suspended machine looks like from inside the process. A wake
outranks a path change in the same tick — both lead to the same health check.

## Reconnect policy

`NetworkRecoveryPolicy` implements the PRD pipeline and two properties worth
stating explicitly, both covered by tests:

- **A user-requested disconnect is never undone.** The policy only retries a
  session that *it* took down; `DesktopSession::stop` called by the UI leaves no
  pending restart.
- **An exhausted burst is not terminal.** Waking with no network yet exhausts
  `MAX_RECOVERY_ATTEMPTS` (3), so the policy retains the profile and a later
  event retries. Without this, the exact scenario the feature exists for would
  leave the session permanently dead.

## Cost of polling

The recovery loop spawns one short-lived process every `PATH_TICK` (5 s) *only
while a session is running*; an idle app spawns none. This is the price of
forbidding `unsafe`. If a safe binding to the OS notification APIs is adopted
later, only `NetworkPathReader` changes — `NetworkWatcher` and
`NetworkRecoveryPolicy` consume events, not sources.

## Validation

```sh
cargo test -p magies-session --test network_recovery
cargo test -p magies-session --test network_watcher
cargo test -p magies-platform --test network_path
cargo test -p magies-platform --test network_path -- --ignored --nocapture
```

The `--ignored` test runs the host's real route command and asserts the
fingerprint is non-empty and stable across two reads. It was verified on real
macOS Intel hardware (`x86_64-apple-darwin`, Core i7-9750H), which is the
platform PRD V1.1 DoD 4 requires a non-cross-compiled result for. Windows and
Linux are covered by the same test in their CI jobs.

## Remaining work

- The real command has only been exercised on macOS Intel. The Windows and
  Linux fingerprints are unverified against a live host, and macOS Apple
  Silicon is unverified.
- The recovery loop holds the session mutex for the duration of a reconnect, so
  a Tauri command issued during recovery waits rather than failing fast.
