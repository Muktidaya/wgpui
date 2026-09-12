# WGPUI Kit and workspace migration

Date: 2026-09-11. State: inspected plan; implementation has not started.

Workspace update later on September 11: the separately requested Developer flattening has moved these checkouts to `Developer/wgpui` and `Developer/wgpui-component`. GitHub locations are unchanged. That local relocation does not implement the Kit consolidation or release described here. Current paths below reflect the flat layout; the original owner directories are retained in recovery records.

## Objective and source

Continue the [shared WGPUI Migration Plan conversation](https://chatgpt.com/share/6aa48fa6-c6a4-83e8-96fd-d499bf0778d2) using the actual Mac checkouts. Consolidate WGPUI's supporting crates under `crates/`, adopt consistent `wgpui-*` package naming where the registry permits it, incorporate the component repository, and deliver WGPUI Kit with the relevant API and behavior of upstream 0.6.1.

The original request was to write a plan. This document proposes implementation and release steps; it does not record them as completed or authorize new external actions by itself.

## Verified starting state

| Item | Observation on 2026-09-11 |
| --- | --- |
| Framework checkout | `/Users/muk/Developer/wgpui` |
| Framework HEAD | `080f7351a5c5f9f635644026bbd2d20aa80527d8` |
| Framework package | `wgpui` 0.3.5; Rust 1.94; resolver 3 |
| Component checkout | `/Users/muk/Developer/wgpui-component` |
| Component HEAD | `811f2745bca20ee51285b114d39d43cc297c5930` |
| Component upstream baseline | `94a313a72a2513aee2780240cd322d552b2395f0`, tag v0.6.0 |
| Target upstream | `36b51819deb52c947a79f8de29e0e9175eda7464`, tag v0.6.1 |
| Release delta | GitHub compare reports 68 commits; returned file list is capped at 300 and is not a complete diff inventory |
| GitHub origins | `Muktidaya/wgpui` and `Muktidaya/wgpui-component`, both **public**, default branch `root`, verified through GitHub API |
| Component upstream remote | Still uses `https://github.com/longbridge/gpui-component.git`; canonical target is `longbridge/gpui-kit` |
| Framework tooling | Tracked `derive/`, `perf/`, and `tests/action_macros.rs`; no additional ignored tooling paths reported by scoped Git status |
| Workspace members | Framework includes root and derive only; component includes base, macros, ui, assets, platform, harness, hello_world |

Both checkouts have existing changes. Framework has a modified STATUS.md. Component has context-maintenance modifications, deleted retired assistant files, and an untracked `.agents/` directory. The Developer umbrella also has unrelated changes and both repositories are registered submodules. Preserve these states explicitly.

The component STATUS.md's private-repository description is stale. Its September 5 publication and test results are historical evidence, not fresh release or test verification in this pass.

## Architectural correction: Kit is a façade

[Upstream v0.6.1 Kit](https://github.com/longbridge/gpui-kit/blob/v0.6.1/crates/kit/Cargo.toml) is an application-facing façade over framework, base, styled components and assets, with feature forwarding and test support. `gpui-component` still exists. The local 0.6.0 tree already contains `crates/kit`, but excludes it from the compiled workspace.

Recommended interpretation: move the component project into the WGPUI repository, make **wgpui-kit** the recommended application dependency, and retain a separate styled-component implementation crate. Merely changing the existing styled library's package name to `wgpui-kit` would not reproduce upstream's façade or feature contract.

Proposed single Git repository and Cargo workspace:

```text
wgpui/
  Cargo.toml                 # wgpui framework + workspace root
  Cargo.lock
  src/
  crates/
    wgpui-derive/
    wgpui-perf/
    wgpui-kit/               # application façade, adapted from upstream kit
    wgpui-base/
    wgpui-component/         # styled implementation used by Kit
    wgpui-kit-macros/
    wgpui-kit-assets/
    wgpui-platform/          # existing shim, release eligibility audited
    wgpui-kit-harness/       # unpublished integration harness
  examples/
  docs/
```

Other on-disk upstream packages (fps, shell, component-shell, webview, stories and WASM examples) receive individual inventory entries and destinations. Production packages that are adopted use `wgpui-*` names. Retained reference packages remain explicitly excluded and unpublished until ported; retaining source is not feature parity. Do not invent crates for folders without Cargo manifests: `tooling/tests/action_macros.rs` belongs in the appropriate existing integration-test target.

Keep framework and Kit versions independent. Candidate Kit version is 0.6.1. Choose the next framework/derive version after checking published versions and the actual compatibility delta; do not reuse an immutable release or synchronize unrelated version series.

## Derive spelling requires a registry decision

The published name is `wgpui_derive`. crates.io treats hyphens and underscores as equivalent for uniqueness, so `wgpui-derive` cannot simply be registered as a separate crate. See the [Rust project's explanation](https://blog.rust-lang.org/2023/10/27/crates-io-non-canonical-downloads/).

Move its directory to `crates/wgpui-derive` regardless. First verify whether crates.io offers an applicable canonical-name change for the existing package; do not assume owner access makes it possible. Until resolved, the workable compatibility form is:

```toml
wgpui-derive = { package = "wgpui_derive", path = "crates/wgpui-derive", version = "<verified version>" }
```

Rust still imports this dependency as `wgpui_derive`. This gives consistent directory/dependency-key spelling but **does not** accomplish the requested canonical registry rename. Record that exception explicitly. Do not delete or yank existing releases as a rename mechanism.

## Execution sequence

### 1. Preserve state and establish a reproducible baseline

- Record branches, full HEADs, remotes, submodule gitfiles, `core.worktree`, status, tracked diffs, untracked files and relevant ignored source in both checkouts and the umbrella.
- Create verified Git bundles for committed refs and separate recoverable copies of the uncommitted source changes; a bundle alone does not preserve them. Keep private administrative records outside published packages.
- Work in isolated migration checkouts. Preserve the live checkouts until replacement is verified; do not stash, reset or clean unrelated work.
- Read current repository guidance and canonical component architecture/design/coding guides. Keep the no-style Base/presentation split and WGPUI backend contract.
- Run baseline framework tests, component/base/harness tests and current hello_world checks under Rust 1.94.0. Record pre-existing failures separately.

### 2. Port the exact upstream release before flattening paths

Fetch the pinned tag into the isolated component checkout and produce a complete local `git diff --name-status` against the recorded 0.6.0 baseline. GitHub's 300-file comparison response is insufficient. Merge/adapt the release while the old directory mapping is still recognizable; keep renames separate from behavior changes where possible.

The 68-commit range includes substantial work, including multi-cursor editing, column selection and CRLF boundaries, bracket completion and indentation, search behavior, markdown line breaks/frontmatter, text measurement and drag autoscroll, resizing, accessibility, icon bytes/default assets, monospace fallback, and Kit headless integration tests. These are a triage inventory derived from commit subjects, not assertions that every implementation has been reviewed.

Preserve adapter fixes around text refinement, scroll following, flex sizing, reduced motion, animation/test-clock behavior and macro reflection until tests establish that an upstream or framework change supersedes each one. Do not restore Zed/gpui-pre native platform dependencies simply to satisfy compilation. Port test-support contracts onto WGPUI's backend.

Create a machine-readable parity table keyed by upstream package/feature/API or behavior with target location, implementation status, test evidence and divergence rationale. Include default features, no-default-features, assets, inspector, test-support, decimal, language features, platform-specific paths and optional native/browser integrations. WASM, webview and shell support cannot be silently omitted from an unqualified full-parity claim. Record any WGPUI backend limitation as an open acceptance gate.

### 3. Consolidate history and filesystem structure

Import the component project's history into the isolated WGPUI repository without squashing away its ancestry. Use a dedicated import commit with a second parent and a prefixed tree, then separate path-mapping commits to flatten the crates. Retain original commit IDs/refs in recovery records and verify imported ancestry and file hashes. Namespace imported release tags to avoid collisions; local component `v0.6.0` is an upstream tag, not its adapter publication tag.

Map derive and perf to the new canonical paths. Repair every Cargo path, build-script resource path, include path, profile entry, test target, script and CI working directory. Move associated READMEs, licenses, assets and tests together. Remove the nested workspace declaration when integrating component members; reconcile workspace dependencies, resolver, profiles and lints at the parent root and regenerate a single lockfile deliberately.

`perf` currently fails even Cargo metadata because it inherits an undefined workspace edition. Supply valid edition/dependencies, rename package and binary to `wgpui-perf`, update its `use perf` and command/path assumptions, and test its actual runner against a small fixture. Keep `publish = false` until a supported public tool is intended. Merely including it as a member does not establish runtime correctness. Review the orphaned action-macro test's stale macro import before activating it.

### 4. Adopt the Kit entry point and migrate consumers

Adapt upstream `crates/kit` to `wgpui-kit`, including re-exports, initialization, feature forwarding, macros and integration tests. Preserve `wgpui-component` as the implementation package for upstream structural correspondence and existing consumers. New applications use `wgpui_kit`; document old/new dependency and import examples.

Rename component assets/macros to the proposed Kit package names where this does not compromise compatibility. Inspect generated macro paths and the assets `links` value: an old/new asset package pair must not introduce a conflicting duplicate native-link identity into one dependency graph. Decide whether compatibility packages are necessary after checking consumers; avoid wrappers that duplicate assets or create dependency cycles.

Confirmed local consumer: `Geistwerk/lab/Cargo.toml` references the component UI and platform directories directly. Update those paths/API imports and test `native`. Also inspect SDK, CAD and all tracked Cargo manifests for framework, derive, Kit and platform dependencies, Git URLs and CI checkout layouts. Scope searches to source/configuration; exclude private operational data and historical archives from mechanical replacement.

### 5. Reconcile GitHub, submodules and workspace operations

Recommended canonical source remote remains `https://github.com/Muktidaya/wgpui.git`. A separate active `wgpui-kit` source repository would compete with the requested single-repository layout. If the old component repository is renamed to `wgpui-kit` for link continuity, treat it as a migration landing/archive repository pointing to WGPUI, not a second source of truth; make that transition only at delivery.

Preserve the old repository's history, issues and releases. Do not delete it. Update upstream metadata to `longbridge/gpui-kit`; use a distinct upstream remote name in the consolidated repository so it is not mistaken for the framework's upstream.

At verified cutover, update Developer `.gitmodules` and exactly the affected gitlinks: Kit is ordinary source inside WGPUI in the proposed design, not another submodule. Preserve `.git/modules` stores used for rollback. Update `repos.json`, bootstrap/dependency checks, CI checkouts, docs and known consumers to the unified source. Only record exact tested commits; historical archive references stay historical. Confirm a fresh recursive clone and bootstrap no longer expect the old sibling component checkout.

### 6. Verify, then release in dependency order

Require all of the following, with commands/results attached to the implementation checkpoint:

- Cargo metadata resolves every intended member and target; excluded sources are listed explicitly.
- Framework/derive tests, base/component tests, Kit feature-matrix tests, harness and examples pass. Use the framework's `scripts/clippy` guidance and appropriate formatting/doc tests.
- Meaningful interaction tests cover editor changes, focus/keyboard/selection, overlays, layout/scrolling, assets, initialization and headless Kit APIs. Real native smoke tests establish behavior that headless tests cannot.
- Linux, macOS and Windows CI pass on the exact candidate commit. Platform-specific unsupported features remain open rather than counted as passed.
- Lab native and affected SDK/CAD consumer checks pass against the same dependency baseline.
- Package file lists retain required licenses, assets and source, exclude administrative artifacts, and have no accidental sibling-only dependencies. Registry-only consumers build from a fresh directory.
- Every upstream parity-table row is accounted for. Compilation alone is not parity, and a reduced workspace is not full upstream acceptance.

Derive (if changed) precedes framework; macros and other leaf dependencies precede base/component; Kit is last. Generate the final order from actual Cargo metadata, including assets and platform dependencies, rather than assuming the old four-package release order still applies. A publishable Kit cannot depend on an unpublished required shim. Resolve that packaging boundary before release.

Dry-run packaging first, then release only the reviewed candidate under the active session's authorization. Verify registry versions, source tags, docs.rs and a registry-only smoke application after publishing. Preserve existing releases without yanking. Use distinct framework and Kit tags.

## Inspection evidence and remaining work

Fresh evidence in this planning pass: local Git/manifest/tooling inventory; current GitHub visibility/default branches; exact v0.6.1 SHA and 68-commit count; upstream Kit manifest; reproduced perf metadata failure.

`cargo +1.94.0 test --offline --locked -p wgpui_derive --lib` completed successfully in 1m 07s, compiling derive and its framework development dependency with warnings. The selected target contains **zero tests**: this establishes compilation only, not behavioral or integration-test acceptance. Existing tracked diff whitespace validation passed; the only new file is this plan.

Baseline command reproducing the perf failure:

```sh
cargo +1.94.0 metadata --offline --no-deps --format-version 1 \
  --manifest-path /Users/muk/Developer/wgpui/tooling/perf/Cargo.toml
```

Result: exit 101, undefined `workspace.package.edition`. This is a pre-existing problem.

No migration, repository rename, remote update, submodule cutover, commit, push or publication has been performed. Full local tests, complete local upstream diff review, feature parity, cross-platform CI and release validation remain implementation work. First implementation checkpoint: verified recovery snapshots, isolated checkouts, complete release diff and baseline results.
