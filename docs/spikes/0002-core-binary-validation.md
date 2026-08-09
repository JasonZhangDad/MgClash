# Core Binary Locator and Validation

Date: 2026-08-09  
Status: B01/B02 scoped validation passed

## Scope

Implement the startup gate required by PRD tasks B01 and B02:

- resolve one configured Core path to its canonical path;
- require a regular file;
- identify the desktop executable format and CPU architecture;
- calculate and compare the complete file's SHA-256;
- return typed errors before process startup;
- prevent `CoreProcessSpec` construction from an unvalidated path.

The locator deliberately does not scan `PATH`, application folders, or the
whole system. Core download and installation will provide the configured path.

## Supported executable headers

| Format | x86_64 | ARM64 |
|---|---:|---:|
| Mach-O 64-bit | Yes | Yes |
| PE | Yes | Yes |
| ELF 64-bit | Yes | Yes |

The tests use small synthetic headers and run on every desktop CI target. Fat
Mach-O, 32-bit executables, and other CPU architectures fail closed as
unsupported instead of being guessed.

## Startup boundary

`locate_core_binary` returns a `ValidatedCoreBinary` only after path, file,
format, architecture, and SHA-256 checks pass. `CoreProcessSpec::new` accepts
that validated value rather than an arbitrary executable path. As a result, the
normal process runner cannot accidentally bypass the startup gate.

Failures are typed as:

- missing or unresolvable path;
- path is not a file;
- read failure;
- unsupported or malformed executable format;
- unsupported or mismatched architecture;
- SHA-256 mismatch.

## Real macOS Intel result

The existing ignored Intel smoke tests now validate each real Core before
launching it:

| Core | Format | Required architecture | Required binary SHA-256 | Result |
|---|---|---|---|---:|
| Xray 26.3.27 | Mach-O | `x86_64` | `afd0eaebb77994a18f29b00c5f50a4f7fbb77da06e24352d43035f3cad3c3786` | Pass |
| sing-box 1.13.18 | Mach-O | `x86_64` | `6e9749a4b40821bf07d301f099e75d871ea435861c9f5f0ac5687dc18e81b759` | Pass |

Both binaries passed validation, opened their configured local listener, and
were stopped and reaped by the shared runtime on a real Intel Mac.

## Remaining work

This task does not implement Core discovery policy, download, signature
verification, version command parsing, rollback, stdout/stderr capture, or crash
recovery. Those remain separate PRD tasks. Minimum macOS 13 compatibility also
still requires a matching test host; this validation ran on macOS 15.7.7.
