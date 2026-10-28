use crate::{Theme, ThemeColor, ThemeConfig, ThemeMode, ThemeSet, highlighter::HighlightTheme};
#[allow(unused)]
use anyhow::Result;
use gpui::{App, Global, SharedString};
use std::{
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, LazyLock},
};

const DEFAULT_THEME: &str = include_str!("./default-theme.json");
pub(crate) static DEFAULT_THEME_COLORS: LazyLock<
    HashMap<ThemeMode, (Arc<ThemeColor>, Arc<HighlightTheme>)>,
> = LazyLock::new(|| {
    let mut colors = HashMap::new();

    let themes: Vec<ThemeConfig> = serde_json::from_str::<ThemeSet>(DEFAULT_THEME)
        .expect("Failed to parse themes/default.json")
        .themes;

    for theme in themes {
        let mut theme_color = ThemeColor::default();
        theme_color.apply_config(&theme, &ThemeColor::default());

        let highlight_theme = HighlightTheme {
            name: theme.name.to_string(),
            appearance: theme.mode,
            style: theme.highlight.unwrap_or_default(),
        };

        colors.insert(
            theme.mode,
            (Arc::new(theme_color), Arc::new(highlight_theme)),
        );
    }

    colors
});

pub(super) fn init(cx: &mut App) {
    cx.set_global(ThemeRegistry::default());
    ThemeRegistry::global_mut(cx).init_default_themes();

    // Observe changes to the theme registry to apply changes to the active theme
    cx.observe_global::<ThemeRegistry>(|cx| {
        let mode = Theme::global(cx).mode;
        let light_theme = Theme::global(cx).light_theme.name.clone();
        let dark_theme = Theme::global(cx).dark_theme.name.clone();

        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&light_theme)
            .cloned()
        {
            Theme::global_mut(cx).light_theme = theme;
        }
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&dark_theme).cloned() {
            Theme::global_mut(cx).dark_theme = theme;
        }

        let theme_name = if mode.is_dark() {
            dark_theme
        } else {
            light_theme
        };

        tracing::info!("Reload active theme: {:?}...", theme_name);
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    })
    .detach();
}

#[derive(Default, Debug)]
pub struct ThemeRegistry {
    themes_dir: PathBuf,
    default_themes: HashMap<ThemeMode, Rc<ThemeConfig>>,
    themes: HashMap<SharedString, Rc<ThemeConfig>>,
    has_custom_themes: bool,
}

impl Global for ThemeRegistry {}

