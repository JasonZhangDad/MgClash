# CP-TUN macOS TUN without code signing

## Scope

ADR 0001/0002 left macOS TUN out of V0.1 on the assumption that an unsigned
build cannot create the device: `TargetPlatform::unsigned_tun_availability`
returned `UnavailableInUnsignedBuild`, and the UI disabled the toggle. This
spike measured what the device actually requires and what an unsigned build can
therefore offer.

## Measurement

Official sing-box 1.13.18 (`sing-box-1.13.18-darwin-amd64`, downloaded from the
SagerNet release page) against a config produced by this repo's own
`SingBoxTunConfigGenerator`, on macOS 15 (Darwin 24.6.0), x86_64:

| Run | Result |
| --- | --- |
| `sing-box check -c generated-macos.json` | accepted |
| `./sing-box run -c generated-macos.json` | `configure tun interface: Connect: operation not permitted` |
| `sudo ./sing-box run -c generated-macos.json` | `inbound/tun[tun-in]: started at utun4`, `sing-box started (0.01s)` |

**The obstacle is privilege, not signature.** macOS opens a `utun` through a
control socket that only root may connect to. No code signing, no Network
Extension entitlement, and no notarization is involved in that step — the same
unsigned binary succeeds the moment it runs as root. The generator output needed
no change beyond dropping `interface_name`, which macOS assigns itself.

## Shared boundary

An elevated Core cannot be a child process the app owns: the authorization
prompt runs it under its own privileged shell, and the app is left holding no
handle. `magies-core-runtime::elevated` therefore models it as:

- `elevation_script(binary, config, pid_file, log_file)` — one readable line,
  because the prompt shows it to the user before they approve it. Paths are
  single-quoted with `'\''` escaping so a path cannot break out of the script.
- `ElevationLauncher` — `OsascriptLauncher` in production
  (`do shell script … with administrator privileges`); tests substitute a plain
  `/bin/sh`, so everything around the prompt is exercised for real and only the
  privilege escalation itself is left unproven.
- `ElevatedCore` — tracks the Core by the PID the script wrote, reports liveness
  with `kill -0`, and stops by signalling that PID.
- The Core's log file, followed as it grows, standing in for the pipe the app
  does not have. It ends when the Core stops; a restart gets a fresh reader.

`ElevatedSingBoxControl` (magies-session) keeps `SingBoxCoreControl`'s order —
validate the config unprivileged, start, wait for the local port, and stop the
elevated Core again if that port never opens. Validation stays first on purpose:
a config sing-box already rejected must never cost the user a password prompt.
The pinned binary is resolved and digest-checked before the prompt as well;
running an unverified Core is worse as root, not better.

## Two prompts

Stopping asks for authorization a second time. The app runs as a normal user and
cannot signal a root-owned process, so `kill` fails with `EPERM` and the stop
falls back to the prompt. This was accepted deliberately rather than worked
around — the alternatives (a privileged helper installed with
`SMJobBless`, or a setuid shim) both need the code signing this spike exists to
avoid.

## Result

`unsigned_tun_availability` now reports `RequiresElevation` on all three
platforms, which the UI already rendered as an enabled toggle with a privileges
notice. `TunRuntime` no longer refuses macOS before starting.

## Remaining work

- **The prompt itself is unverified.** Every automated test substitutes a shell
  for it. Approving it, dismissing it (`tun_authorization_declined`), and the
  second prompt on stop have not been exercised on a real Mac.
- **No traffic has been routed through a macOS TUN by the app.** The `sudo` run
  above proves the device is created and sing-box starts; it does not prove the
  app's session lifecycle, System Proxy exclusivity, or DNS hijack behave once
  the device is up.
- **Windows and Linux TUN remain unverified on real machines.** They were
  already `RequiresElevation` and are unchanged by this spike.
- The elevated Core survives an app crash: the PID file is the only record, and
  nothing reclaims it at startup. A leftover root Core would keep the `utun`
  until it is killed by hand.
