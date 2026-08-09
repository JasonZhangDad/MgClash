# Hysteria2 Parser

Date: 2026-08-09
Status: C06 scoped validation passed

## Scope

C06 adds a strict `Hysteria2Parser` to `magies-profiles` for the official
`hysteria2://` and `hy2://` sharing URI schemes. The implementation follows:

- https://v2.hysteria.network/docs/developers/URI-Scheme/
- https://v2.hysteria.network/docs/developers/Protocol/
- https://sing-box.sagernet.org/configuration/outbound/hysteria2/
- https://github.com/2dust/v2rayN/blob/master/v2rayN/ServiceLib/Handler/Fmt/Hysteria2Fmt.cs

Authentication tokens and `username:password` authentication, IPv4, bracketed
IPv6, the default port 443, percent-encoded fields, remarks, SNI, strict
`insecure=0/1`, ALPN, TLS fingerprints, and `salamander` or `gecko`
obfuscation are supported. The v2rayN Gecko packet-size extensions are retained
with typed numeric and range validation.

Hysteria2 runs on QUIC, so its shared node intentionally has no generic
`TransportConfig`; mapping it to TCP would be incorrect. TLS is always present,
and the shared node keeps UDP enabled.

Authority and v2rayN-style port hopping, certificate pinning, ECH, Realm URIs,
unknown parameters, and duplicate parameters cannot currently be represented
losslessly. They return typed errors instead of producing an incomplete node.

## Credential boundary

Authentication and the optional obfuscation password are returned in a
`Hysteria2Credential`. Credential and obfuscation `Debug` output is redacted,
and neither type is serializable or clonable. The caller stores the credential
outside the shared model and supplies a `CredentialRef` before materializing a
`ProxyNode`.

## Test result

Ten integration tests cover both schemes, default and explicit endpoints,
userpass authentication, IPv6, TLS fields, Salamander and Gecko, intrinsic
QUIC mapping, credential materialization, debug redaction, and typed failures.
`hysteria2.rs` has 98.33% line coverage; the Rust workspace has 93.08% line
coverage.
