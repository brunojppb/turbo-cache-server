# Release build and cache analysis

## Baseline and target architecture

The latest release run shared for this analysis took about 24 minutes overall: the Linux compilation step took 10m19s, the macOS compilation step took 7m04s, and the separate final Docker build took 11m59s. Those are the observed release baseline numbers. The local validation and hosted PR job timings below measure the new builds; a complete release run after merge is still needed to measure end-to-end release speedup. [Baseline release run](https://github.com/brunojppb/turbo-cache-server/actions/runs/40304310062)

The selected architecture removes the second Rust compilation and separates compilation from runtime-image assembly:

```text
Linux native matrix (amd64 + arm64, in parallel)
  ├─ Dockerfile artifact target: compile one static executable for that runner's native architecture
  ├─ GitHub Actions cache: separate linux-amd64 / linux-arm64 scopes, mode=max
  ├─ export the executable directly as a workflow artifact
  └─ smoke-check the artifact in the matching runtime image

Native Apple Silicon macOS job
  ├─ compile aarch64-apple-darwin
  └─ rust-cache keyed for the macOS ARM64 release job

Release publish
  ├─ use the collected Linux executables as input to Dockerfile.release
  ├─ package and push the multi-architecture GHCR image without Rust tooling
  └─ publish the three architecture-specific binaries
```

The Linux builds now run natively on `ubuntu-24.04` and `ubuntu-24.04-arm`. That removes the Linux cross-compilation toolchain (`zig`/`cargo-zigbuild`) from the Rust Docker builder and lets the two architectures compile concurrently. The job exports its binary using BuildKit's local output and does not push a temporary build image. `Dockerfile.release` then copies those exact artifacts into a scratch runtime image, so the image published to GHCR contains the same Linux executable bytes attached to the GitHub release. The macOS build uses the native Apple Silicon `aarch64-apple-darwin` target and Rust dependency caching.

This is the chosen design over the initial manifest-promotion recommendation. Promoting a compiled image manifest from the existing Docker Hub build repository would avoid a second compile, but it would keep an intermediate registry image and require Docker Hub credentials and publication in the release path. Direct artifact export removes that registry dependency and the associated secret while preserving one compile per architecture. Native Linux matrix jobs also make the independent architecture builds explicit and parallel. Both paths can eliminate duplicate Rust compilation; the artifact approach better fits the desired release inputs and avoids trusting a second registry as the handoff. The native builds are a design choice, not a measured performance claim.

## Cache configuration and alternatives

The Linux jobs use BuildKit's GitHub Actions cache backend, with stable architecture-specific scopes (`linux-amd64`, `linux-arm64`) and `mode=max`. Separate scopes prevent one architecture from replacing the other's cache. Pushes from `main` warm reusable caches; pull-request builds can restore caches allowed from the base/default branch, while GitHub's cache access rules still limit visibility across unrelated branches. GitHub documents `gha` cache scope as the identity for cache objects and documents that each scope can be overwritten by later exports, so the per-architecture scope is intentional.

`mode=max` includes intermediate build stages such as Cargo Chef's dependency cook layer. The backend is convenient for GitHub-hosted runners and avoids maintaining a registry cache repository or credentials. Its tradeoffs are GitHub cache quotas/eviction and API rate limits; Docker notes that frequent `gha` lookups can be rate limited. If cache size or rate limits become material, a registry cache is a valid alternative: export and import the same dedicated cache ref, set `mode=max`, and serialize writes or allocate refs by branch/architecture to avoid overwriting concurrent caches. The current selected workflow does not use a registry cache.

`Swatinem/rust-cache` applies to the macOS job because that job invokes Cargo directly on the GitHub runner. Its cache includes Cargo registry/git data and dependency build artifacts, and keys include the Rust compiler/host plus Cargo manifests, lockfile, toolchain, config, and selected environment. It does not automatically cache Cargo state inside a BuildKit container. Conversely, the Linux Docker build uses BuildKit's `gha` cache for Docker layers, which already includes Cargo Chef's dependency layer; adding rust-cache there would not share the runner's cache with the container build.

## Cargo Chef and version handling

Cargo Chef remains useful in the Linux Dockerfile: `prepare` creates a dependency recipe from manifests and the lockfile, and `cook` populates the release dependency layer before application source compilation. Its upstream README says that the recipe remains stable when dependencies remain unchanged and requires the same Rust version across stages. The planner currently copies source files as well as manifests before generating the recipe. Source edits can rerun the planner stage, but an unchanged recipe allows the later `cook` instruction to be reused; a narrower planner context is a possible small optimization, after the cache backend is measured.

Do not assume the release package-version rewrite invalidates the Cargo Chef dependency layer. Cargo Chef's upstream source explicitly masks versions of local packages to `0.0.1` in manifests and the lockfile when preparing the recipe. The observed recipe/build logs also show the dummy root package as `decay v0.0.1` even when the release package version is `4.0.21`. Therefore, the script's `Cargo.toml` and root `Cargo.lock` version update may invalidate an earlier Docker `COPY`/planner instruction, but should not change the normalized dependency recipe by itself. Dependency changes, Rust/toolchain changes, Dockerfile builder changes, build flags, or platform-specific build inputs can still invalidate dependency artifacts.

## Platform and packaging details

Rust documents `aarch64-apple-darwin` as the Tier 1 ARM64 macOS target for Apple Silicon, with a macOS 11.0 minimum. GitHub's standard `macos-14` and `macos-15` hosted runners use Apple Silicon M1. An explicit version label such as `macos-15` avoids silently changing the image when `macos-latest` advances. The universal2 binary is removed from the action and release naming; Linux ARM64 remains a separate `decay-arm64` artifact, while macOS uses `decay-darwin-arm64`.

The artifact flow keeps three identities aligned: `decay-x64` and `decay-arm64` are the action binaries and are also staged as the Linux release binaries; `decay-darwin-arm64` serves Apple Silicon macOS. The runtime image copies the Linux artifacts according to `TARGETARCH` and adds CA certificates. A smoke check compares the executable copied from the image with the original artifact, while architecture and ELF interpreter checks verify the expected CPU and static musl linkage before packaging.

## Risks and follow-up measurements

- GitHub currently documents standard Linux ARM64 hosted runners as public preview. Confirm availability and action compatibility for the organization. If that runner becomes unavailable, the same native job can use another ARM64 runner; avoid silently falling back to QEMU compilation because Docker warns that emulated compilation can be much slower.
- Standard GitHub-hosted macOS runners have fewer CPUs and memory than some larger runners. The first hosted PR macOS job completed in 4m33s; main-branch cache warming and a subsequent release are still needed to measure release cache reuse.
- GitHub Actions cache entries can be evicted and are limited by the repository/org cache policy. Review cache hit/miss output and cache size after a few main-branch runs; compare total runtime, not only compile time, because exporting/restoring `mode=max` has a transfer cost.
- The versioned Docker image and downloadable Linux release binaries now originate from the same exported build artifacts. The smoke test checks this for each build before artifacts are uploaded; keep it attached to the artifact-producing job.

## Primary sources

- Current repository implementation: [build-binaries.yml](../.github/workflows/build-binaries.yml), [release.yml](../.github/workflows/release.yml), [Dockerfile](../Dockerfile), [Dockerfile.release](../Dockerfile.release), [action/util.mjs](../action/util.mjs).
- Baseline release timing: [GitHub Actions run 40304310062](https://github.com/brunojppb/turbo-cache-server/actions/runs/40304310062).
- Cargo Chef recipe/cook behavior and same-toolchain requirement: [cargo-chef README](https://github.com/LukeMathWalker/cargo-chef); package version normalization: [cargo-chef version masking source](https://docs.rs/cargo-chef/latest/src/chef/skeleton/version_masking.rs.html).
- Docker BuildKit cache backends and cache mode: [cache backends](https://docs.docker.com/build/cache/backends/), [GitHub Actions backend](https://docs.docker.com/build/cache/backends/gha/), [registry backend](https://docs.docker.com/build/cache/backends/registry/).
- Docker's multi-platform build approaches and QEMU cost caveat: [multi-platform builds](https://docs.docker.com/build/building/multi-platform/).
- GitHub runner architecture labels and resource sizes: [choosing a runner](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job), [larger runners reference](https://docs.github.com/en/actions/reference/runners/larger-runners).
- Swatinem/rust-cache cache keys and cached paths: [rust-cache README](https://github.com/Swatinem/rust-cache).
- Rust Apple Silicon target and deployment target: [Apple Darwin targets](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html).


## Verification performed for this change

Required Rust formatting, Clippy and all-feature tests passed (38 unit, 1 settings, 29 HTTP tests). actionlint 1.7.12, six Node architecture-selection tests and two Python version-staging tests passed. Independent review identified Docker-tag-incompatible SemVer input, now covered by regression tests. The separate build context and root Dockerfile path were verified against build-push-action's argument construction and actual builds.

All hosted checks passed on [the first PR build](https://github.com/brunojppb/turbo-cache-server/actions/runs/41037876398): Linux AMD64 8m05s, Linux ARM64 7m21s, and macOS ARM64 4m33s, in parallel. This confirms native runner availability and successful GitHub cache export. It does not yet prove a warm release cache restore from main.

### Full local build validation

Tested commit `b988b88` on an Apple Silicon Mac, with an ARM64 Docker VM exposing 18 CPUs and about 32 GiB RAM. The initial three builds ran concurrently, so these are validation timings under contention rather than controlled hardware benchmarks. AMD64 Linux ran under emulation; the corresponding CI job runs natively.

The source snapshot used package version `0.0.0-local.1`. Both Linux architectures compiled from an empty, dedicated BuildKit builder, with no cache import, and exported `mode=max` caches. macOS used a fresh Cargo target directory, retaining the host's downloaded Cargo registry. A second dedicated BuildKit builder imported the exported caches, then built a source edit and version `0.0.0-local.2`. Both Cargo Chef cook layers were `CACHED`; only the application was compiled. The changed macOS build also compiled only the application.

| Target | Cold build | Source + version change | Unchanged rebuild |
| --- | ---: | ---: | ---: |
| Linux ARM64, native | 245.58s | 66.54s | 6.06s |
| Linux AMD64, emulated | 358.41s | 78.48s | 0.76s |
| macOS ARM64, native | 69.30s | 61.56s | 0.26s |

Cold Linux timings include tool installation and cache export; changed-build timings include cache restore and application compilation, without another cache export. The ARM64 and AMD64 exported caches occupied approximately 699 MiB and 705 MiB respectively. Local cache storage verifies intermediate-layer export/import across fresh builders, but does not model GitHub cache network latency or eviction. The remaining optimized application build/link is still a noticeable cost, even when dependency caching works.

Runtime and packaging checks all passed:

- Newly compiled Linux binaries have the expected ELF architecture and static linkage; the macOS executable contains only ARM64.
- Both newly built Linux runtime images passed HTTP health checks, CA certificate presence and byte-for-byte executable comparison. Input binaries were deliberately changed to mode 0644 before packaging to verify that `COPY --chmod=755` restores executable permissions.
- The native macOS executable passed an HTTP health check for both the initial build and the changed-version build.
- A complete two-platform OCI image archive was produced locally without a registry push. Both platform manifests, blob digests, executable bytes and mode, CA certificates, embedded release version, `HOST`, working directory and command were checked.
- Runtime assembly took 2.41s for ARM64 and 1.85s for AMD64; creating the multi-platform OCI archive took 1.33s with certificate layers already cached. No Rust compilation occurred during packaging.
- The standalone source-build Docker runtime target completed in 1.44s with its previously built artifact cached and passed the same health and byte-identity checks.
- Changed binaries contain `0.0.0-local.2`; unchanged Linux rebuilds preserve executable bytes.

The local artifacts, scripts and full logs are retained at `/tmp/decay-build-validation.23hySN` on the validation machine. Temporary builders were removed. No release or registry image was published. End-to-end release publication and warm GitHub cache restoration from main remain to be confirmed after merge.
