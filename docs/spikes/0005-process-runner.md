# Process Runner Launch Boundary

Date: 2026-08-09
Status: B05 scoped validation passed

## Audit result

The Phase 0 process runner already covered the B05 lifecycle baseline:

- start one validated Core and reject duplicate starts;
- poll running and exited states;
- wait with a bounded timeout;
- force-stop and reap the child process;
- clean up a running child when the runtime is dropped;
- return typed operating-system errors;
- restart after a stop or natural exit.

Streaming output, health checks, graceful shutdown, and crash recovery remain
separate B06-B08 tasks.

## Gap and fix

`CoreProcessSpec` previously copied only the path from a
`ValidatedCoreBinary`. A Core file changed or deleted after spec construction
could therefore reach the operating-system spawn call without another
architecture or SHA-256 check.

The spec now retains the complete validated binary identity. Immediately before
every spawn, the runner resolves and validates the file again using the original
architecture and SHA-256. A changed or missing binary fails with
`CoreRuntimeError::BinaryValidationFailed`, and the runtime remains stopped.

This check is not a substitute for keeping Core artifacts in an
application-controlled directory: all path-based validation has a small
check-to-execution race if another process can replace the file concurrently.

## Test result

Cross-platform lifecycle tests cover deleted and hash-modified Core files,
duplicate starts, restart, early exit, timeout, forced stop, and process cleanup.
Unix tests also preserve coverage of a genuine permission-based spawn failure.

Official Xray 26.3.27 and sing-box 1.13.18 `darwin/amd64` binaries passed the
new launch-time revalidation, opened their local listeners, and were stopped and
reaped successfully on a real Intel Mac.
