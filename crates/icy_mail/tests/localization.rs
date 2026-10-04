use std::collections::{BTreeMap, BTreeSet, HashMap};

use i18n_embed::{fluent::FluentLanguageLoader, unic_langid::LanguageIdentifier};
use icy_mail::Localizations;
use regex::Regex;

fn german_loader() -> FluentLanguageLoader {
    let loader = FluentLanguageLoader::new("icy_mail", "en".parse().unwrap());
    i18n_embed::select(&loader, &Localizations, &["de-DE".parse().unwrap()]).unwrap();
    loader.set_use_isolating(false);
    loader
}

fn catalog_variables(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let messages = Regex::new(r"(?m)^([a-zA-Z][a-zA-Z0-9_-]*) =").unwrap();
    let variables = Regex::new(r"\$([a-zA-Z][a-zA-Z0-9_-]*)").unwrap();
    let entries: Vec<_> = messages.captures_iter(source).collect();
    let mut result = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let start = entry.get(0).unwrap().end();
        let end = entries.get(index + 1).map_or(source.len(), |next| next.get(0).unwrap().start());
        let args = variables.captures_iter(&source[start..end]).map(|capture| capture[1].to_owned()).collect();
        assert!(result.insert(entry[1].to_owned(), args).is_none(), "duplicate message: {}", &entry[1]);
    }
    result
}

#[test]
fn german_catalog_covers_english_messages_and_variables() {
    let english = catalog_variables(include_str!("../i18n/en/icy_mail.ftl"));
    let german = catalog_variables(include_str!("../i18n/de/icy_mail.ftl"));
    assert!(!english.is_empty());
    assert_eq!(english, german);

    let loader = german_loader();
    // Inspect German resources directly so English fallback cannot hide missing translations.
    let language: LanguageIdentifier = "de".parse().unwrap();
    let parsed = loader.with_message_iter(&language, |messages| {
        messages
            .map(|message| {
                assert!(message.value.is_some(), "missing value: {}", message.id.name);
                message.id.name.to_owned()
            })
            .collect::<BTreeSet<_>>()
    });
    let expected = german.into_keys().collect::<BTreeSet<_>>();
    let unexpected: Vec<_> = parsed.difference(&expected).collect();
    let missing: Vec<_> = expected.difference(&parsed).collect();
    assert!(unexpected.is_empty(), "unexpected messages: {unexpected:?}");
    assert!(missing.is_empty(), "unparsed messages: {missing:?}");
}

#[test]
fn german_locale_formats_labels_plurals_and_placeholders() {
    let loader = german_loader();
    assert_eq!(loader.current_languages()[0], "de".parse::<LanguageIdentifier>().unwrap());
    assert_eq!(loader.get("toolbar-open"), "Öffnen");
    assert_eq!(loader.get("settings-title"), "Einstellungen");
    assert_eq!(loader.get("settings-signature"), "Signatur");
    assert_eq!(loader.get("composer-signature"), "Signatur");
    let quote_help = loader.get("settings-quote-header-hint");
    for placeholder in ["{author}", "{subject}", "{date}"] {
        assert!(quote_help.contains(placeholder));
    }
    assert_eq!(
        loader.get_args("composer-qwk-header-limit-tooltip", HashMap::from([("limit", 255)])),
        "Dieses Paket unterstützt Kopffelder mit bis zu 255 Zeichen"
    );
    assert_eq!(loader.get("settings-extraction-cache-days"), "Entpackte Pakete aufbewahren (Tage)");
    let cache_help = loader.get("settings-extraction-cache-help");
    assert!(cache_help.contains("letzten Nutzung"));
    assert!(cache_help.contains("nächsten Öffnen"));
    assert!(cache_help.contains("0 deaktiviert"));
    assert_eq!(loader.get("folder-outbox"), "Postausgang");
    assert_eq!(loader.get("address-title-manage"), "Adressbuch");

    for (count, expected) in [(0, "0 Dateien"), (1, "1 Datei"), (2, "2 Dateien")] {
        assert_eq!(loader.get_args("status-files", HashMap::from([("count", count)])), expected);
    }
    for (key, singular, plural) in [
        ("status-outbox-drafts", "1 Entwurf im Postausgang", "2 Entwürfe im Postausgang"),
        ("notice-marked-read", "1 Nachricht als gelesen markiert", "2 Nachrichten als gelesen markiert"),
        (
            "composer-draft-saved-outbox",
            "Entwurf im Postausgang gespeichert, noch nicht versendet. Exportiere die Antworten, wenn du bereit bist.",
            "Entwurf im Postausgang gespeichert (2 Entwürfe), noch nicht versendet. Exportiere die Antworten, wenn du bereit bist.",
        ),
        ("list-draft-count", "1 Nachricht", "2 Nachrichten"),
        (
            "notice-replies-imported",
            "1 Antwort in den Postausgang importiert.",
            "2 Antworten in den Postausgang importiert.",
        ),
    ] {
        assert_eq!(loader.get_args(key, HashMap::from([("count", 1)])), singular);
        assert_eq!(loader.get_args(key, HashMap::from([("count", 2)])), plural);
    }
    assert!(loader.get("list-outbox-steps").contains(".REP-Datei"));
    assert_eq!(
        loader.get_args_concrete("list-date-weekday", HashMap::from([("weekday", 1.into()), ("time", "08:00".into())])),
        "Dienstag 08:00"
    );
    assert_eq!(
        loader.get_args("status-reading-progress", HashMap::from([("read", 2), ("total", 5), ("unread", 3)])),
        "2 von 5 gelesen · 3 ungelesen"
    );
    assert!(loader
        .get_args("dialog-mail-exported-message", HashMap::from([("path", "/tmp/OUT.rep")]))
        .contains("/tmp/OUT.rep"));
    assert_eq!(
        loader.get_args_concrete(
            "draft-issue-field-too-long",
            HashMap::from([("field", "Betreff".into()), ("length", 26.into()), ("limit", 25.into()),])
        ),
        "Betreff enthält 26 Zeichen; QWK erlaubt 25"
    );
    assert_eq!(
        loader.get_args(
            "app-forward-header",
            HashMap::from([("from", "Jörg"), ("to", "Anna"), ("date", "27.09.2026"), ("subject", "Grüße"),])
        ),
        "--- Weitergeleitete Nachricht ---\nVon: Jörg\nAn: Anna\nDatum: 27.09.2026\nBetreff: Grüße"
    );
}
