# Unified Node Model

Date: 2026-08-09
Status: C01 scoped validation passed

## Scope

C01 adds the shared, Tauri-independent `magies-domain` crate described by the
cross-platform PRD. `ProxyNode` contains the common node identity, endpoint,
credential reference, transport, TLS, source, latency, test-time, UDP, and
enabled fields. `ProxyProtocol` has stable serialized names for the five P0
protocols: VLESS, VMess, Trojan, Shadowsocks, and Hysteria2.

Transport and TLS are typed enums rather than arbitrary string maps. C01
contains TCP, WebSocket, gRPC, standard TLS, and Reality because they are the
variants needed by the next VLESS parser task. Protocol-specific tasks may add
variants when their parser tests require them.

## Validation and secret boundary

The constructor rejects empty names and servers, ports outside `1..=65535`, and
missing credential references with `NodeModelError`. The validated string
types apply the same checks during deserialization, so persisted input cannot
bypass the constructor's invariants.

`ProxyNode` never contains a protocol UUID, password, or other authentication
value. It stores only a `CredentialRef` for the platform secret store. The
reference has a redacted `Debug` implementation so diagnostic output from the
complete node does not reveal its value.

## Test result

Automated tests cover P0 protocol serialization, PRD field defaults,
transport/TLS and metadata round trips, required-field failures,
deserialization validation, normalization, and debug redaction. The crate has
100% line coverage; the Rust workspace has 87.53% line coverage.
