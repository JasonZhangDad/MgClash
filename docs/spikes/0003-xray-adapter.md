# Xray Adapter

Date: 2026-08-09
Status: B03 scoped validation passed

## Scope

Implement the Xray-specific boundary required by PRD task B03:

- accept only a previously validated Core binary;
- read and parse `xray version`;
- resolve a configuration to a regular canonical file;
- validate it with `xray run -test -c <config>`;
- build the validated `xray run -c <config>` process specification;
- preserve command failures and Xray stderr as typed errors.

The Adapter does not generate Xray JSON. UI and future domain/config-generator
code therefore remain independent of Xray's configuration schema.

## Test result

Cross-platform integration tests compile a minimal fake Xray executable for the
active CI target. They cover version parsing, configuration acceptance and
rejection, missing and non-file paths, command startup failure, and process
startup/stop through the shared runtime.

The existing real macOS Intel smoke test now uses `XrayAdapter`. The official
Xray 26.3.27 `darwin/amd64` binary passed SHA-256/architecture validation,
reported its version, accepted the local HTTP fixture, opened the listener, and
was stopped and reaped successfully.

## Remaining work

B04 will add the equivalent sing-box Adapter. Streaming stdout/stderr, health
checks, and crash recovery remain B06-B08. Config generation remains outside
the runtime Adapter and will be developed from the shared domain model.
