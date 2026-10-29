// Source-only boundary test: requires both workspace source modules.

#[cfg(test)]
mod tests {
    /// Every name `wgpui_base::dock` exports has to be reachable from
    /// `wgpui_component::dock`, or an application cannot write its own skin
    /// without depending on the foundation crate directly.
    ///
    /// This reads both export lists rather than naming them, because the way
    /// this went wrong was checking the list against a description of base
    /// instead of against base itself: a hand-written list cannot notice a
    /// name base gained after it was written; two names were missing when
    /// this was added.
    ///
    /// The parse is deliberately crude — it takes the braces of each
    /// `pub use ...::{..}` and the tail of each single-name `pub use a::b;` —
    /// so a reformat of either file could trip it. That failure says "look at
    /// the two lists", which is the right thing to do anyway.
    fn exported_names(source: &str, prefix: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find(prefix) {
            rest = &rest[at + prefix.len()..];
            let Some(end) = rest.find(';') else { break };
            let (item, tail) = rest.split_at(end);
            rest = tail;
            let item = item.trim();
            let list = match (item.find('{'), item.rfind('}')) {
                (Some(open), Some(close)) if open < close => &item[open + 1..close],
                // `pub use a::b;` — the name is the last path segment.
                _ => item.rsplit("::").next().unwrap_or(""),
            };
            names.extend(
                list.split(',')
                    .map(|name| name.split(" as ").next().unwrap_or("").trim().to_string())
                    .filter(|name| !name.is_empty()),
            );
        }
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn every_base_dock_export_is_reachable_from_here() {
        let base = include_str!("../../wgpui-base/src/dock/mod.rs");
        let skin = include_str!("../src/dock/mod.rs");

        let exported = exported_names(base, "pub use ");
        assert!(
            exported.len() > 30,
            "the parse found only {} names in base's dock module, so it is \
             reading the wrong thing rather than reporting the truth",
            exported.len()
        );

        let reachable = exported_names(skin, "pub use wgpui_base::dock::");
        // `Panel` and `PanelView` are re-exported under other names because
        // this module's own `Panel`/`PanelView` extend them; `Dock` is a
        // documented omission. See the doc on the re-export block.
        let renamed = ["Panel", "PanelView"];
        let omitted = ["Dock"];

        let missing: Vec<&String> = exported
            .iter()
            .filter(|name| {
                !reachable.contains(name)
                    && !renamed.contains(&name.as_str())
                    && !omitted.contains(&name.as_str())
            })
            .collect();

        assert!(
            missing.is_empty(),
            "wgpui_base::dock exports these, and wgpui_component::dock does not \
             re-export them: {missing:?}. Add them to the list, or add the \
             name to `omitted` with the reason on the re-export block."
        );
    }
}
