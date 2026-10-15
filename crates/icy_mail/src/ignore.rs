//! Ignore rules ("twits"): messages from, to or about people or topics the reader does not want to see.

use serde::{Deserialize, Serialize};

use crate::qwk::{reply_prefix_len, MessageInfo, QwkPackage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IgnoreField {
    #[default]
    Author,
    Recipient,
    /// Matched without `Re:`/`Fwd:` prefixes, so a whole thread is caught.
    Subject,
}

impl IgnoreField {
    pub const ALL: [Self; 3] = [Self::Author, Self::Recipient, Self::Subject];
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IgnoreRule {
    pub field: IgnoreField,
    /// Case-insensitive; `*` matches any text and `?` one character. Without wildcards the whole field must match.
    pub pattern: String,
}

impl IgnoreRule {
    pub fn matches(&self, info: &MessageInfo) -> bool {
        let text = match self.field {
            IgnoreField::Author => info.from.as_str(),
            IgnoreField::Recipient => info.to.as_str(),
            IgnoreField::Subject => &info.subject[reply_prefix_len(&info.subject)..],
        };
        wildcard_match(&self.pattern, text)
    }
}

/// What becomes of an ignored message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IgnoreMode {
    /// Left out of the lists and unread counts.
    #[default]
    Hide,
    /// Listed dimmed; the reader shows a notice until the message is revealed.
    Collapse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ignored {
    Hidden,
    Collapsed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IgnoreList {
    pub mode: IgnoreMode,
    pub rules: Vec<IgnoreRule>,
}

impl IgnoreList {
    pub fn matches(&self, info: &MessageInfo) -> bool {
        self.rules.iter().any(|rule| rule.matches(info))
    }

    /// How each message of `package` is ignored. Messages addressed to `user` are never hidden,
    /// only collapsed, so mail meant for the reader cannot disappear unnoticed.
    pub fn classify(&self, package: &QwkPackage, user: &str) -> Vec<Option<Ignored>> {
        package
            .infos
            .iter()
            .map(|info| {
                if !self.matches(info) {
                    None
                } else if self.mode == IgnoreMode::Hide && !package.matches_personal(&info.to, user) {
                    Some(Ignored::Hidden)
                } else {
                    Some(Ignored::Collapsed)
                }
            })
            .collect()
    }

    /// Adds a rule unless an equal one (ignoring case) exists. Returns whether it was added.
    pub fn add(&mut self, field: IgnoreField, pattern: &str) -> bool {
        let pattern = pattern.trim();
        if pattern.is_empty()
            || self
                .rules
                .iter()
                .any(|rule| rule.field == field && rule.pattern.trim().to_lowercase() == pattern.to_lowercase())
        {
            return false;
        }
        self.rules.push(IgnoreRule {
            field,
            pattern: pattern.to_string(),
        });
        true
    }

    /// Without rules whose pattern is empty, as left by an unfinished settings row.
    pub fn without_empty(mut self) -> Self {
        self.rules.retain(|rule| !rule.pattern.trim().is_empty());
        self
    }
}

/// Case-insensitive match of the whole (trimmed) `text` against a pattern with `*` and `?`.
/// An empty pattern matches nothing.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.trim().chars().flat_map(char::to_lowercase).collect();
    if pattern.is_empty() {
        return false;
    }
    let text: Vec<char> = text.trim().chars().flat_map(char::to_lowercase).collect();
    // Greedy matching with backtracking to the last `*`.
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&ch| ch == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_match_whole_fields_ignoring_case() {
        assert!(wildcard_match("John Doe", "  john doe "));
        assert!(!wildcard_match("John", "John Doe"), "without wildcards the whole field must match");
        assert!(wildcard_match("john*", "John Doe"));
        assert!(wildcard_match("*doe", "John Doe"));
        assert!(wildcard_match("*politic*", "US Politics today"));
        assert!(wildcard_match("j?hn*", "John"));
        assert!(wildcard_match("*a*b*", "xxaYYbzz"));
        assert!(!wildcard_match("*a*b", "xxaYYbzz"));
        assert!(wildcard_match("ПРИВЕТ*", "привет мир"));
        assert!(!wildcard_match("", "anything"));
        assert!(!wildcard_match("   ", "anything"));
        assert!(wildcard_match("*", "anything"));
    }

    #[test]
    fn rules_match_their_field_and_subjects_ignore_reply_prefixes() {
        let (_dir, package) = crate::qwk::tests::load();
        let info = &package.infos[1];
        assert!(info.ref_number != 0 && info.subject.to_ascii_lowercase().starts_with("re"));
        let author = IgnoreRule {
            field: IgnoreField::Author,
            pattern: info.from.to_string(),
        };
        assert!(author.matches(info));
        let recipient = IgnoreRule {
            field: IgnoreField::Recipient,
            pattern: info.from.to_string(),
        };
        assert_eq!(recipient.matches(info), info.to.eq_ignore_ascii_case(&info.from));
        let subject = IgnoreRule {
            field: IgnoreField::Subject,
            pattern: package.infos[0].subject.to_string(),
        };
        assert!(subject.matches(info), "a reply's subject matches without its Re: prefix");
    }

    #[test]
    fn hide_mode_collapses_personal_mail_and_add_skips_duplicates() {
        let (_dir, package) = crate::qwk::tests::load();
        let mut list = IgnoreList::default();
        assert!(list.add(IgnoreField::Author, &package.infos[0].from));
        assert!(
            !list.add(IgnoreField::Author, &package.infos[0].from.to_uppercase()),
            "duplicates are not added"
        );
        assert!(!list.add(IgnoreField::Subject, "  "));
        let user = package.infos[0].to.to_string();
        let classes = list.classify(&package, &user);
        assert_eq!(classes[0], Some(Ignored::Collapsed), "mail addressed to the reader is never hidden");
        let classes = list.classify(&package, "somebody else");
        assert_eq!(classes[0], Some(Ignored::Hidden));
        list.mode = IgnoreMode::Collapse;
        assert_eq!(list.classify(&package, "somebody else")[0], Some(Ignored::Collapsed));
        for (index, info) in package.infos.iter().enumerate() {
            assert_eq!(classes[index].is_some(), list.matches(info));
        }
        list.rules.push(IgnoreRule::default());
        assert_eq!(list.clone().without_empty().rules.len(), 1);
    }
}
