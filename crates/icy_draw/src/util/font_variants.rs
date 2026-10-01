//! Groups text art fonts that are variants of one font, such as the color sets of a TDF
//! font named `Acidscape1C`, `Acidscape1G` and `Acidscape1R` or `Agony 2 Red` and `Agony 2 Purp`.

use std::collections::HashMap;
use std::hash::Hash;

/// Shortest name part left after removing the variant suffix.
const MIN_STEM_CHARS: usize = 3;

/// Colors that name the variants of a font. A trailing word matches when it starts one of
/// them, since TDF font names are cut off after 12 characters (`Purp`, `Silve`, `Margen`).
const COLOR_WORDS: [&str; 24] = [
    "red", "green", "grn", "blue", "blu", "cyan", "cyn", "purple", "ppl", "magenta", "margenta", "violet", "pink", "silver", "slv", "white", "gray", "grey",
    "gold", "yellow", "orange", "org", "brown", "black",
];

fn is_color_word(word: &str) -> bool {
    let word = word.to_lowercase();
    !word.is_empty() && word.chars().all(char::is_alphabetic) && COLOR_WORDS.iter().any(|color| color.starts_with(&word))
}

/// Splits a trailing color word off `name`: `Nest Red` is `Nest` in red.
fn split_color_word(name: &str) -> (&str, Option<&str>) {
    let name = name.trim();
    match name.rsplit_once(char::is_whitespace) {
        Some((head, last)) if is_color_word(last) && head.trim_end().chars().count() >= MIN_STEM_CHARS => (head.trim_end(), Some(last)),
        _ => (name, None),
    }
}

/// Splits a trailing letter off `name`: `Acidscape1C` is `Acidscape1` in color `C`.
fn split_letter(name: &str) -> (&str, Option<&str>) {
    match name.char_indices().next_back() {
        Some((split, last)) if last.is_alphabetic() && name[..split].chars().count() >= MIN_STEM_CHARS => (&name[..split], Some(&name[split..])),
        _ => (name, None),
    }
}

/// The name without its variant suffix, a trailing color word (`Nest Red`) and then a trailing
/// letter (`Acidscape1C`): names with the same stem are variants of each other. Case is ignored.
#[must_use]
pub fn variant_stem(name: &str) -> String {
    split_letter(split_color_word(name).0).0.to_lowercase()
}

/// Groups `fonts` in order of their first member. A font joins a group when its name has the
/// same [`variant_stem`] and its `key` (e.g. kind and size) matches; everything else stays alone.
#[must_use]
pub fn variant_groups<'a, K: Eq + Hash>(fonts: impl IntoIterator<Item = (usize, &'a str, K)>) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut by_key: HashMap<(String, K), usize> = HashMap::new();
    for (index, name, key) in fonts {
        let group = *by_key.entry((variant_stem(name), key)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(index);
    }
    groups
}

/// Short labels that tell the variants of a group apart: their color words (`Red`, `Purp`) when
/// any variant has one, otherwise their last letters (`C`, `G`). A name without either is `•`,
/// repeated labels are numbered and names that all end alike are numbered instead.
#[must_use]
pub fn variant_labels<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let names: Vec<&str> = names.into_iter().map(str::trim).collect();
    let words: Vec<Option<&str>> = names.iter().map(|name| split_color_word(name).1).collect();
    let suffixes: Vec<String> = if words.iter().any(Option::is_some) {
        words.iter().map(|word| word.unwrap_or("•").to_owned()).collect()
    } else {
        names
            .iter()
            .map(|name| split_letter(name).1.map_or_else(|| "•".to_owned(), str::to_uppercase))
            .collect()
    };
    if suffixes.iter().all(|suffix| suffix.eq_ignore_ascii_case(&suffixes[0])) {
        return (1..=suffixes.len()).map(|number| number.to_string()).collect();
    }
    suffixes
        .iter()
        .enumerate()
        .map(|(index, suffix)| {
            let repeat = suffixes[..index].iter().filter(|other| other.eq_ignore_ascii_case(suffix)).count();
            if repeat == 0 {
                suffix.clone()
            } else {
                format!("{suffix} {}", repeat + 1)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(names: &[(&str, u8)]) -> Vec<Vec<usize>> {
        variant_groups(names.iter().enumerate().map(|(index, (name, key))| (index, *name, *key)))
    }

    #[test]
    fn color_variants_share_a_group() {
        let names = [
            ("Acidscape1C", 0),
            ("Blade", 0),
            ("Acidscape1G", 0),
            ("acidscape1p", 0),
            ("Acidscape1", 0),
            ("Acidscape1R", 0),
        ];
        assert_eq!(groups(&names), vec![vec![0, 2, 3, 4, 5], vec![1]]);
    }

    #[test]
    fn different_kinds_or_short_names_stay_apart() {
        assert_eq!(groups(&[("FontA", 0), ("FontB", 1)]), vec![vec![0], vec![1]]);
        assert_eq!(groups(&[("Abc", 0), ("Abd", 0)]), vec![vec![0], vec![1]]);
        assert_eq!(groups(&[("Font1", 0), ("Font2", 0)]), vec![vec![0], vec![1]]);
        assert_eq!(groups(&[("Metal", 0), ("Metals", 0)]), vec![vec![0], vec![1]]);
    }

    #[test]
    fn trailing_color_words_name_variants_even_when_cut_off() {
        let names = [
            ("Agony 2 Red", 0),
            ("Agony 2 Purp", 0),
            ("Agony 2", 0),
            ("Agony 2 Silve", 0),
            ("Agony 3 Red", 0),
            ("Red", 0),
        ];
        assert_eq!(groups(&names), vec![vec![0, 1, 2, 3], vec![4], vec![5]]);
        assert_eq!(
            groups(&[("ACID 3D", 0), ("ACID 3D Blue", 0), ("Acid", 0), ("Acid Silver", 0)]),
            vec![vec![0, 1], vec![2, 3]]
        );
        assert_eq!(split_color_word("Nest  Margen "), ("Nest", Some("Margen")));
        assert_eq!(split_color_word("Font 34"), ("Font 34", None));
    }

    #[test]
    fn labels_name_the_differing_suffix_or_the_position() {
        assert_eq!(variant_labels(["Acidscape1C", "Acidscape1g", "Acidscape1R"]), ["C", "G", "R"]);
        assert_eq!(variant_labels(["Agony 2 Red", "Agony 2 Purp"]), ["Red", "Purp"]);
        assert_eq!(variant_labels(["Nest Red", "Nest red", "Nest Blue"]), ["Red", "red 2", "Blue"]);
        assert_eq!(variant_labels(["Chaos 2", "Chaos 2 Blue"]), ["•", "Blue"]);
        assert_eq!(variant_labels(["ACID 3D", "ACID 3D Blue"]), ["•", "Blue"]);
        assert_eq!(variant_labels(["Project 13", "Project 13 C"]), ["•", "C"]);
        assert_eq!(variant_labels(["Acidscape1", "Acidscape1C"]), ["•", "C"]);
        assert_eq!(variant_labels(["Blocky", "Blocky"]), ["1", "2"]);
    }
}
