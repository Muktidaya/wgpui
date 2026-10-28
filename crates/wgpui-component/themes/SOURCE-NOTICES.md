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
