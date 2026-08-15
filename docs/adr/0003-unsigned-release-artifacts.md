# ADR 0003: Unsigned release artifacts

## Status

Accepted for V0.1.

## Context

ADR 0001 locked in shipping without commercial code signing. PRD V1.1 section 5
fixes the artifact shape per platform, and Definition of Done items 5 and 6 fix
the file name and require the unsigned risk to be visible both on the download
page and at first launch. This ADR records how that is produced.

## Decision

`scripts/package-unsigned.sh` builds the host's artifact.

| Platform | Artifact | Why |
| --- | --- | --- |
| macOS | `.app` inside a `.tar.gz`, and a `.dmg` | the `.app` layout is what Gatekeeper prompts about; a bare Mach-O is not a usable desktop artifact. The `.dmg` adds the drag-to-Applications install every macOS user expects — it grants no integrity the archive lacks, since neither is signed |
| Windows | portable `.zip`, a bare portable `.exe`, and an NSIS installer | supports installed and portable use without Authenticode; the bare `.exe` is what someone who wants no archive downloads |
| Linux | `.tar.gz`, `.deb`, `.rpm`, and `.AppImage` | PRD V1.1 section 5 planned the packaged formats; none is repository- or GPG-signed, so they are convenience, not provenance |

A packaged Linux install splits the executable from its resources
(`/usr/bin/<exe>` and `/usr/lib/<name>/`), which the tarball does not. The app
looks in both places, and `package-unsigned.sh` fails the build when the built
`.deb` does not carry the Core where the app will look for it — an installer
whose app cannot find its own Core is worse than no installer.

The name is `mgclash-<version>-<os>-<cpu>-unsigned.<ext>`. Every artifact ships a
`.sha256` sidecar: with no signature, a published digest is the only integrity
check a downloader has.

Nothing receives a publisher identity signature, notarization, or stapling.
macOS x86_64 reports `code object is not signed at all`. Apple's arm64 linker
adds an automatic ad-hoc signature; it has no Developer ID or Team ID and is
allowed by the unsigned-release definition in the cross-platform PRD.

### Locating the Core

`CoreSettings::resolve_from` prefers the `MAGIES_SING_BOX_BIN` /
`MAGIES_SING_BOX_SHA256` runtime override, then falls back to a Core shipped
inside the artifact — beside the executable on Windows and Linux, in
`Contents/Resources` on macOS — checked against a digest baked into the app
binary at build time from `MAGIES_SING_BOX_SHA256`.

The digest is compiled in rather than read from a file next to the Core on
purpose: anything able to replace the Core binary can equally replace a digest
file beside it, so a sidecar pin would not be a pin at all. A bundled Core with
no compiled-in digest is refused rather than trusted.

## Consequences

- Every artifact contains its target's official sing-box 1.13.18 executable and
  license. The release workflow verifies the archive against the digest below,
  computes the extracted executable's digest, compiles that digest into MgClash,
  and packages the same executable.

  | Target | Official archive SHA-256 |
  | --- | --- |
  | macOS x86_64 | `500f0decfc21f7cdb2aaa4fe193b7857a41b07c38ee3a0b15bd53e3c7af3671c` |
  | macOS aarch64 | `9fbc05946b584423457a2778035e0cee2d9b239a4af5ae1932d9b79991149107` |
  | Windows x86_64 | `65045155ffdc506334f01a4353889657ddfc024f72b394081a9abaef34dfbef3` |
  | Linux x86_64 | `d34d987ed6ae39ca3760269264fb502b867e5477db45518c829b07776245c495` |

- The Windows artifact also contains the Authenticode-verified Wintun 0.14.1
  amd64 DLL and its license, per ADR 0002. Both the portable ZIP and NSIS
  installer also contain the pinned Core and both licenses.
- Gatekeeper and SmartScreen prompts are expected and documented in `README.md`
  and in the app's first-run notice; they are not defects.

## Verification

Built and checked on real macOS Intel hardware (`x86_64-apple-darwin`,
Core i7-9750H):

```text
mgclash-0.1.0-macos-x86_64-unsigned.tar.gz            23 MB
  MgClash.app/Contents/MacOS/magies-desktop           Mach-O 64-bit executable x86_64
  MgClash.app/Contents/Resources/sing-box             Mach-O 64-bit executable x86_64
  MgClash.app/Contents/Resources/LICENSE-sing-box     present
sing-box version                                      1.13.18
shasum -a 256 -c …tar.gz.sha256                       OK
codesign -dv MgClash.app                              code object is not signed at all
```

Actions run `31461933876` then built and uploaded all four target artifacts. The
downloaded artifacts were independently checked:

- every archive matched its `.sha256` sidecar;
- MgClash and sing-box had the expected arm64 or x86_64 format;
- every artifact contained sing-box and `LICENSE-sing-box`;
- each Core digest was present in its paired MgClash executable;
- macOS x86_64 was completely unsigned; macOS arm64 was linker-signed ad-hoc
  with no Team ID.
