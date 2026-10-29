# Upstream provenance and retired reference source

The active WGPUI 0.4.0 / Kit 0.8.1 implementation adapts upstream GPUI Kit v0.8.1,
commit `99576db58ec8103417c41a092e891b4022d24e7e`. The preceding imported v0.6.1
baseline was `36b51819deb52c947a79f8de29e0e9175eda7464`.

The separate historical `upstream-kit/` snapshot described GPUI Component/Kit
v0.6.0, pinned to `94a313a72a2513aee2780240cd322d552b2395f0`. Its 1,063 tracked
files were verified recoverable from repository commit
`40a9bc86ea12a7d30d6a34961a9bcc08e125bab4` before retiring that working-tree copy.
For example:

```sh
git show 40a9bc86ea12a7d30d6a34961a9bcc08e125bab4:upstream-kit/docs/ARCHITECTURE.md
```

Original histories, authorship and licenses remain intact. These current guides
adapt the historical architecture, styling/motion, design, coding, accessibility
and testing material authored by GPUI/Component/Kit contributors. The full old
English/Chinese website, skills, images and research remain recoverable in Git;
these guides do not claim to reproduce that site or provide translated copies.
The retained [Apache license](LICENSE-APACHE) applies to adapted material.

| Historical content | Current disposition |
| --- | --- |
| Architecture/design/coding/style/accessibility/testing guides | Adapted into this directory against current code |
| Theme catalog | Optional validated files in `crates/wgpui-component/themes/`, with author/source metadata |
| Tooltip edge and custom-title-bar examples | Adapted binaries in `examples/kit-hello-world/src/bin/` |
| Old FPS implementation | Retired; the active `crates/wgpui-fps` remains authoritative |
| Shell, component-shell, webview, browser gallery and JavaScript examples | Retired without adding platform support |
| Other historical examples, website, schema, scripts and skill snapshots | Recoverable historical source, not active build or package inputs |

The source move changes documentation/example ownership, not the active crate
layering. See [architecture](architecture.md) and the [package map](../kit.md).
