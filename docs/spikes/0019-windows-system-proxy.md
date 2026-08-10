# D03 Windows System Proxy

## Scope

The Windows Adapter reads and writes the current user's WinINet LAN connection
options: flags, protocol-specific proxy servers, bypass list, and PAC URL. Raw
snapshots retain settings outside the shared model so recovery can restore the
exact prior values.

Writes use `INTERNET_OPTION_PER_CONNECTION_OPTION`, followed by
`INTERNET_OPTION_PROXY_SETTINGS_CHANGED` and `INTERNET_OPTION_REFRESH`. The
fixed PowerShell/C# bridge keeps Win32 unsafe code outside this repository,
whose Rust code forbids `unsafe`. Desired snapshot JSON is supplied on stdin;
PAC and proxy values never appear in process arguments, errors, or `Debug`.

## Windows x86-64 validation

The Windows CI job first runs a read-only real WinINet test. It then explicitly
runs an ignored round-trip test on its ephemeral user: read the full snapshot,
write that same snapshot, refresh WinINet, read it again, and require exact
equality.

```text
cargo test -p magies-platform --test windows_system_proxy_real \
    -- --ignored --nocapture
```

Windows exposes one explicit-proxy enable flag. The Adapter rejects a shared
state that mixes an enabled proxy with a disabled but configured endpoint,
rather than silently changing its meaning.
