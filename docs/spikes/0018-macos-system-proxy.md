# D03 macOS System Proxy

## Scope

D03 adds typed HTTP, HTTPS, SOCKS, and PAC state plus a macOS
`networksetup` Adapter for one explicit network service. Disabled settings keep
their configured endpoint so a later recovery operation can restore the exact
state.

PAC URLs are treated as secrets: `Debug` output and Adapter errors never
include command output or the URL. Authenticated proxies are not configured in
V0.1, so write commands explicitly disable proxy authentication.

The Adapter itself is non-transactional. The following recovery task must read
and persist a snapshot before applying MgClash settings, then restore that
snapshot if any command fails or the application exits unexpectedly.

## macOS Intel validation

The host's `Wi-Fi` HTTP, HTTPS, SOCKS, and PAC settings were read through the
public Adapter on a macOS Intel machine. The ignored test performs no writes:

```text
env MAGIES_MACOS_NETWORK_SERVICE=Wi-Fi \
    cargo test -p magies-platform --test macos_system_proxy_real \
    -- --ignored --nocapture
```

Real setting changes are intentionally excluded from automated tests because
they can disrupt the developer or CI host's network. Command-sequence tests use
an in-memory executor and cover enabled, disabled, malformed, and failed cases.
