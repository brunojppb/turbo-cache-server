---
name: artifact-key-formal-verify
description: Use when changing ArtifactId::new, is_key_segment, object_path, how routes build a team or hash, or the rust-s3 version. Checks the artifact key namespace over every value in a stated sweep.
---

# Verify the artifact key namespace

## Property

Every `ArtifactId` that `ArtifactId::new` accepts maps to exactly one S3 object key, `/<team>/<hash>`. The request URL for that key stays inside the bucket. No two accepted ids share a key.

A break lets one team read or overwrite the artifacts of another team, or reach outside the bucket path.

## Run

```sh
cargo test --locked --lib artifact_key_namespace
```

It runs in about one second after the first build. The test is `storage::tests::artifact_key_namespace` in `src/storage.rs`.

## Scope

- The tool is an exhaustive sweep, not a prover. Kani is not installed here, and string formatting and URL parsing are poor fits for it.
- The sweep builds values from up to 3 tokens of `a`, `.`, `%2e`, `%`, `/`, `\`, `?`, `#`. That is 585 values for `team` and for `hash`, so about 342,000 pairs.
- For each accepted pair the test calls the real `ArtifactId::new` and `object_path`. It then builds the URL the way `rust-s3` 0.38 does: `s3::signing::uri_encode` on the path, then `url::Url::parse`.
- It asserts three path segments with the first one equal to the bucket name. It asserts the same host and no query or fragment. It asserts that no two pairs give the same key.
- Not covered: values longer than 3 tokens, other characters, non-ASCII input, how S3 itself treats a key, and what the HTTP route does before it calls `ArtifactId::new`.

## Known result

The current code passes. Broken variants that the test catches:

- `is_key_segment` without `".."`. Counterexample: team `a`, hash `..` resolves to `/bucket/`.
- `is_key_segment` without the `/` and `\` check. Counterexample: team `a`, hash `/` resolves to `/bucket/a//`.
- `object_path` that drops the middle `/`. Counterexample: team `a`, hash `a` resolves to `/bucket/aa`, which has two segments.

One variant passes: `is_key_segment` without the `\` check alone. `rust-s3` encodes `\` as `%5C`, so the URL stays in the bucket. The `\` check is a second layer for a different URL builder. Keep it.

## When a check fails

1. Read the pair in the panic message. It is a `(team, hash)` value in the test's own terms.
2. If you changed `is_key_segment` or `object_path`, the product code is wrong. Fix it, or ask the user if the key layout change is on purpose.
3. If you upgraded `rust-s3`, compare its `Request::url` with the test's URL build. A new URL build is a modeling error in the test. Update the test to match, then check that the broken variants above still fail.
4. Never remove a token or lower `MAX_TOKENS` to make the test pass.

## Keep in step with the code

Covered code:

- `ArtifactId::new` and `is_key_segment` in `src/domain.rs`
- `object_path` in `src/storage.rs`
- `Request::url` and `signing::uri_encode` in `rust-s3` 0.38

Last confirmed at commit `c832284`. Move this commit forward only after you confirm that the test still follows the covered code at the new commit.
