# sing-box Adapter

Date: 2026-08-09
Status: B04 scoped validation passed

## Scope

Implement the sing-box-specific boundary required by PRD task B04:

- accept only a previously validated Core binary;
- read and parse `sing-box version`;
- resolve a configuration to a regular canonical file;
- validate it with `sing-box check -c <config>`;
- build the validated `sing-box run -c <config>` process specification;
- preserve command failures and sing-box stderr as typed errors.

The Adapter does not generate sing-box JSON. UI and future domain/config-generator
code remain independent of sing-box's configuration schema.

## Shared boundary

Xray and sing-box now reuse only the Core-independent path resolution and
one-shot command execution helpers. Version formats, validation commands,
runtime arguments, result types, and errors remain owned by each Adapter.

## Test result

Cross-platform integration tests compile a minimal fake sing-box executable for
the active CI target. They cover version parsing, configuration acceptance and
rejection, missing and non-file paths, command startup failure, and process
startup/stop through the shared runtime.

The real macOS Intel smoke test now uses `SingBoxAdapter`. The official sing-box
1.13.18 `darwin/amd64` binary passed SHA-256/architecture validation, reported
its version, accepted the local mixed-proxy fixture, opened the listener, and
was stopped and reaped successfully.

## Remaining work

B05 is the shared process runner and already has the scoped start/poll/stop
foundation from the Phase 0 spike; its remaining PRD gap must be audited before
expanding it. Streaming stdout/stderr, health checks, and crash recovery remain
B06-B08. Config generation remains outside the runtime Adapters.
