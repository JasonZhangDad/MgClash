# D03 Linux System Proxy

## Scope

Linux has no desktop-independent System Proxy API. V0.1 supports GNOME through
the `org.gnome.system.proxy` GSettings schemas; other desktop environments must
surface an explicit unsupported result instead of reporting false success.

The snapshot preserves mode, PAC URL, ignored hosts, same-proxy behavior,
HTTP/HTTPS/FTP/SOCKS endpoints, and deprecated HTTP authentication fields. PAC,
proxy host, and authentication values are redacted from `Debug`. MgClash does
not enable GNOME proxy authentication: applying an app state clears temporary
authentication and FTP values, while the separately captured snapshot remains
available for exact restoration.

Writes use the safe GIO API rather than the discouraged `gsettings` subprocess.
GSettings delay/apply groups the child-schema updates, reverts pending values on
an error, and synchronizes the backend after apply.

## Linux x86-64 validation

The Linux CI job reads the installed GNOME schemas without mutation during the
normal workspace test. It then runs an exact read/write/read round trip in
GIO's process-local memory backend:

```text
env GSETTINGS_BACKEND=memory \
    cargo test -p magies-platform --test linux_system_proxy_real \
    -- --ignored --nocapture
```

This validates the real GIO schema and write paths without changing the CI
user's desktop settings.
