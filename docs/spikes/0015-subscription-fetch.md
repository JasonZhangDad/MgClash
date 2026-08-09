# Subscription Fetch

Date: 2026-08-09
Status: C07 scoped validation passed

## Scope

C07 adds an asynchronous `SubscriptionFetcher` to `magies-profiles`. It only
retrieves raw subscription bytes; protocol parsing, deduplication, database
transactions, stale-node handling, and UI status belong to C08 and later work.

The fetcher follows the PRD's conditional-request and timeout requirements and
uses reqwest 0.13.4:

- https://docs.rs/reqwest/0.13.4/reqwest/struct.ClientBuilder.html
- https://docs.rs/reqwest/0.13.4/reqwest/struct.Response.html
- https://docs.rs/reqwest/0.13.4/reqwest/struct.Error.html

It sends stored `ETag` and `Last-Modified` validators and returns either updated
bytes or an HTTP 304 result. Missing response validators are retained only for
304; a successful new representation does not inherit stale validators.

## Safety limits

Defaults are a 15-second total timeout, five redirects, and an 8 MiB response
limit. Callers can lower or raise them explicitly. The body is read in bounded
chunks, so a server cannot bypass the limit by omitting or lying about
`Content-Length`.

Only HTTP and HTTPS URLs are accepted. HTTPS uses Rustls with Ring and platform
certificate verification, avoiding an OpenSSL runtime dependency for unsigned
macOS Intel/ARM, Windows x64, and Linux x64 builds.

Subscription URL query tokens and response bodies are excluded from typed
errors and debug output. Reqwest errors have their URL removed before they can
cross the module boundary; server error bodies and invalid header values are
never copied into errors.

## Test result

Ten integration tests cover updated and 304 responses, conditional headers,
relative redirects, timeout, HTTP errors, known and chunked body limits,
invalid inputs and validators, truncated bodies, invalid response headers, and
debug redaction. `subscription.rs` has 93.91% line coverage; the Rust workspace
has 93.15% line coverage.
