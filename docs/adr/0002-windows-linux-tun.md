# ADR 0002: Windows/Linux TUN backend

## Status

Accepted for unsigned V0.1.

## Decision

Windows x86_64 and Linux x86_64 use the pinned official sing-box 1.13.18
`tun` inbound. Xray remains available for System Proxy mode but is not a V0.1
TUN backend. Unsigned macOS builds reject TUN before startup.

The shared `TunProfile` owns IPv6, MTU, auto-route, and strict-route settings.
The sing-box generator emits platform-specific interface names and enables
`auto_redirect` only for Linux automatic routing. The default userspace stack
is `gvisor`.

Linux uses the kernel `/dev/net/tun` device. The packaged sing-box process must
receive `CAP_NET_ADMIN` through the installer/helper path; the GUI process does
not run as root.

Windows packages the unmodified official Wintun 0.14.1 amd64 DLL beside
`sing-box.exe`. The MgClash executable remains unsigned. Packaging must verify:

- Wintun ZIP SHA256
  `07c256185d6ee3652e09fa55c0b673e2624b565e02c4b9091c79ca7d2f24ef51`;
- `wintun.dll` Authenticode status is `Valid`;
- sing-box Windows ZIP SHA256
  `65045155ffdc506334f01a4353889657ddfc024f72b394081a9abaef34dfbef3`.

The Windows release job performs both Wintun checks before passing the DLL and
license to `scripts/package-unsigned.sh`. The portable ZIP places `wintun.dll`
beside `sing-box.exe` and includes `LICENSE-wintun`.

## Verification

CI downloads the pinned upstream artifacts, verifies hashes and the Windows
driver signature, then runs a generated IPv4 TUN config with routing disabled.
The smoke succeeds only when sing-box creates the device and stays alive for
the observation window. Routing and DNS are separate E05/E06 integration tests.

Primary references:

- https://sing-box.sagernet.org/configuration/inbound/tun/
- https://docs.kernel.org/6.5/networking/tuntap.html
- https://www.wintun.net/
