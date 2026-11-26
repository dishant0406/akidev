//! Themes: many sites run the same CMS, so a theme is a base definition and a site is a few lines
//! that extend it. `{"extends": "madara", "base_url": "https://..."}` is the theme with those
//! fields replaced. Objects are merged key by key; anything else (a list, a text) is replaced.

use crate::def::SourceDef;
use anyhow::{Context, bail};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default, Clone)]
pub struct Themes(HashMap<String, Value>);

impl Themes {
    pub fn insert(&mut self, name: &str, theme: Value) {
        self.0.insert(name.to_string(), theme);
    }

    /// A definition with its `extends` chain merged in, as a document that stands alone: what is stored when
    /// a source is installed by hand, and what `resolve` reads. The definition's own fields win, so its id and
    /// version stay its own. Loading a document with no themes at all must still work.
    pub fn resolve_document(&self, mut definition: Value) -> anyhow::Result<Value> {
        let mut seen = Vec::new();
        while let Some(name) = definition
            .get("extends")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            if seen.contains(&name) {
                bail!("themes extend each other in a circle: {name}");
            }
            seen.push(name.clone());
            let mut base = self.0.get(&name).with_context(|| format!("the definition extends a theme called {name}, which is not installed: add the repository that has it, then paste this again (a source does not bring its own theme)"))?.clone();
            if let Some(object) = definition.as_object_mut() {
                object.remove("extends");
            }
            // The theme may extend another; the site's own fields win over both.
            merge(&mut base, definition);
            definition = base;
        }
        if let Some(object) = definition.as_object_mut() {
            object.remove("theme");
        }
        Ok(definition)
    }

    pub fn resolve(&self, definition: Value) -> anyhow::Result<SourceDef> {
        Ok(serde_json::from_value(self.resolve_document(definition)?)?)
    }
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(base), Value::Object(over)) => {
            for (key, value) in over {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (slot, over) => *slot = over,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_site_overrides_fields_and_keeps_the_rest_of_the_theme() {
        let mut themes = Themes::default();
        themes.insert(
            "t",
            json!({"theme": true, "a": {"x": 1, "y": 2}, "list": [1, 2], "keep": "k"}),
        );
        let out = themes
            .resolve_document(json!({"extends": "t", "a": {"y": 3}, "list": [9]}))
            .unwrap();
        assert_eq!(
            out,
            json!({"a": {"x": 1, "y": 3}, "list": [9], "keep": "k"})
        );
    }

    #[test]
    fn themes_can_extend_themes_and_circles_are_refused() {
        let mut themes = Themes::default();
        themes.insert("a", json!({"extends": "b", "one": 1}));
        themes.insert("b", json!({"two": 2, "one": 0}));
        assert_eq!(
            themes.resolve_document(json!({"extends": "a"})).unwrap(),
            json!({"one": 1, "two": 2})
        );
        themes.insert("c", json!({"extends": "c"}));
        assert!(themes.resolve_document(json!({"extends": "c"})).is_err());
    }

    #[test]
    fn a_document_that_extends_a_theme_keeps_nothing_of_the_theme_afterwards() {
        let mut themes = Themes::default();
        themes.insert(
            "t",
            json!({
                "theme": true, "version": 9, "base_url": "https://t",
                "detail": {"request": {"url": "{base}/manga/{id}"}, "title": "h1"},
                "chapters": {"request": {"url": "{base}/c"}, "rows": "li", "chapter": {"id": "a@href"}},
                "pages": {"request": {"url": "{base}/p/{id}"}, "images": "img@src"}
            }),
        );
        let document = themes
            .resolve_document(json!({
                "extends": "t", "schema": 1, "id": "site", "name": "Site", "version": 1, "base_url": "https://site",
                "chapters": {"request": {"url": "{base}/chapters/{id}"}}
            }))
            .unwrap();
        // Its own id, name, version and request win over the theme's; the rest of the theme is merged in;
        // and neither `extends` nor `theme` is left behind.
        assert_eq!(document["id"], json!("site"));
        assert_eq!(document["version"], json!(1));
        assert_eq!(
            document["chapters"]["request"]["url"],
            json!("{base}/chapters/{id}")
        );
        assert_eq!(document["chapters"]["rows"], json!("li"));
        assert_eq!(document["detail"]["title"], json!("h1"));
        assert!(document.get("extends").is_none() && document.get("theme").is_none());
        // Loading it again with no themes at all is what an installed source does.
        Themes::default().resolve(document).unwrap();
    }

    #[test]
    fn extending_a_theme_that_is_not_installed_says_what_to_do_about_it() {
        let error = Themes::default()
            .resolve_document(json!({"extends": "madara", "schema": 1}))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("madara") && error.contains("not installed"),
            "{error}"
        );
    }
}
