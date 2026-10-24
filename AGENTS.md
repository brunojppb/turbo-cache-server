# Agent guidance

Read this file before changing the repository. Keep these instructions short and durable; use the code, tests, and README for details that change with features.

## What this repository does

`decay` is a Rust server for Turborepo's remote cache protocol. It serves artifact requests with Actix Web and stores artifact bytes and metadata in an S3-compatible bucket. The repository also ships a Docker image and a GitHub Action. Preserve the existing Turborepo contract when changing routes, headers, status codes, or artifact keys.

## Map

- `src/startup.rs` composes the Actix app, routes, middleware, settings, and storage.
- `src/routes/` handles the HTTP contract; `src/routes/artifacts.rs` maps artifact requests, responses, and metadata.
- `src/storage.rs` owns S3 operations and translates storage failures.
- `src/auth/` handles the optional shared `TURBO_TOKEN`.
- `src/app_settings.rs` reads environment settings; `src/telemetry.rs` sets up tracing and metrics.
- `tests/e2e/` exercises the server over HTTP with a Wiremock S3 endpoint.
- `action/`, `Dockerfile`, and `.github/workflows/` package, test, and release the server.

## Design rules

- Keep this one crate unless a concrete change needs a new module or crate. Put domain decisions in Rust types and functions close to the behavior they govern. Do not copy a database-oriented DDD layer structure into this server.
- Keep HTTP parsing, status codes, and response shapes at the route seam; keep bucket access and S3 error translation in `storage.rs`. Wire concrete dependencies in `startup.rs`.
- Preserve streaming for artifact uploads and downloads. Do not buffer whole artifacts merely to simplify a change.
- Treat request paths, query values, headers, and stored metadata as untrusted. Keep team and artifact key handling consistent across PUT, GET, and HEAD. A team name is a storage namespace, not an authenticated account: `TURBO_TOKEN` is one optional shared server token.
- Do not log tokens, credentials, artifact contents, or unbounded client-supplied values. Keep secrets out of `Debug` output and tracing fields.
- Prefer an existing dependency or a small local function before adding a crate. Record a consequential architecture choice and its reason in the PR or a short ADR when it needs lasting explanation.
- Add a focused regression test for changed behavior. A prose rule that can be checked reliably should become a test or CI check when it matters.

## Working on changes

Use the pinned Rust 1.99.1 toolchain in `rust-toolchain.toml`; `Cargo.toml` declares the same minimum. Run commands from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked -- -D warnings
cargo test --all-features --locked
```

The HTTP tests use Wiremock and do not need a live S3 service. They may still need loopback binding and uncached crates on a new machine. See `.agents/skills/cache-server-verify/SKILL.md` when choosing proof for a behavior change. Report any check you could not run and why; do not call it passing.

Keep PRs focused. Explain the behavior or instruction change, the reason for it, the checks run, and any remaining verification limit. Do not commit generated binaries or local credentials as part of a guidance change.
