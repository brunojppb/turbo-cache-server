---
name: cache-server-verify
description: Use when changing or reviewing this Rust cache server's behavior, Turborepo artifact protocol, S3 storage, authentication, settings, or release packaging.
---

# Verify cache server

Choose proof at the seam that changed, then run the repository checks. Read `AGENTS.md` first.

## Choose the proof

| Change | Focused proof |
| --- | --- |
| Parsing, metadata conversion, settings, or auth token comparison | Unit test beside the affected Rust code |
| Route, status, header, token middleware, or streamed artifact behavior | HTTP test in `tests/e2e/` using the Wiremock S3 endpoint |
| S3 request, error mapping, encryption, or metadata persistence | Storage test and, when visible to a client, an HTTP test asserting the Wiremock request and response |
| Docker, Action, or release packaging | Inspect the affected workflow and run its relevant local build or check; report any platform or credential-dependent step that was not run |

For a bug, reproduce the observed failure in a test before the fix when practical. Assert the client-visible result and the relevant S3 request; a successful status alone may miss missing bytes or headers. Cover malformed or missing input and storage failures when they are part of the changed behavior. Keep the optional shared-token behavior in mind: test configured and unconfigured modes when auth is affected.

## Run checks

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked -- -D warnings
cargo test --all-features --locked
```

Start with the focused test while iterating. Run the full checks before delivery. The end-to-end suite starts an HTTP server and Wiremock on loopback; it does not require AWS, Docker, or a live S3 bucket. If a check cannot run because crates are unavailable, networking is restricted, or loopback is blocked, report the command and exact cause. Do not treat an unrun check as a pass.

## Report proof

State which behavior changed, the focused test that exercises it, the full check results, and any unverified condition. For protocol changes, name the affected method, path, headers, status, and stored metadata or bytes so a reviewer can compare them with the existing contract.
