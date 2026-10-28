# Theme source notices

Catalog adaptations retain the historical Longbridge Apache notice in
`LICENSE-APACHE`. Palette/source notices below remain separately applicable;
a repository-wide license does not override a more specific directory notice.
Names, authors and source URLs in the JSON files are preserved.

| Catalog file | Retained notice | Source scope |
| --- | --- | --- |
| `adventure.json` | [licenses/iterm2-collection.txt](licenses/iterm2-collection.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `catppuccin.json` | [licenses/catppuccin.txt](licenses/catppuccin.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `everforest.json` | [licenses/everforest.txt](licenses/everforest.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `fahrenheit.json` | [licenses/iterm2-collection.txt](licenses/iterm2-collection.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `flexoki.json` | [licenses/flexoki.txt](licenses/flexoki.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `harper.json` | [licenses/iterm2-collection.txt](licenses/iterm2-collection.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `kibble.json` | [licenses/iterm2-collection.txt](licenses/iterm2-collection.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `macos-classic.json` | [licenses/macos-classic.txt](licenses/macos-classic.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `mellifluous.json` | [licenses/mellifluous.txt](licenses/mellifluous.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `molokai.json` | [licenses/molokai.txt](licenses/molokai.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `spaceduck.json` | [licenses/spaceduck.txt](licenses/spaceduck.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `tokyonight.json` | [licenses/tokyonight.txt](licenses/tokyonight.txt) | Current upstream notice retained; collection/individual-theme caveats remain applicable |
| `ayu.json` | [licenses/ayu-mit.txt](licenses/ayu-mit.txt) | Exact referenced Zed revision; theme-specific MIT override, Ike Ku2016 |
| `hybrid.json` | [licenses/hybrid-source.txt](licenses/hybrid-source.txt) | Verbatim source header; Jellybeans includes complete MIT text, Hybrid declaresMIT and names upstream palette/syntax sources |
| `jellybeans.json` | [licenses/jellybeans-source.txt](licenses/jellybeans-source.txt) | Verbatim source header; Jellybeans includes complete MIT text, Hybrid declaresMIT and names upstream palette/syntax sources |

Aurora is covered by the retained Longbridge Apache notice. Ayu uses the MIT
notice at the exact Zed revision cited by its source URL, rather than Zed's
root AGPL license. Molokai retains both Wimer Hazenberg's theme copyright and
Tomas Restrepo's separately stated MIT source-code terms.

## Open source-scope findings

The iTerm collection license expressly reserves individual-theme copyrights
and licenses; its MIT collection notice alone does not establish those rights
for Adventure, Fahrenheit, Harper or Kibble. Alduin has no license grant found
in its reviewed README/source header. Gruvbox declares MIT/X11 in its README,
but a complete upstream notice was not located in the reviewed root/header.
Hybrid declares MIT and identifies additional palette/syntax sources; the
complete source-scope chain is not yet established. Asciinema, Solarized and
Twilight cite general websites rather than pinned theme-specific license
texts. These are release-audit findings, not findings of incompatibility.

The catalog is optional and is not loaded automatically. Its production-loader
tests establish schema/color compatibility; they do not establish permissions.
Do not treat the retained Apache snapshot notice or preserved author metadata
as a blanket clearance of unresolved upstream scope.

## Focused upstream evidence

These sources supplement the preserved catalog metadata. Matching palette values
supports the source relationship; it does not by itself settle every imported
role's provenance or grant permission for unrelated material.

- **Solarized:** [full MIT notice](licenses/solarized-mit.txt), copyright 2011
  Ethan Schoonover, from `altercation/solarized` revision
  `62f656a02f93c5190a8753159e34b385588d5ff3`. The canonical source matches six
  distinct base colors in each imported light/dark variant. Other UI roles are
  adaptations. The earlier reviewed revision had different palette values.
- **Harper:** [original source declaration](licenses/harper-source-readme.txt)
  at `idleberg/Harper-iTerm2` revision
  `5d5b3082df8e9e1060c0e9a97150878456656373` names Jan T. Sott and
  [CC-BY-SA-4.0](https://creativecommons.org/licenses/by-sa/4.0/).
  Six imported colors match, including background, foreground and four hues.
  The catalog adapts terminal colors to UI roles and adds role/alpha variants;
  preserve this change indication and the original attribution.
- **Fahrenheit:** [original source declaration](licenses/fahrenheit-source-readme.txt)
  at `fcpg/vim-fahrenheit` revision
  `fac91854c4e95ca249911761e0c5584f4fa8c658` states
  [CC-BY-SA-4.0](https://creativecommons.org/licenses/by-sa/4.0/) and explicitly
  links the iTerm port. That port at collection revision
  `4a5043e87c32158f0d7d9f6cdd03f30804c58bce` matches three imported colors,
  including its characteristic foreground. Attribution is to fcpg; no personal
  name is inferred. The imported UI role adaptation remains distinct from the
  original Vim source, whose quantized palette differs.
- **Twilight:** [complete repository permission and exceptions](licenses/twilight-source-permission.txt)
  from `textmate/themes.tmbundle` revision
  `9af146748ca3f0d3935985b6c731ccc7781831d9`. The theme names Michael Sheets;
  no accompanying exception was located. Background and primary accent match.
  This is a custom permission grant, not an MIT declaration; remaining adapted
  roles and the applicable scope remain review items.
- **Asciinema:** matched palette values appear in current player revision
  `415db52438edc88adab9d65227a6cc36b40b7885`, whose root carries Apache-2.0
  and copyright 2011–2021 Marcin Kulik. The original palette revision
  `8e7f90d1acdabe3cc36b37c988f810561e45946f` carried GPL-3.0. Current terms
  must not be described as retroactively relicensing the historical import;
  the catalog's source/license history remains an explicit review item.

The earlier open-findings inventory records the initial lookup stage. These
additional grants narrow those questions without treating the collection MIT
notice as permission for individual themes. Adventure, Kibble, Alduin, Gruvbox,
Hybrid and Molokai still have the previously described scope questions.
