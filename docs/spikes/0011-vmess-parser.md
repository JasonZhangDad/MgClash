# VMess Parser

Date: 2026-08-09
Status: C03 scoped validation passed

## Scope

C03 adds a strict `VmessParser` to `magies-profiles`. It accepts both VMess
formats used by the desktop import path:

- the readable VMess AEAD URL proposed by XTLS/Xray-core;
- v2rayN's `vmess://Base64(JSON)` subscription format, with padded or
  unpadded standard and URL-safe Base64.

The format boundaries follow the current primary references:

- https://github.com/XTLS/Xray-core/discussions/716
- https://github.com/2dust/v2rayN/wiki/Description-of-VMess-share-link

AEAD URLs use `auto` as the default encryption and do not accept VLESS-only
`flow`. Legacy JSON accepts string or numeric `v`, `port`, `aid`, and
`insecure` fields and preserves its declared `aid` and `scy` in the parsed
credential. TCP, WebSocket, and gRPC plus none, TLS, or AEAD Reality security
are mapped to the current shared domain model.

Unsupported transports and TCP headers return typed errors. Unknown legacy
JSON fields are rejected. Non-empty `vcn` and `pcs` are also rejected because
the current `TlsConfig` cannot carry certificate-name verification or pinned
certificate hashes without losing meaning.

## Credential boundary

The VMess UUID, security selection, and alter ID are returned in a
`VmessCredential`. Its `Debug` output is redacted, and it is intentionally not
serializable or clonable. As with VLESS, a caller must first store the
credential in the platform secret store and then provide a `CredentialRef` to
materialize a `ProxyNode`; the shared node model never contains the plaintext
VMess credential.

## Test result

Eleven integration tests cover both sharing formats, both Base64 alphabets,
string and numeric legacy fields, TCP defaults, WebSocket/TLS, IPv6
gRPC/Reality, credential materialization, debug redaction, and typed failures
for malformed or lossy inputs. `vmess.rs` has 97.63% line coverage; the Rust
workspace has 91.21% line coverage.
