# Trojan Parser

Date: 2026-08-09
Status: C04 scoped validation passed

## Scope

C04 adds a strict `TrojanParser` to `magies-profiles` for
`trojan://password@host:port` sharing URIs. The supported query fields and
transport mappings follow the current Xray documentation and v2rayN parser:

- https://xtls.github.io/en/config/outbounds/trojan.html
- https://xtls.github.io/en/config/transport.html
- https://github.com/2dust/v2rayN/blob/master/v2rayN/ServiceLib/Handler/Fmt/TrojanFmt.cs
- https://github.com/2dust/v2rayN/blob/master/v2rayN/ServiceLib/Handler/Fmt/BaseFmt.cs

TCP/RAW, WebSocket, and gRPC map to the current shared transport model. TLS,
REALITY, IPv4, bracketed IPv6, percent-encoded passwords and fields, remarks,
and Trojan flow are supported. Both v2rayN insecure aliases,
`allowInsecure` and `insecure`, are accepted when their values agree.

Trojan links default to TLS when `security` is omitted because Xray requires
transport security for public Trojan connections. Explicit `security=none`
is preserved for private, trusted links. WebSocket plus REALITY is rejected
because Xray does not support that transport-security combination.

Unsupported transports, RAW header camouflage, conflicting aliases, lossy
insecure settings, unknown parameters, duplicate parameters, and malformed
transport-specific fields return typed errors instead of being ignored.

## Credential boundary

The password and flow are returned in a `TrojanCredential`. Its `Debug` output
is redacted, and the type is intentionally not serializable or clonable. The
caller stores the credential in the platform secret store and then supplies a
`CredentialRef` to materialize a `ProxyNode`; the shared node model never
contains the plaintext Trojan password.

## Test result

Nine integration tests cover the exact scheme, default TCP/TLS, percent
encoding, WebSocket/TLS, both v2rayN insecure aliases, IPv6 gRPC/REALITY,
flow, RAW and explicit-none compatibility, credential materialization, debug
redaction, and typed failures. `trojan.rs` has 98.03% line coverage; the Rust
workspace has 92.03% line coverage.
