# VLESS Parser

Date: 2026-08-09
Status: C02 scoped validation passed

## Scope

C02 adds the shared, Tauri-independent `magies-profiles` crate and a strict
`VlessParser` for standard `vless://UUID@host:port` sharing URIs. It parses the
common TCP, WebSocket, and gRPC transports with none, TLS, or Reality transport
security. IPv4, bracketed IPv6, IDN normalization, percent-encoded fields,
optional remarks, protocol encryption, and flow are supported.

The parser follows the XTLS/Xray-core sharing-link proposal's case-sensitive
field names, percent encoding, port range, and no-duplicate-parameter rules:

- https://github.com/XTLS/Xray-core/discussions/716

Transport variants that the current unified model cannot carry without loss,
including mKCP, HTTP, HTTPUpgrade, and XHTTP, return
`UnsupportedTransport`. Unknown, duplicate, irrelevant, empty, and missing
parameters also return typed errors instead of being ignored.

## Credential boundary

The VLESS user UUID, encryption, and flow are returned in a
`VlessCredential`, whose `Debug` output is redacted and which is intentionally
not serializable or clonable. The parser returns a `ParsedVlessNode`; after the
caller writes its credential to the platform secret store, it can supply the
resulting `CredentialRef` and materialize a `ProxyNode`. The shared node model
therefore never stores the authentication UUID in plaintext.

## Test result

Nine integration tests cover TCP defaults, WebSocket/TLS, IPv6 gRPC/Reality,
IPv4, percent encoding, name fallback, materialization after secret storage,
debug redaction, and typed failures for malformed or unsupported input. The
parser crate has 97.65% line coverage; the Rust workspace has 89.62% line
coverage.
