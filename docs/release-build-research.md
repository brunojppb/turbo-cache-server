# Release build and cache analysis

## Baseline and target architecture

The latest release run shared for this analysis took about 24 minutes overall: the Linux compilation step took 10m19s, the macOS compilation step took 7m04s, and the separate final Docker build took 11m59s. Those are the observed release baseline numbers. The local validation and hosted PR job timings below measure the new builds; a complete release run after merge is still needed to measure end-to-end release speedup. [Baseline release run](https://github.com/brunojppb/turbo-cache-server/actions/runs/40304310062)

The selected architecture removes the second Rust compilation and separates compilation from runtime-image assembly:

```text
Linux native matrix (amd64 + arm64, in parallel)
  ├─ Dockerfile artifact target: compile one static executable for that runner's native architecture
  ├─ GHCR registry cache: separate buildcache-amd64 / buildcache-arm64 tags, mode=max
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

This is the chosen design over the initial manifest-promotion recommendation. Promoting a compiled image manifest from the existing Docker Hub build repository would avoid a second compile, but it would keep an intermediate registry image and require Docker Hub credentials and publication in the release path. Direct artifact export removes the intermediate Docker Hub image dependency and its credential while preserving one compile per architecture. GHCR is used separately as an optional dependency cache; cache misses remain valid builds. Native Linux matrix jobs also make the independent architecture builds explicit and parallel. Both paths can eliminate duplicate Rust compilation; the artifact approach better fits the desired release inputs and avoids trusting a second registry as the handoff. The native builds are a design choice, not a measured performance claim.

## Cache configuration and alternatives

The Linux jobs export BuildKit registry caches to the existing public GHCR package, under `buildcache-amd64` and `buildcache-arm64`. The same ref is used for import and export, with `mode=max` to retain intermediate Cargo Chef dependency layers. Separate tags keep architecture caches independent. Cache data is separate from the deployable version and `latest` tags. Unlike GitHub Actions caches, registry tags are not subject to the seven-day inactivity eviction; keep these tags excluded from any package cleanup policy. Docker documents registry cache support for intermediate stages and non-image exporters such as the binary artifact target. [Registry cache](https://docs.docker.com/build/cache/backends/registry/), [GitHub cache retention](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#usage-limits-and-eviction-policy)

Only trusted `push` and `workflow_dispatch` builds on `main` authenticate and export the caches, using `GITHUB_TOKEN` with `packages: write`. The PR caller grants only `packages: read`; the main-branch cache-warming workflow and release caller grant `packages: write`. The reusable Linux job inherits these permissions, while the macOS job narrows its permissions to repository reads. Cache export depends on the guarded login step succeeding. PR builds restore anonymously from the existing public package and never export or receive registry credentials. Using tags in the existing public runtime package avoids a newly created private package that fork PRs could not read. Cache-writing jobs on the same ref are serialized per architecture, including main and release builds. A missing cache remains a valid cold build. [GHCR authentication and public pulls](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)

The earlier PR revision used GitHub Actions' `gha` backend. Although it avoids registry authentication, GitHub removes cache entries after seven days without access. The user's three-week idle-release scenario therefore requires a registry cache. The GHCR caches keep the dependency layers available between idle releases, provided that registry cleanup does not remove them. Toolchain, manifest, dependency or build-flag changes can still cause legitimate misses.

`Swatinem/rust-cache` remains on the native macOS job, which invokes Cargo directly rather than inside Docker. It caches downloaded crates and compiled dependencies with compiler/host and project configuration in the key. It still uses GitHub Actions storage and can expire after seven days without access. This change moves Linux Docker layers to GHCR; it does not make native macOS Cargo cache retention durable. [rust-cache](https://github.com/Swatinem/rust-cache)

## Cargo Chef and version handling

Cargo Chef remains useful in the Linux Dockerfile: `prepare` creates a dependency recipe from manifests and the lockfile, and `cook` populates the release dependency layer before application source compilation. Its upstream README says that the recipe remains stable when dependencies remain unchanged and requires the same Rust version across stages. The planner currently copies source files as well as manifests before generating the recipe. Source edits can rerun the planner stage, but an unchanged recipe allows the later `cook` instruction to be reused; a narrower planner context is a possible small optimization, after the cache backend is measured.

Do not assume the release package-version rewrite invalidates the Cargo Chef dependency layer. Cargo Chef's upstream source explicitly masks versions of local packages to `0.0.1` in manifests and the lockfile when preparing the recipe. The observed recipe/build logs also show the dummy root package as `decay v0.0.1` even when the release package version is `4.0.21`. Therefore, the script's `Cargo.toml` and root `Cargo.lock` version update may invalidate an earlier Docker `COPY`/planner instruction, but should not change the normalized dependency recipe by itself. Dependency changes, Rust/toolchain changes, Dockerfile builder changes, build flags, or platform-specific build inputs can still invalidate dependency artifacts.

## Platform and packaging details

Rust documents `aarch64-apple-darwin` as the Tier 1 ARM64 macOS target for Apple Silicon, with a macOS 11.0 minimum. GitHub's standard `macos-14` and `macos-15` hosted runners use Apple Silicon M1. An explicit version label such as `macos-15` avoids silently changing the image when `macos-latest` advances. The universal2 binary is removed from the action and release naming; Linux ARM64 remains a separate `decay-arm64` artifact, while macOS uses `decay-darwin-arm64`.

The artifact flow keeps three identities aligned: `decay-x64` and `decay-arm64` are the action binaries and are also staged as the Linux release binaries; `decay-darwin-arm64` serves Apple Silicon macOS. The runtime image copies the Linux artifacts according to `TARGETARCH` and adds CA certificates. A smoke check compares the executable copied from the image with the original artifact, while architecture and ELF interpreter checks verify the expected CPU and static musl linkage before packaging.

## Risks and follow-up measurements

- GitHub currently documents standard Linux ARM64 hosted runners as public preview. Confirm availability and action compatibility for the organization. If that runner becomes unavailable, the same native job can use another ARM64 runner; avoid silently falling back to QEMU compilation because Docker warns that emulated compilation can be much slower.
- Standard GitHub-hosted macOS runners have fewer CPUs and memory than some larger runners. The first hosted PR macOS job completed in 4m33s; main-branch cache warming and a subsequent release are still needed to measure release cache reuse.
- Keep the GHCR buildcache tags excluded from cleanup policies. `mode=max` export/restore has a transfer cost; compare total runtime and cache hit output over subsequent releases. The native macOS Cargo cache still has GitHub Actions retention limits.
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

All hosted checks passed on [the first PR build](https://github.com/brunojppb/turbo-cache-server/actions/runs/41037876398): Linux AMD64 8m05s, Linux ARM64 7m21s, and macOS ARM64 4m33s, in parallel. This validated native runner availability and the earlier GitHub cache export configuration. The GHCR cache revision needs separate validation below.

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

The local artifacts, scripts and full logs are retained at `/tmp/decay-build-validation.23hySN` on the validation machine. Temporary builders were removed. No release or registry image was published. End-to-end release publication and warm GHCR cache restoration from main remain to be confirmed after merge.


### GHCR cache revision verification

The revised workflows passed actionlint 1.7.12 and independent review of caller/reusable token permissions, trusted writer conditions, architecture refs and writer serialization. Required formatting, Clippy and all-feature Rust tests passed again, along with the Node and Python packaging contract tests.

Both architecture caches were exported with `type=registry,mode=max` to separate tags in an isolated local OCI registry. A second fresh BuildKit builder imported only those registry refs, restored Cargo Chef's intermediate dependency layers (`CACHED`), performed no Rust compilation and reproduced the original executable bytes for both AMD64 and ARM64. The test registry and builders were removed; logs and restored executables remain at `/tmp/decay-registry-validation.qd8XPt` on the validation machine.

An anonymous token/manifest request to the existing public GHCR package returned HTTP 200. Live GHCR writes were not attempted because the local CLI credential lacks package scope. The first trusted main-branch warming run after merge will authenticate with the workflow's `GITHUB_TOKEN` and populate the two persistent cache tags; PR validation intentionally has read-only registry access. GHCR upload and subsequent hosted restoration still need confirmation through that trusted path. No runtime image tag or release was changed by this cache revision.
