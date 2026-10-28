#![cfg(not(target_family = "wasm"))]
#[path = "support/theme_catalog.rs"]
mod catalog;

#[test]
fn published_catalogs_match_current_loader() -> anyhow::Result<()> {
    let index = catalog::catalog_index()?;
    assert_eq!(index.published.len(), 13);
    assert_eq!(index.source_only.len(), 8);
    assert!(
        index
            .published
            .iter()
            .all(|name| !index.source_only.contains(name))
    );
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
        &index.published,
    )
}
