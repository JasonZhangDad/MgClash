# D01 Local SOCKS

## Scope

D01 adds a pure, platform-independent generator for loopback-only SOCKS
inbounds. It does not write runtime files, select a proxy node, mutate System
Proxy, or add the D02 HTTP inbound.

The listener is fixed to `127.0.0.1`, defaults to port `10808`, and rejects
ports outside `1..=65535` before configuration generation.

## Generated Core configurations

- Xray: `socks` inbound with no authentication and UDP enabled, plus a
  `freedom` direct outbound.
- sing-box: `socks` inbound with no users, plus a `direct` outbound.

These shapes follow the official Core documentation and are covered by exact
JSON golden tests.

## macOS Intel validation

Host: macOS Intel `x86_64`.

| Core | Version | Official artifact | Archive SHA-256 | Result |
|---|---|---|---|---|
| Xray | 26.3.27 | `Xray-macos-64.zip` | `f5b0471d3459eff1b82e48af0aeac186abcc3298210070afbbbd8437a4e8b203` | Pass |
| sing-box | 1.13.18 | `sing-box-1.13.18-darwin-amd64.tar.gz` | `500f0decfc21f7cdb2aaa4fe193b7857a41b07c38ee3a0b15bd53e3c7af3671c` | Pass |

Both smoke tests generated JSON, ran the Core's config validation command,
started the Core, waited for the TCP listener, completed a SOCKS5 no-auth
handshake, and stopped the process.

```text
env MAGIES_XRAY_BIN=/absolute/path/to/xray \
    MAGIES_SING_BOX_BIN=/absolute/path/to/sing-box \
    cargo test -p magies-profiles --test local_proxy_core_smoke \
    -- --ignored --nocapture
```

The third-party Core binaries are temporary test inputs and are not committed
or packaged by this task.
