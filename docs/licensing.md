# WGPUI 0.4.0 licensing inventory

The framework's Apache-2.0 terms and retained Zed/gpui-pre copyright notice are
in [LICENSE](../LICENSE). The combined file preserves the original GPUI license
text. Base, Component and Kit retain their Longbridge notices and full terms in
each package's `LICENSE-APACHE`; historical Kit adaptations are described in
[upstream provenance](kit/upstream-provenance.md).

Separately identified portions keep their own terms. These notices describe
scope, rather than offering a choice between licenses for all repository code.

| Material | Applicable notice |
| --- | --- |
| Microsoft Terminal gamma-ratio adaptation | [MIT terms](../src/platform/microsoft_terminal_license.txt), with the exact source revision |
| smithay-clipboard adaptation | [MIT terms](../src/platform/clipboard_licenses.txt) |
| Bundled Lucide and Feather-derived icons | [ISC/MIT notices](../crates/wgpui-kit-assets/LICENSE-LUCIDE) |
| Dragon example illustration by Ebaychatter0 | [CC-BY-SA-3.0 attribution and terms URI](../examples/legacy/svg/NOTICE.txt) |
| Original geometric example fixtures | [Apache-2.0 fixture provenance and regeneration](../examples/legacy/image/README.md) |
| Optional theme catalogs | [Source-specific notices and open scope findings](../crates/wgpui-component/themes/SOURCE-NOTICES.md) |

Original contributor commit histories, author/committer records and notices
remain intact. Contributor integration adaptations are separate commits. Retired
historical reference files and replaced demo images remain recoverable in Git.

## Dependency and distribution scope

The current locked, all-feature, all-platform workspace graph contains 869
registry package instances and 13 workspace packages. This includes optional,
build and development dependencies, and is not the dependency set of every
individual crate or binary. The workspace packages retain their own versions:
framework/derive 0.4.0, Kit/FPS 0.8.1 and supporting 0.1.0 packages.

The source crates refer to Cargo dependencies rather than vendoring all of their
source. A downstream binary distribution must assess its actual feature/platform
closure and retain the applicable dependency notices. Native SDKs, shared system
libraries and installed fonts have separate distribution scopes. No font files
are bundled in this repository's current source packages.

The audit distinguishes simple permissive choices from combined obligations,
including Ring/AWS-LC component notices, Unicode data, LLVM exceptions,
`option-ext`'s MPL-2.0 files and WebPKI root data under CDLA-Permissive-2.0.
An `OR` alternative is not a requirement to apply every offered license.

## Open release-audit findings

The exact locked source archives for `dispatch 0.2.0`, `mac 0.1.1`,
`seahash 4.1.0` and `simd_helpers 0.1.0` contain license declarations but no
complete license/permission text located by the audit. Available source headers
and upstream revisions were checked; these remain notice/provenance questions,
not findings of incompatibility.

The optional theme catalog has the source-scope questions listed in its
[SOURCE-NOTICES](../crates/wgpui-component/themes/SOURCE-NOTICES.md). Ayu's
MIT notice at the exact referenced Zed revision is resolved independently of
Zed's repository-root AGPL terms. The iTerm collection's MIT notice expressly
reserves the rights in individual themes.

Package SPDX defaults describe the Rust source license; they must not be read
as relicensing separately identified icons, illustrations or theme material.
The package-level representation of mixed contents remains a release review
item. Archive/source integrity verification is separate from permission and
notice review. This inventory does not declare blanket licensing acceptance.

### Qualified SeaHash follow-up

The source mirror `redox-unofficial-mirrors/seahash` adds a complete MIT text
in commit `3088c5c912b70b586d27bf553fbe964e025a2c89`, immediately after the
locked source revision `94b632aeac099031c373599313d5b5f0acbbaec0`. Its sole
change is the license file; the text contains no copyright line. This supplies
qualified mirrored-history evidence, rather than primary GitLab verification.
A copyright notice from another Ticki project must not be substituted for a
missing SeaHash notice. The pinned archive's missing text and primary-source
verification question remain recorded separately.
