# MgClash

MgClash is a cross-platform desktop proxy client targeting:

- macOS 13+ on Intel (`x86_64`) and Apple Silicon (`aarch64`)
- Windows 10/11 on `x86_64`
- Ubuntu 22.04+ on `x86_64`

The current milestone is the cross-platform foundation. Product behavior is
defined by the original [V1.0 PRD](Magies_Proxy_PRD_V1.0.md) together with the
[V1.1 cross-platform addendum](docs/PRD_V1.1_CROSS_PLATFORM_ADDENDUM.md). When
the documents conflict, V1.1 takes precedence.

## Development

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings

cd apps/desktop
npm ci
npm test
npm run tauri -- build --no-bundle
```

## Releases are unsigned — read this before downloading

Every artifact is **unsigned**. No Apple Developer ID, no notarization, no
Microsoft Authenticode, no Linux repository or GPG signature. This is a
deliberate decision for the current phase (see ADR 0001/0002), not an oversight,
and it has consequences you should understand before running a build:

- **macOS** — Gatekeeper blocks the app on first launch. You must open it
  explicitly (right-click → Open, or System Settings → Privacy & Security).
  **TUN mode is unavailable**: `NEPacketTunnelProvider` requires an Apple-issued
  `com.apple.developer.networking.networkextension` entitlement that an unsigned
  build cannot carry. Local HTTP/SOCKS and System Proxy do work.
- **Windows** — SmartScreen warns on first run and needs "More info → Run
  anyway". TUN requires administrator rights.
- **Linux** — the tarball is not installed from a signed repository. TUN
  requires `CAP_NET_ADMIN`.

Because nothing is signed, **verify the SHA-256 yourself**. Every artifact ships
with a `.sha256` next to it:

```sh
shasum -a 256 -c mgclash-0.1.0-macos-aarch64-unsigned.tar.gz.sha256
```

The same warning is shown inside the app on first launch.

### Artifact names

`mgclash-<version>-<os>-<cpu>-unsigned.<ext>`, for example
`mgclash-0.1.0-linux-x86_64-unsigned.tar.gz`. macOS ships a `.app` in a
tarball, Windows a portable ZIP, Linux a tarball.

Build one locally with:

```sh
scripts/package-unsigned.sh
```
