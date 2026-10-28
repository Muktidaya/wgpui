use std::collections::BTreeSet;
use wgpui_component::theme::{ThemeConfigColors, ThemeRegistry, ThemeSet, try_parse_background};

#[derive(serde::Deserialize)]
pub struct CatalogIndex {
    pub published: Vec<String>,
    pub source_only: Vec<String>,
}

pub fn catalog_index() -> anyhow::Result<CatalogIndex> {
    Ok(serde_json::from_str(include_str!(
        "../../themes/catalog-index.json"
    ))?)
}

pub fn validate_catalogs(catalogs: &[(&str, &str)], expected: &[String]) -> anyhow::Result<()> {
    let actual = catalogs
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "catalog roster differs from distribution index"
    );
    assert_eq!(actual.len(), catalogs.len(), "duplicate catalog input");
    let known_colors = serde_json::to_value(ThemeConfigColors::default())?;
    let mut registry = ThemeRegistry::default();
    let mut names = BTreeSet::new();
    for (file, source) in catalogs {
        let raw: serde_json::Value = serde_json::from_str(source)?;
        let set: ThemeSet = serde_json::from_str(source)?;
        assert!(!set.name.is_empty(), "{file}: missing catalog name");
        assert!(
            set.author.is_some() && set.url.is_some(),
            "{file}: missing provenance"
        );
        assert!(!set.themes.is_empty(), "{file}: empty catalog");
        for theme in raw["themes"].as_array().expect("theme array") {
            for (key, value) in theme["colors"].as_object().expect("color map") {
                assert!(
                    known_colors.get(key).is_some(),
                    "{file}: unsupported role {key}"
                );
                try_parse_background(value.as_str().expect("color string"))
                    .map_err(|error| anyhow::anyhow!("{file}: invalid {key}: {error}"))?;
            }
        }
        for theme in &set.themes {
            assert!(
                names.insert(theme.name.clone()),
                "{file}: duplicate theme name"
            );
        }
        registry.load_themes_from_str(source)?;
        for theme in &set.themes {
            assert!(
                registry.themes().contains_key(&theme.name),
                "{file}: theme not loaded"
            );
        }
    }
    Ok(())
}