impl ThemeRegistry {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Watch themes directory.
    ///
    /// And reload themes to trigger the `on_load` callback.
    #[cfg(not(target_family = "wasm"))]
    pub fn watch_dir<F>(themes_dir: PathBuf, cx: &mut App, on_load: F) -> Result<()>
    where
        F: Fn(&mut App) + 'static,
    {
        Self::_watch_themes_dir(themes_dir.clone(), cx)?;
        Self::global_mut(cx).themes_dir = themes_dir;

        // Load theme in the background.
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                Self::reload_themes(cx);
                on_load(cx);
            })
        })
        .detach_and_log_err(cx);

        Ok(())
    }

    /// Returns a reference to the map of themes (including default themes).
    pub fn themes(&self) -> &HashMap<SharedString, Rc<ThemeConfig>> {
        &self.themes
    }

    /// Returns a sorted list of themes.
    pub fn sorted_themes(&self) -> Vec<&Rc<ThemeConfig>> {
        let mut themes = self.themes.values().collect::<Vec<_>>();
        // sort by is_default true first, then light first dark later, then by name case-insensitive
        themes.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then(a.mode.cmp(&b.mode))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        themes
    }

    /// Returns a reference to the map of default themes.
    pub fn default_themes(&self) -> &HashMap<ThemeMode, Rc<ThemeConfig>> {
        &self.default_themes
    }

    pub fn default_light_theme(&self) -> &Rc<ThemeConfig> {
        &self.default_themes[&ThemeMode::Light]
    }

    pub fn default_dark_theme(&self) -> &Rc<ThemeConfig> {
        &self.default_themes[&ThemeMode::Dark]
    }

    pub fn load_themes_from_str(&mut self, content: &str) -> anyhow::Result<()> {
        let theme_set = serde_json::from_str::<ThemeSet>(content)?;
        for theme in theme_set.themes {
            if !self.themes.contains_key(&theme.name) {
                let theme_name = theme.name.clone();
                self.themes.insert(theme_name, Rc::new(theme));
                self.has_custom_themes = true;
            }
        }
        Ok(())
    }

    fn init_default_themes(&mut self) {
        let default_themes: Vec<ThemeConfig> = serde_json::from_str::<ThemeSet>(DEFAULT_THEME)
            .expect("failed to parse default theme.")
            .themes;
        for theme in default_themes.into_iter() {
            if theme.mode.is_dark() {
                self.default_themes.insert(ThemeMode::Dark, Rc::new(theme));
            } else {
                self.default_themes.insert(ThemeMode::Light, Rc::new(theme));
            }
        }
        self.themes_dir = PathBuf::from("./themes");
        self.themes = self
            .default_themes
            .values()
            .map(|theme| {
                let name = theme.name.clone();
                (name, Rc::clone(theme))
            })
            .collect();
    }

    #[cfg(not(target_family = "wasm"))]
    fn _watch_themes_dir(themes_dir: PathBuf, cx: &mut App) -> anyhow::Result<()> {
        use notify::Watcher as _;

        if !themes_dir.exists() {
            std::fs::create_dir_all(&themes_dir)?;
        }

        let (sender, receiver) = smol::channel::bounded(1);
        let mut watcher = notify::RecommendedWatcher::new(
            move |result: notify::Result<notify::Event>| match result {
                Ok(event) => {
                    let touched = event.need_rescan()
                        || matches!(
                            event.kind,
                            notify::EventKind::Any
                                | notify::EventKind::Create(_)
                                | notify::EventKind::Modify(_)
                                | notify::EventKind::Remove(_)
                        );
                    if touched {
                        match sender.try_send(()) {
                            Ok(()) | Err(smol::channel::TrySendError::Full(())) => {}
                            Err(smol::channel::TrySendError::Closed(())) => {
                                tracing::debug!("Theme watcher receiver closed")
                            }
                        }
                    }
                }
                Err(error) => tracing::error!("Failed to receive theme event: {error}"),
            },
            notify::Config::default().with_follow_symlinks(false),
        )?;
        watcher
            .watch(&themes_dir, notify::RecursiveMode::NonRecursive)
            .map_err(|error| {
                if matches!(error.kind, notify::ErrorKind::MaxFilesWatch) {
                    anyhow::Error::new(error)
                        .context("Theme file watch limit reached; theme hot reload is disabled")
                } else {
                    anyhow::Error::new(error)
                }
            })?;

        cx.spawn(async move |cx| {
            // Dropping the watcher would remove the OS registration while awaiting events.
            let _watcher = watcher;
            while receiver.recv().await.is_ok() {
                tracing::info!("Reloading themes...");
                cx.update(Self::reload_themes)?;
            }
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
        Ok(())
    }

    #[cfg(not(target_family = "wasm"))]
    fn reload_themes(cx: &mut App) {
        let registry = Self::global_mut(cx);
        match registry.reload() {
            Ok(_) => {
                tracing::info!("Themes reloaded successfully.");
            }
            Err(e) => tracing::error!("Failed to reload themes: {:?}", e),
        }
    }

    #[cfg(not(target_family = "wasm"))]
    /// Reload themes from the `themes_dir`.
    fn reload(&mut self) -> Result<()> {
        let mut themes = vec![];

        if self.themes_dir.exists() {
            for entry in std::fs::read_dir(&self.themes_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let file_content = std::fs::read_to_string(path.clone())?;

                    match serde_json::from_str::<ThemeSet>(&file_content) {
                        Ok(theme_set) => {
                            themes.extend(theme_set.themes);
                        }
                        Err(e) => {
                            tracing::error!(
                                "ignored invalid theme file: {}, {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        self.themes.clear();
        for theme in self.default_themes.values() {
            self.themes
                .insert(theme.name.clone(), Rc::new((**theme).clone()));
        }

        for theme in themes.iter() {
            if self.themes.contains_key(&theme.name) {
                continue;
            }

            if theme.is_default {
                self.default_themes
                    .insert(theme.mode, Rc::new(theme.clone()));
            }

            self.has_custom_themes = true;
            self.themes
                .insert(theme.name.clone(), Rc::new(theme.clone()));
        }

        Ok(())
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> anyhow::Result<Self> {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "wgpui-theme-test-{}-{timestamp}",
                std::process::id()
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                eprintln!("Failed to remove theme test directory: {error}");
            }
        }
    }

    #[test]
    fn retained_theme_catalog_matches_current_loader() -> anyhow::Result<()> {
        let known_colors =
            serde_json::to_value(crate::theme::schema::ThemeConfigColors::default())?;
        let mut registry = ThemeRegistry::default();
        registry.init_default_themes();
        let mut names = std::collections::HashSet::new();
        for (file, source) in [
            ("adventure", include_str!("../../themes/adventure.json")),
            ("alduin", include_str!("../../themes/alduin.json")),
            ("asciinema", include_str!("../../themes/asciinema.json")),
            ("aurora", include_str!("../../themes/aurora.json")),
            ("ayu", include_str!("../../themes/ayu.json")),
            ("catppuccin", include_str!("../../themes/catppuccin.json")),
            ("everforest", include_str!("../../themes/everforest.json")),
            ("fahrenheit", include_str!("../../themes/fahrenheit.json")),
            ("flexoki", include_str!("../../themes/flexoki.json")),
            ("gruvbox", include_str!("../../themes/gruvbox.json")),
            ("harper", include_str!("../../themes/harper.json")),
            ("hybrid", include_str!("../../themes/hybrid.json")),
            ("jellybeans", include_str!("../../themes/jellybeans.json")),
            ("kibble", include_str!("../../themes/kibble.json")),
            (
                "macos-classic",
                include_str!("../../themes/macos-classic.json"),
            ),
            ("mellifluous", include_str!("../../themes/mellifluous.json")),
            ("molokai", include_str!("../../themes/molokai.json")),
            ("solarized", include_str!("../../themes/solarized.json")),
            ("spaceduck", include_str!("../../themes/spaceduck.json")),
            ("tokyonight", include_str!("../../themes/tokyonight.json")),
            ("twilight", include_str!("../../themes/twilight.json")),
        ] {
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
                    crate::theme::color::try_parse_background(
                        value.as_str().expect("color string"),
                    )
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
                    registry.themes.contains_key(&theme.name),
                    "{file}: theme not loaded"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn watch_directory_setup_failure_reaches_caller() -> anyhow::Result<()> {
        let directory = TestDirectory::new()?;
        let blocker = directory.0.join("file");
        std::fs::write(&blocker, b"not a directory")?;
        let cx = gpui::TestAppContext::single();
        cx.update(|cx| {
            cx.set_global(ThemeRegistry::default());
            let previous = ThemeRegistry::global(cx).themes_dir.clone();
            let result = ThemeRegistry::watch_dir(blocker.join("themes"), cx, |_| {
                panic!("failed watcher setup must not run the load callback");
            });
            assert!(result.is_err());
            assert_eq!(ThemeRegistry::global(cx).themes_dir, previous);
        });
        cx.quit();
        Ok(())
    }

    #[test]
    fn theme_reload_tracks_file_create_modify_and_remove() -> anyhow::Result<()> {
        let directory = TestDirectory::new()?;
        let path = directory.0.join("theme.json");
        let mut registry = ThemeRegistry::default();
        registry.init_default_themes();
        registry.themes_dir = directory.0.clone();

        let mut set = ThemeSet {
            themes: vec![ThemeConfig {
                name: "First custom theme".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        std::fs::write(&path, serde_json::to_vec(&set)?)?;
        registry.reload()?;
        assert!(registry.themes.contains_key("First custom theme"));

        if let Some(theme) = set.themes.first_mut() {
            theme.name = "Renamed custom theme".into();
        }
        std::fs::write(&path, serde_json::to_vec(&set)?)?;
        registry.reload()?;
        assert!(!registry.themes.contains_key("First custom theme"));
        assert!(registry.themes.contains_key("Renamed custom theme"));

        std::fs::remove_file(&path)?;
        registry.reload()?;
        assert!(!registry.themes.contains_key("Renamed custom theme"));
        assert_eq!(registry.themes.len(), registry.default_themes.len());
        Ok(())
    }
}
