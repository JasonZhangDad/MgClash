# Shadowsocks Parser

Date: 2026-08-09
Status: C05 scoped validation passed

## Scope

C05 adds a strict `ShadowsocksParser` to `magies-profiles`. Its accepted URI
forms follow SIP002 and the current v2rayN compatibility parser:

- `ss://BASE64URL(method:password)@host:port`;
- `ss://method:percent-encoded-password@host:port`, including AEAD-2022;
- legacy `ss://BASE64(method:password@host:port)` links.

References:

- https://shadowsocks.org/doc/sip002.html
- https://xtls.github.io/en/config/outbounds/shadowsocks.html
- https://sing-box.sagernet.org/configuration/outbound/shadowsocks/
- https://github.com/2dust/v2rayN/blob/master/v2rayN/ServiceLib/Handler/Fmt/ShadowsocksFmt.cs

IPv4, bracketed IPv6, percent-encoded fields, padded and unpadded standard or
URL-safe Base64, remarks, and passwords containing colons are supported. The
cipher allowlist is the union currently understood by the bundled Xray and
sing-box core families, including AEAD-2022 and their documented legacy
methods.

SIP003 plugins and unknown query extensions cannot be represented by the
current shared node model. They return typed errors instead of being silently
dropped and producing a node that appears imported but cannot connect.

## Credential boundary

The cipher method and password are returned in a `ShadowsocksCredential`. Its
`Debug` output is redacted, and the type is intentionally not serializable or
clonable. The caller stores it in the platform secret store and supplies a
`CredentialRef` before materializing a `ProxyNode`. The shared node defaults
retain Shadowsocks UDP support.

## Test result

Ten integration tests cover all three URI forms, supported cipher families,
IPv6, percent encoding, credential materialization, UDP defaults, debug
redaction, and typed failure paths. `shadowsocks.rs` has 96.22% line coverage;
the Rust workspace has 92.44% line coverage.
