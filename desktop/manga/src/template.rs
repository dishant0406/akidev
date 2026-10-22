//! Filling in the holes of a text: `{name}` holes and `{? ... ?}` groups that vanish when a hole
//! inside is empty. A hole named `name:raw` is not percent-encoded.
//!
//! The same grammar as the streaming definitions in `akidev-core`, kept separate here because that
//! filler is tied to the streaming `Kind`.

/// Fills `template`. `None` when a hole outside any group has nothing.
pub fn fill(template: &str, hole: &dyn Fn(&str) -> Option<String>, encode: bool) -> Option<String> {
    let mut text = String::new();
    // The group being read, and whether every hole in it had a value.
    let mut group: Option<(String, bool)> = None;
    let mut rest = template;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("{?") {
            group = Some((String::new(), true));
            rest = after;
        } else if let Some(after) = rest.strip_prefix("?}") {
            if let Some((inside, true)) = group.take() {
                text.push_str(&inside);
            }
            rest = after;
        } else if let Some(after) = rest.strip_prefix('{') {
            let close = after.find('}')?;
            let name = &after[..close];
            rest = &after[close + 1..];
            let (name, raw) = match name.strip_suffix(":raw") {
                Some(name) => (name, true),
                None => (name, false),
            };
            let value = hole(name).filter(|value| !value.is_empty()).map(|value| {
                if encode && !raw {
                    percent_encode(&value)
                } else {
                    value
                }
            });
            match (&mut group, value) {
                (Some((inside, _)), Some(value)) => inside.push_str(&value),
                (Some((_, complete)), None) => *complete = false,
                (None, Some(value)) => text.push_str(&value),
                (None, None) => return None,
            }
        } else {
            let width = rest.chars().next().map_or(1, char::len_utf8);
            let (literal, next) = rest.split_at(width);
            match &mut group {
                Some((inside, _)) => inside.push_str(literal),
                None => text.push_str(literal),
            }
            rest = next;
        }
    }
    Some(text)
}

pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holes(name: &str) -> Option<String> {
        match name {
            "term" => Some("solo leveling".into()),
            "id" => Some("/manga/x/".into()),
            _ => None,
        }
    }

    #[test]
    fn encodes_holes_and_keeps_raw_ones() {
        assert_eq!(
            fill("/s?q={term}", &holes, true).unwrap(),
            "/s?q=solo%20leveling"
        );
        assert_eq!(fill("{id:raw}ajax", &holes, true).unwrap(), "/manga/x/ajax");
    }

    #[test]
    fn groups_vanish_when_a_hole_is_empty() {
        assert_eq!(fill("a{?&g={genre}?}b", &holes, true).unwrap(), "ab");
        assert_eq!(
            fill("a{?&q={term}?}b", &holes, true).unwrap(),
            "a&q=solo%20levelingb"
        );
    }

    #[test]
    fn an_empty_hole_outside_a_group_empties_the_value() {
        assert!(fill("{genre}", &holes, true).is_none());
    }
}
