#![cfg(not(target_family = "wasm"))]
// Source-only target: intentionally excluded from Cargo archives along with eight palettes.
#[path = "support/theme_catalog.rs"]
mod catalog;

#[test]
fn complete_source_catalog_matches_current_loader() -> anyhow::Result<()> {
    let index = catalog::catalog_index()?;
    assert_eq!(index.published.len(), 13);
    assert_eq!(index.source_only.len(), 8);
    let expected = index
        .published
        .into_iter()
        .chain(index.source_only)
        .collect::<Vec<_>>();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("themes");
    let mut files = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && path.file_stem().is_some_and(|stem| stem != "catalog-index")
        {
            files.insert(
                path.file_stem()
                    .expect("JSON stem")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    assert_eq!(
        files,
        expected.iter().cloned().collect(),
        "source catalog is incomplete or unindexed"
    );
    assert_eq!(expected.len(), 21);
    catalog::validate_catalogs(
        &[
            ("adventure", include_str!("../themes/adventure.json")),
            ("alduin", include_str!("../themes/alduin.json")),
            ("asciinema", include_str!("../themes/asciinema.json")),
            ("aurora", include_str!("../themes/aurora.json")),
            ("ayu", include_str!("../themes/ayu.json")),
            ("catppuccin", include_str!("../themes/catppuccin.json")),
            ("everforest", include_str!("../themes/everforest.json")),
            ("fahrenheit", include_str!("../themes/fahrenheit.json")),
            ("flexoki", include_str!("../themes/flexoki.json")),
            ("gruvbox", include_str!("../themes/gruvbox.json")),
            ("harper", include_str!("../themes/harper.json")),
            ("hybrid", include_str!("../themes/hybrid.json")),
            ("jellybeans", include_str!("../themes/jellybeans.json")),
            ("kibble", include_str!("../themes/kibble.json")),
            (
                "macos-classic",
                include_str!("../themes/macos-classic.json"),
            ),
            ("mellifluous", include_str!("../themes/mellifluous.json")),
            ("molokai", include_str!("../themes/molokai.json")),
            ("solarized", include_str!("../themes/solarized.json")),
            ("spaceduck", include_str!("../themes/spaceduck.json")),
            ("tokyonight", include_str!("../themes/tokyonight.json")),
            ("twilight", include_str!("../themes/twilight.json")),
        ],
        &expected,
    )
}
