//! Themes: many sites run the same CMS, so a theme is a base definition and a site is a few lines
//! that extend it. `{"extends": "madara", "base_url": "https://..."}` is the theme with those
//! fields replaced. Objects are merged key by key; anything else (a list, a text) is replaced.

use crate::def::SourceDef;
use anyhow::{Context, bail};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Themes(HashMap<String, Value>);

impl Themes {
    pub fn insert(&mut self, name: &str, theme: Value) {
        self.0.insert(name.to_string(), theme);
    }

    /// A definition with its `extends` chain resolved, as a document.
    pub fn resolve_value(&self, mut definition: Value) -> anyhow::Result<Value> {
        let mut seen = Vec::new();
        while let Some(name) = definition.get("extends").and_then(Value::as_str).map(str::to_string) {
            if seen.contains(&name) {
                bail!("themes extend each other in a circle: {name}");
            }
            seen.push(name.clone());
            let mut base = self.0.get(&name).with_context(|| format!("no theme called {name}"))?.clone();
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
        Ok(serde_json::from_value(self.resolve_value(definition)?)?)
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
        themes.insert("t", json!({"theme": true, "a": {"x": 1, "y": 2}, "list": [1, 2], "keep": "k"}));
        let out = themes.resolve_value(json!({"extends": "t", "a": {"y": 3}, "list": [9]})).unwrap();
        assert_eq!(out, json!({"a": {"x": 1, "y": 3}, "list": [9], "keep": "k"}));
    }

    #[test]
    fn themes_can_extend_themes_and_circles_are_refused() {
        let mut themes = Themes::default();
        themes.insert("a", json!({"extends": "b", "one": 1}));
        themes.insert("b", json!({"two": 2, "one": 0}));
        assert_eq!(themes.resolve_value(json!({"extends": "a"})).unwrap(), json!({"one": 1, "two": 2}));
        themes.insert("c", json!({"extends": "c"}));
        assert!(themes.resolve_value(json!({"extends": "c"})).is_err());
    }
}
