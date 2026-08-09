# Core TCP Health Check

Date: 2026-08-09
Status: B07 scoped validation passed

## Scope

B07 defines the shared Core readiness check as two conditions:

1. the managed Core process is still running;
2. its configured local TCP listener accepts a connection within a caller-owned
   timeout.

This is the smallest portable health boundary shared by Xray and sing-box. A
process-only check can report healthy before the proxy is ready, while a
Core-specific API check would couple the process runner to optional controller
configuration.

## Contract

`CoreRuntime::wait_for_tcp_health` returns `CoreHealth` with the elapsed startup
time when the listener is ready. It polls process state before each bounded TCP
attempt and returns typed errors for:

- a health check requested without a running Core;
- a Core that exits before readiness;
- an underlying runtime polling failure;
- a timeout, including the target address and last operating-system connection
  error when an attempt occurred.

A failed readiness check does not stop a Core that is still running. HTTP URL
tests, real proxy traffic tests, node latency scoring, periodic monitoring, and
automatic restart remain outside B07.

## Test result

Automated tests cover delayed readiness, early process exit, bounded timeout,
zero timeout, missing process state, typed error messages, and error sources.
The Rust workspace retains 85.82% line coverage; the new health module has 100%
line coverage.

Official Xray 26.3.27 and sing-box 1.13.18 `darwin/amd64` binaries both passed
the new health API against their configured loopback listeners and were stopped
and reaped successfully on a real Intel Mac.
