#![cfg(not(target_family = "wasm"))]
// Release-tree directory roster check; packaged validation separately checks the same 13 catalogs.
#[path = "support/theme_catalog.rs"]
mod catalog;

#[test]
fn release_source_catalog_matches_current_loader() -> anyhow::Result<()> {
    let index = catalog::catalog_index()?;
    assert_eq!(index.published.len(), 13);
    assert_eq!(index.history_only.len(), 8);
    let expected = index.published;
    assert!(expected.iter().all(|name| !index.history_only.contains(name)));
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
    assert_eq!(expected.len(), 13);
    catalog::validate_catalogs(
        &[
            ("aurora", include_str!("../themes/aurora.json")),
            ("ayu", include_str!("../themes/ayu.json")),
            ("catppuccin", include_str!("../themes/catppuccin.json")),
            ("everforest", include_str!("../themes/everforest.json")),
            ("fahrenheit", include_str!("../themes/fahrenheit.json")),
            ("flexoki", include_str!("../themes/flexoki.json")),
            ("harper", include_str!("../themes/harper.json")),
            ("jellybeans", include_str!("../themes/jellybeans.json")),
            (
                "macos-classic",
                include_str!("../themes/macos-classic.json"),
            ),
            ("mellifluous", include_str!("../themes/mellifluous.json")),
            ("solarized", include_str!("../themes/solarized.json")),
            ("spaceduck", include_str!("../themes/spaceduck.json")),
            ("tokyonight", include_str!("../themes/tokyonight.json")),
        ],
        &expected,
    )
}
