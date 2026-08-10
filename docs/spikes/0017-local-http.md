# D02 Local HTTP

## Scope

D02 adds a pure, platform-independent generator for a loopback-only HTTP proxy
inbound. It does not enable transparent proxying, authentication, automatic
System Proxy mutation, or remote proxy-node outbounds.

The listener is fixed to `127.0.0.1`, defaults to port `10809`, and rejects
ports outside `1..=65535` before configuration generation.

## Generated Core configurations

- Xray: `http` inbound with `allowTransparent` disabled and a `freedom`
  direct outbound.
- sing-box: `http` inbound with `set_system_proxy` disabled and a `direct`
  outbound. System Proxy remains an application/platform Adapter concern.

SOCKS and HTTP generators share the Core-specific log and direct-outbound
shells so later composition does not duplicate those fields.

## macOS Intel validation

The smoke tests use the same verified official Xray 26.3.27 and sing-box
1.13.18 `darwin/amd64` binaries recorded by D01. For each Core they:

1. generate and validate the JSON configuration;
2. start the Core and wait for the loopback HTTP listener;
3. send an absolute-form HTTP proxy request to a temporary local origin;
4. verify that the origin receives `/health` and the client receives `204`;
5. stop and reap the Core process.

```text
env MAGIES_XRAY_BIN=/absolute/path/to/xray \
    MAGIES_SING_BOX_BIN=/absolute/path/to/sing-box \
    cargo test -p magies-profiles --test local_proxy_core_smoke \
    -- --ignored --nocapture
```

Both Xray and sing-box passed on a real macOS Intel host.
