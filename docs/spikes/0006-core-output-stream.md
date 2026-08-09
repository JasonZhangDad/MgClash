# Core stdout/stderr Stream

Date: 2026-08-09
Status: B06 scoped validation passed

## Scope

The process runner now captures a Core's stdout and stderr from startup. Each
successful `CoreRuntime::start` returns a `CoreOutput` receiver with two typed
events:

- `Chunk`, containing the source stream and the original bytes;
- `ReadFailed`, containing the source stream and the original `io::Error`.

Output stays as bytes because Core output is not guaranteed to be UTF-8. Chunks
preserve the order within each pipe; no total ordering is promised between
stdout and stderr.

## Lifecycle

Dedicated readers continuously drain both pipes so a noisy Core cannot block on
a full operating-system pipe. The runtime joins the readers after stop, natural
exit, failed output setup, or drop. Readers keep draining if a caller discards
the receiver, preserving compatibility with callers that do not consume logs.

The receiver disconnects after both readers reach EOF. Reader I/O failures are
sent as typed events, while a reader panic becomes a typed runtime error during
poll or stop. Output persistence, parsing, filtering, redaction, and UI display
remain outside B06.

## Test result

Automated tests cover live stdout and stderr delivery, non-UTF-8 bytes, natural
exit and channel closure, a dropped receiver under output larger than pipe
capacity, and the typed read-error payload.

Official Xray 26.3.27 and sing-box 1.13.18 `darwin/amd64` binaries still opened
their local listeners and were stopped and reaped successfully on a real Intel
Mac with stdout and stderr piped through the new readers.
