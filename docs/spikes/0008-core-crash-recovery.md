# Core Crash Recovery

Date: 2026-08-09
Status: B08 scoped validation passed

## Scope

B08 implements the PRD's bounded recovery transition for an unexpectedly exited
Core. `CoreRuntime::recover_after_crash` accepts the original validated process
specification and B07 TCP health target, then performs at most three consecutive
restart attempts.

Each attempt uses the normal B05/B06 startup path, so the binary architecture
and SHA-256 are revalidated and stdout/stderr readers are recreated. Recovery is
successful only after the restarted process passes TCP readiness. The returned
`CoreRecovery` includes the attempt count, health timing, and new live output
receiver.

## State and failures

Recovery is accepted only from `CoreState::Exited`. A running or user-stopped
Core is not restarted. An unhealthy restarted process is stopped and reaped
before another attempt begins.

After three failed attempts the runtime enters
`CoreState::Failed { attempts: 3 }`; another recovery call cannot spawn a fourth
process. An explicit user `start` begins a new sequence and clears the failed
state. Start, health, cleanup, exhausted-attempt, and retry-limit failures remain
typed and preserve their error sources.

The recovery call is a synchronous lifecycle boundary. Background scheduling,
network-change monitoring, system-proxy restoration, notification policy, and
backoff timing remain separate manager/platform tasks.

## Test result

Automated tests cover successful recovery and its new output stream, running and
user-stopped rejection, three crashing attempts, unhealthy-attempt cleanup,
retry-limit enforcement, manual start after failure, launch-time binary
revalidation, and typed error chains. The Rust workspace has 86.32% line
coverage; the recovery module has 93.48% line coverage.

On a real Intel Mac, official Xray 26.3.27 and sing-box 1.13.18 `darwin/amd64`
were each started, health-checked, terminated with `SIGKILL`, detected as
exited, recovered with the same validated configuration, health-checked again,
and stopped cleanly.
