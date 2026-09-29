# Icy Mail Benutzeroberfläche (Deutsch)

dialog-close_button = Schließen

# Toolbar
toolbar-open = Öffnen
toolbar-open-tooltip = Nachrichtenpaket öffnen (Ctrl+O)
toolbar-new = Neu
toolbar-new-tooltip = Neue Nachricht schreiben (Ctrl+N)
toolbar-export = Antworten exportieren
toolbar-export-count = Antworten exportieren ({ $count })
toolbar-export-tooltip = Antwortpaket zum Hochladen in die BBS speichern (Ctrl+Shift+E)
toolbar-next-unread = Nächste ungelesene
toolbar-next-unread-tooltip = Nächste ungelesene Nachricht öffnen (N)
toolbar-menu = Menü
toolbar-threads-tooltip = Nachrichten nach Diskussionsfäden gruppieren (Ctrl+T)
toolbar-list-tooltip = Nachrichten als Liste anzeigen (Ctrl+T)
toolbar-search-hint = Nachrichten durchsuchen
toolbar-search-hint-fields = Suchen in: { $fields }
search-fields-tooltip = Auswählen, was durchsucht wird
search-fields-title = Suchen in
search-field-from = Von
search-field-to = An
search-field-subject = Betreff
search-field-text = Nachrichtentext
search-fields-all = Überall suchen
search-bodies-running = Nachrichtentexte werden durchsucht…
search-message-error = Nachricht { $number } konnte nicht durchsucht werden: { $error }
toolbar-search-clear = Suche löschen (Esc)

# Main menu
menu-file = Datei
menu-message = Nachricht
menu-tools = Werkzeuge
menu-help = Hilfe
menu-open-packet = Paket öffnen…
menu-open-recent = Zuletzt geöffnet
menu-reload-packet = Paket neu laden
menu-packet-information = Paketinformationen
menu-new-message = Neue Nachricht
menu-reply = Antworten
menu-forward = Weiterleiten
menu-export-replies = Antworten exportieren…
menu-next-unread = Nächste ungelesene Nachricht
menu-mark-read = Als gelesen markieren
menu-star = Markieren
menu-unstar = Markierung entfernen
menu-mark-unread = Als ungelesen markieren
menu-mark-folder-read = Ordner als gelesen markieren
menu-address-book = Adressbuch…
menu-taglines = Taglines…
menu-add-author = Absender zum Adressbuch hinzufügen
menu-save-message = Nachricht speichern…
menu-save-message-utf8 = Nachricht als UTF-8 speichern…
menu-save-tagline = Tagline der Nachricht speichern
menu-view = Ansicht
menu-view-list = Liste
menu-view-threads = Diskussionsfäden
menu-unread-only = Nur ungelesene Nachrichten
menu-reading-pane = Lesebereich
menu-message-zoom = Nachrichtenzoom
menu-appearance = Erscheinungsbild
menu-settings = Einstellungen…
menu-keyboard-shortcuts = Tastenkürzel
menu-about = Über Icy Mail
menu-new-window = Neues Fenster
menu-close-window = Fenster schließen

# Status bar
status-reading-progress = { $read } von { $total } gelesen · { $unread } ungelesen
status-reading-progress-tooltip = Gelesene Nachrichten in diesem Paket
status-no-packet = Kein Paket geöffnet
status-update-available = Update verfügbar: { $version }
status-message-zoom = Nachrichtenzoom
status-outbox-drafts = { $count ->
    [one] 1 Entwurf im Postausgang
   *[other] { $count } Entwürfe im Postausgang
}
status-show-outbox = Postausgang anzeigen
status-files = { $count ->
    [one] 1 Datei
   *[other] { $count } Dateien
}

# Sidebar
sidebar-offline-mail = Offline-Nachrichten
sidebar-packet-information = Paketinformationen
sidebar-mailboxes = Postfächer
sidebar-conferences = Konferenzen
sidebar-sort-conferences = Konferenzen sortieren
sidebar-sort-number = Nach Nummer
sidebar-sort-name = Nach Name
sidebar-sort-count = Nach Nachrichtenanzahl
sidebar-unread-conferences-only = Nur Konferenzen mit ungelesenen Nachrichten zeigen
sidebar-network-tooltip = { $network }: { $conferences } Konferenzen, { $unread } ungelesen
sidebar-conferences-all-read = Alle Konferenzen sind gelesen
sidebar-conference-tooltip = Konferenz { $number }
    { $count } Nachrichten, { $unread } ungelesen
folder-all = Alle Nachrichten
folder-all-tooltip = { $count } Nachrichten, { $unread } ungelesen
folder-personal = Persönlich
folder-personal-tooltip = Nachrichten an { $user }
folder-starred = Markiert
folder-starred-tooltip = Nachrichten, die du zum späteren Lesen oder Beantworten markiert hast (S)
folder-outbox = Postausgang
folder-outbox-tooltip = Antworten und neue Nachrichten, die auf den Export warten
folder-bulletins = Bulletins
folder-bulletins-tooltip = Begrüßungs- und News-Bildschirme, Bulletins und Listen neuer Dateien der BBS

# File dialogs
loading-open-title = Nachrichtenpaket öffnen
loading-filter-packages = Nachrichtenpakete
loading-filter-all = Alle Dateien
loading-save-message-title = Nachricht speichern
loading-filter-message = Nachrichtentext
loading-export-title = Antwortpaket exportieren
loading-filter-reply = QWK-Antwortpaket

# Start page
welcome-opening = { $name } wird geöffnet…
welcome-tagline = BBS-Nachrichten offline lesen und beantworten.
welcome-open-packet = Paket öffnen…
welcome-open-packet-tooltip = QWK-Paket öffnen (Ctrl+O)
welcome-drop-title = QWK-Paket hierher ziehen
welcome-drop-release = Loslassen, um das Paket zu öffnen
welcome-recent-unread = { $count } ungelesen
welcome-recent-all-read = Alles gelesen
welcome-recent-messages = { $count ->
    [one] 1 Nachricht
   *[other] { $count } Nachrichten
}
welcome-recent-starred = { $count } markiert
welcome-recent-drafts = { $count ->
    [one] 1 Entwurf zu exportieren
   *[other] { $count } Entwürfe zu exportieren
}
welcome-recent-created = Gepackt { $date }
welcome-recent-missing = { $file } wurde verschoben oder gelöscht
welcome-recent-packets = Zuletzt geöffnete Pakete
welcome-forget-recent = Aus der Liste entfernen

# Command line
cli-about = Ein Offline-QWK-Nachrichtenleser zum Lesen und Erstellen von Antwortpaketen
cli-debug-help = Debug-Protokollierung einschließlich unverarbeiteter ANSI-QWK-Kopfzeilen aktivieren
cli-file-help = Zu öffnendes Nachrichtenpaket (QWK in einem gängigen Archivformat)

# Application: notices, errors and generated text
folder-conference = Konferenz { $number }
app-tab-folders = Ordner
app-tab-messages = Nachrichten
app-tab-message = Nachricht
app-no-subject = (kein Betreff)
app-save-message-failed = { $path } konnte nicht gespeichert werden: { $error }
app-export-failed = { $path } konnte nicht exportiert werden:
    { $error }
app-drafts-failed = Die Entwürfe für { $path } konnten nicht geladen werden:
    { $error }
app-no-conference = Dieses Paket enthält keine Konferenz zum Schreiben einer Nachricht.
app-new-message-failed = Neue Nachricht konnte nicht begonnen werden: { $error }
app-message-unavailable = Die ausgewählte Nachricht ist nicht mehr verfügbar.
app-original-failed = Die ursprüngliche Nachricht konnte nicht gelesen werden: { $error }
app-reply-failed = Antwort konnte nicht begonnen werden: { $error }
app-draft-unavailable = Der Entwurf ist nicht mehr verfügbar.
app-delete-draft-failed = Entwurf konnte nicht gelöscht werden: { $error }
app-taglines-read-failed = Taglines konnten nicht gelesen werden:
    { $error }
app-taglines-save-failed = Taglines konnten nicht gespeichert werden:
    { $error }
# Inserted into the message text; QWK can only transport CP437 characters.
app-quote-attribution = Am { $date } schrieb { $name }:
app-forward-header = --- Weitergeleitete Nachricht ---
    Von: { $from }
    An: { $to }
    Datum: { $date }
    Betreff: { $subject }
notice-finish-writing = Zuerst die aktuelle Nachricht speichern oder verwerfen
notice-read-marks-unavailable = Lesemarkierungen sind nicht verfügbar: { $error }
notice-message-saved = Nachricht gespeichert unter { $path }
notice-read-marks-failed = Lesemarkierungen konnten nicht gespeichert werden: { $error }
notice-stars-failed = Markierung konnte nicht gespeichert werden: { $error }
notice-marked-read = { $count ->
    [one] 1 Nachricht als gelesen markiert
   *[other] { $count } Nachrichten als gelesen markiert
}
notice-continuing-in = Weiter in { $name }
notice-no-more-unread = Keine weiteren ungelesenen Nachrichten
notice-no-more-unread-filtered = Keine weiteren ungelesenen Nachrichten in dieser Suche. Lösche die Suche, um die übrigen zu sehen.
notice-draft-deleted = Entwurf gelöscht
notice-nothing-to-export = Keine Antworten zum Exportieren vorhanden
notice-no-tagline = Diese Nachricht enthält keine Tagline
notice-tagline-known = Diese Tagline ist bereits in der Liste
notice-tagline-saved = Tagline gespeichert: ... { $tagline }
notice-message-copied = Nachricht in die Zwischenablage kopiert
notice-text-copied = Text in die Zwischenablage kopiert

# Settings
settings-title = Einstellungen
settings-page-general = Allgemein
settings-page-monitor = Monitor
settings-section-appearance = Erscheinungsbild
settings-theme-label = Design
settings-theme-follow-system = Systemeinstellung
settings-theme-light = Hell
settings-theme-dark = Dunkel
settings-section-messages = Nachrichten
settings-reading-mode-label = Nachrichtendarstellung
settings-reading-mode-classic = Klassisch (BBS-Terminal)
settings-reading-mode-modern = Modern (Text)
settings-modern-font-label = Schrift
settings-modern-font-proportional = Proportional
settings-modern-font-monospace = Feste Breite
settings-modern-font-size-label = Schriftgröße
settings-zoom-label = Zoom
settings-reading-pane-label = Lesebereich
settings-reading-pane-automatic = Automatisch
settings-reading-pane-below = Unter der Liste
settings-reading-pane-right = Rechts neben der Liste
settings-zoom-fit-width = An Breite anpassen
settings-zoom-fit = Einpassen
settings-section-writing = Schreiben
settings-add-random-tagline = Neue Nachrichten mit einer zufälligen Tagline versehen

# Mail dialogs
dialog-mail-app-title = Icy Mail
dialog-mail-delete-draft-title = Diesen Entwurf löschen?
dialog-mail-delete-draft-message = „{ $title }“ wird aus dem Postausgang entfernt. Dies kann nicht rückgängig gemacht werden.
dialog-mail-delete-draft-keep = Behalten
dialog-mail-delete-draft-delete = Löschen
dialog-mail-discard-title = Diese Nachricht verwerfen?
dialog-mail-discard-quit-message = Die aktuelle Nachricht wurde nicht gespeichert. Fenster trotzdem schließen?
dialog-mail-discard-message = Die Änderungen an dieser Nachricht gehen verloren.
dialog-mail-discard-keep-editing = Weiter bearbeiten
dialog-mail-discard-discard = Verwerfen
dialog-mail-export-problems-title = Einige Nachrichten müssen korrigiert werden
dialog-mail-export-problems-message =
    Diese Nachrichten im Postausgang müssen vor dem Export des Antwortpakets korrigiert werden:

    { $problems }
dialog-mail-show-outbox = Postausgang anzeigen
dialog-mail-exported-title = Antwortpaket bereit
dialog-mail-exported-message = Das Antwortpaket wurde hier gespeichert:

    { $path }

    Es wurde noch nicht versendet. Lade diese .REP-Datei in deine BBS hoch, um die Antworten zuzustellen. Die Entwürfe bleiben im Postausgang.
dialog-mail-open-folder = Ordner öffnen
dialog-mail-open-folder-error = { $path } konnte nicht geöffnet werden:
    { $error }
dialog-mail-packet-info-subtitle = Paketinformationen
dialog-mail-packet-board = BBS
dialog-mail-packet-location = Standort
dialog-mail-packet-phone = Telefon
dialog-mail-packet-sysop = Sysop
dialog-mail-packet-bbs-id = BBS-ID
dialog-mail-packet-packet = Paket
dialog-mail-packet-user = Benutzer
dialog-mail-packet-created = Erstellt
dialog-mail-packet-messages = Nachrichten
dialog-mail-packet-unread = Ungelesen
dialog-mail-packet-conferences = Konferenzen
dialog-mail-packet-outbox = Postausgang
dialog-mail-packet-file = Datei

# Keyboard shortcuts
shortcuts-title = Tastenkürzel
shortcuts-subtitle = Schnellreferenz für Icy Mail
shortcuts-section-reading = Lesen
shortcuts-reading-previous-next-entry = Vorheriger oder nächster Eintrag
shortcuts-reading-collapse-expand-thread = Diskussionsfaden ein- oder ausklappen
shortcuts-reading-next-pane = Nächster Bereich
shortcuts-reading-open-selected-folder-message = Ausgewählten Ordner oder ausgewählte Nachricht öffnen
shortcuts-reading-page-down-next-unread = Seite abwärts, dann nächste ungelesene Nachricht
shortcuts-reading-next-unread = Nächste ungelesene Nachricht
shortcuts-reading-mark-read-unread = Als gelesen oder ungelesen markieren
shortcuts-reading-star = Nachricht markieren oder Markierung entfernen
shortcuts-reading-mark-folder-read = Ordner als gelesen markieren
shortcuts-reading-search-messages = Nachrichten durchsuchen
shortcuts-reading-switch-list-threads = Zwischen Liste und Diskussionsfäden wechseln
shortcuts-reading-save-message-tagline = Tagline der Nachricht speichern
shortcuts-reading-address-book = Adressbuch
shortcuts-reading-add-author-address-book = Absender zum Adressbuch hinzufügen
shortcuts-section-writing = Schreiben
shortcuts-writing-new-message = Neue Nachricht
shortcuts-writing-reply = Antworten
shortcuts-writing-forward = Weiterleiten
shortcuts-writing-save-draft = Entwurf speichern
shortcuts-writing-pick-recipient-address-book = Empfänger aus dem Adressbuch auswählen
shortcuts-writing-choose-tagline = Tagline beim Schreiben auswählen
shortcuts-writing-edit-tagline-list = Tagline-Liste bearbeiten
shortcuts-writing-cancel-writing = Schreiben abbrechen
shortcuts-writing-delete-selected-draft = Ausgewählten Entwurf löschen
shortcuts-writing-export-reply-packet = Antwortpaket exportieren
shortcuts-section-packets = Pakete
shortcuts-packets-open-packet = Paket öffnen
shortcuts-packets-reload-packet = Paket neu laden
shortcuts-section-windows = Fenster
shortcuts-windows-new-window = Neues Fenster
shortcuts-windows-close-window = Fenster schließen
shortcuts-windows-settings = Einstellungen
shortcuts-windows-keyboard-shortcuts = Tastenkürzel
shortcuts-section-message-editor = Nachrichteneditor
shortcuts-editor-text-color = Textfarbe ändern, auch für die Auswahl
shortcuts-editor-character-table = Zeichentabelle, Auswahl per Tastatur
shortcuts-editor-quote-panel = Zitatbereich
shortcuts-editor-find-text = Text suchen
shortcuts-editor-find-next = Weitersuchen
shortcuts-editor-delete-line = Zeile löschen
shortcuts-editor-undo = Rückgängig
shortcuts-editor-redo = Wiederholen
shortcuts-editor-select-all-quote-rest = Alles auswählen oder Rest zitieren
shortcuts-editor-insert-overwrite = Einfügen oder überschreiben
shortcuts-editor-close-panel-selection = Bereich schließen oder Auswahl aufheben

# Composer
composer-title-edit-draft = Entwurf bearbeiten
composer-title-new-message = Neue Nachricht
composer-title-reply = Antworten
composer-title-forward-message = Nachricht weiterleiten
composer-draft-saved-needs-changes = Entwurf gespeichert - vor dem Export sind noch Änderungen nötig
composer-draft-saved-outbox = { $count ->
    [one] Entwurf im Postausgang gespeichert, noch nicht versendet. Exportiere die Antworten, wenn du bereit bist.
   *[other] Entwurf im Postausgang gespeichert ({ $count } Entwürfe), noch nicht versendet. Exportiere die Antworten, wenn du bereit bist.
}
composer-save-error = Entwurf konnte nicht gespeichert werden: { $error }
composer-forwarding-origin = Weiterleitung von { $origin }
composer-replying-origin = Antwort auf { $origin }
composer-save-draft = Entwurf speichern
composer-save-draft-tooltip = Nachricht im Postausgang aufbewahren (Ctrl+S)
composer-cancel = Abbrechen
composer-cancel-tooltip = Ohne Speichern schließen (Esc)
composer-delete-draft = Entwurf löschen
composer-conference = Konferenz
composer-conference-number = Konferenz { $number }
composer-private = Privat
composer-private-tooltip = Nur der Empfänger und der Sysop können private Nachrichten lesen
composer-field-to = An
composer-field-subject = Betreff
composer-field-from = Von
composer-qwk-header-limit-tooltip = QWK-Kopffelder können bis zu 25 Zeichen enthalten
composer-address-book-tooltip = Aus dem Adressbuch auswählen (Ctrl+B)
composer-tagline = Tagline
composer-no-tagline = Keine Tagline
composer-choose-tagline-tooltip = Tagline auswählen (Ctrl+T)
composer-random-tagline = Zufällige Tagline

# Terminal editor
editor-paste-replaced = { $count } eingefügte Zeichen sind nicht in CP437 enthalten und wurden durch „?“ ersetzt
editor-character-not-cp437 = „{ $character }“ ist kein CP437-Zeichen - Ctrl+G öffnet die Zeichentabelle
editor-no-original-to-quote = Keine ursprüngliche Nachricht zum Zitieren vorhanden
editor-find-not-found = „{ $query }“ wurde nicht gefunden
editor-qwk-separator-unusable = Zeichen 227 (E3h) ist der QWK-Zeilentrenner und kann nicht verwendet werden
editor-control-code-unusable = Zeichen { $code } ({ $hex }h) ist ein Terminal-Steuerzeichen und kann nicht verwendet werden
editor-text-color-tooltip = Textfarbe (Ctrl+K)
editor-placeholder = Schreib deine Nachricht… F1 zeigt alle Tastenkürzel.
editor-characters = Zeichen
editor-characters-tooltip = CP437-Zeichentabelle (Ctrl+G)
editor-quote = Zitieren
editor-quote-tooltip = Ursprüngliche Nachricht zitieren (Ctrl+Q)
editor-quote-disabled-tooltip = Nur bei Antworten gibt es eine Nachricht zum Zitieren
editor-find = Suchen
editor-find-tooltip = Text suchen (Ctrl+F, F3 weitersuchen)
editor-keys-tooltip = Editor-Tastenkürzel (F1)
editor-find-in-message = In Nachricht suchen
editor-find-next-tooltip = Weitersuchen (F3)
editor-close-tooltip = Schließen (Esc)
editor-quote-from-original = Aus der ursprünglichen Nachricht zitieren
editor-quote-rest = Rest zitieren
editor-quote-rest-tooltip = Ausgewählte Zeile und alle folgenden zitieren (Ctrl+A)
editor-quote-line = Zeile zitieren
editor-quote-line-tooltip = Ausgewählte Zeile zitieren (Enter)
editor-quote-help = ↑↓ auswählen · Enter Zeile zitieren · Ctrl+A Rest zitieren · Esc schließen
editor-character-details = Zeichen { $code } · { $hex }h
editor-character-details-reserved = Zeichen { $code } · { $hex }h · reserviert
editor-character-picking-hint = Pfeiltasten auswählen · Enter einfügen · Esc zurück zum Text
editor-character-click-hint = Zum Einfügen klicken · Ctrl+G Auswahl per Tastatur
editor-selection-color = Farbe der Auswahl
editor-text-color = Textfarbe
editor-foreground = Vordergrund
editor-background = Hintergrund
editor-blink = Blinken
editor-color-preview-text = Der schnelle braune Fuchs
editor-color-help = ←→ ↑↓ 0-F auswählen · Leertaste blinken
editor-default = Standard
editor-default-color-tooltip = Hellgrau auf Schwarz (Entf)
editor-apply = Anwenden
editor-cancel = Abbrechen
editor-status-line = Zl
editor-status-column = Sp
editor-status-sample = Aa
editor-status-insert = EIN
editor-status-overwrite = ÜBR
editor-status-color = Farbe
editor-status-chars = Zeichen
editor-status-quote = Zitat
editor-status-find = Suche
editor-status-help = Hilfe
editor-toggle-insert-tooltip = Zwischen Einfügen und Überschreiben wechseln (Einfg)
editor-color-black = Schwarz
editor-color-blue = Blau
editor-color-green = Grün
editor-color-cyan = Cyan
editor-color-red = Rot
editor-color-magenta = Magenta
editor-color-brown = Braun
editor-color-light-gray = Hellgrau
editor-color-dark-gray = Dunkelgrau
editor-color-light-blue = Hellblau
editor-color-light-green = Hellgrün
editor-color-light-cyan = Hellcyan
editor-color-light-red = Hellrot
editor-color-light-magenta = Hellmagenta
editor-color-yellow = Gelb
editor-color-white = Weiß

# Reader view
reader-no-message-selected = Keine Nachricht ausgewählt
reader-pick-message-detail = Eine Nachricht aus der Liste auswählen, um sie hier zu lesen.
reader-next-message = Nächste Nachricht
reader-previous-message = Vorherige Nachricht
reader-copy-message-text = Nachrichtentext kopieren
reader-no-tagline = Diese Nachricht enthält keine Tagline
reader-save-tagline = Tagline „{ $tagline }“ speichern (T)
reader-add-author-address-book = Absender zum Adressbuch hinzufügen (Shift+A)
reader-more-actions = Weitere Aktionen
reader-menu-copy-message-text = Nachrichtentext kopieren
reader-menu-save-tagline = Tagline speichern
reader-menu-add-author = Absender zum Adressbuch hinzufügen
reader-mark-as-read-short = Als gelesen markieren (M)
reader-star-short = Markieren (S)
reader-unstar-short = Markierung entfernen (S)
reader-mark-as-unread-short = Als ungelesen markieren (M)
reader-forward-short = Weiterleiten (Ctrl+L)
reader-reply-short = Antworten (Ctrl+R)
reader-to = an
reader-private = Privat
reader-reply-to = Antwort auf #{ $number }
reader-show-original-message = Ursprüngliche Nachricht anzeigen
reader-copy = Kopieren
reader-copy-message = Nachricht kopieren
reader-reply = Antworten
reader-forward = Weiterleiten

# Draft preview
reader-no-draft-selected = Kein Entwurf ausgewählt
reader-no-draft-detail = Eine Antwort oder neue Nachricht schreiben, um den Postausgang zu füllen.
reader-conference-number = Konferenz { $number }
reader-kind-new-message = Neue Nachricht
reader-kind-reply = Antwort
reader-kind-forward = Weiterleitung
reader-delete-draft-short = Entwurf löschen (Entf)
reader-edit = Bearbeiten
reader-edit-draft-short = Entwurf bearbeiten (Enter)
reader-to-capital = An
reader-no-recipient = (kein Empfänger)
reader-from = von
reader-delete = Löschen
reader-fix-before-export = Vor dem Export korrigieren

# Bulletin preview
reader-copy-file-text = Text kopieren
reader-next-file = Nächste Datei
reader-previous-file = Vorherige Datei
file-kind-welcome = Begrüßung
file-kind-news = Neuigkeiten
file-kind-bulletin = Bulletin
file-kind-bulletin-number = Bulletin { $number }
file-kind-new-files = Neue Dateien
file-kind-goodbye = Verabschiedung

# Message list
list-draft-count = { $count ->
    [one] 1 Nachricht
   *[other] { $count } Nachrichten
}
list-message-count-unread = { $count } · { $unread } ungelesen
list-export-replies = Antworten exportieren…
list-outbox-steps = Entwürfe werden lokal gespeichert, nicht versendet. Exportiere die Antworten als .REP-Datei und lade sie dann in deine BBS hoch.
list-unread = Ungelesen
list-show-only-unread-messages = Nur ungelesene Nachrichten anzeigen
list-column-from = Von
list-column-subject-threads = Betreff (Diskussionsfäden)
list-column-subject = Betreff
list-column-date = Datum
list-date-today = Heute { $time }
list-date-yesterday = Gestern { $time }
list-date-weekday = { $weekday ->
    [0] Montag
    [1] Dienstag
    [2] Mittwoch
    [3] Donnerstag
    [4] Freitag
    [5] Samstag
   *[6] Sonntag
} { $time }
list-column-lines = Zeilen
list-column-title = Titel
list-column-file = Datei
list-reply = Antworten
list-forward = Weiterleiten
list-mark-as-read = Als gelesen markieren
list-star = Markieren
list-unstar = Markierung entfernen
list-star-tooltip = Markieren oder Markierung entfernen (S)
list-mark-as-unread = Als ungelesen markieren
list-nothing-matches = Keine Treffer für „{ $query }“
list-all-read = Alle Nachrichten in diesem Ordner wurden gelesen
list-no-personal = Keine Nachrichten an { $name }
list-folder-empty = Dieser Ordner ist leer
list-no-starred = Markiere Nachrichten mit S oder dem Stern daneben, um sie hier wiederzufinden.
list-no-messages = Keine Nachrichten

# Draft list
list-column-to = An
list-column-conference = Konferenz
list-column-written = Verfasst
list-no-recipient = (kein Empfänger)
list-needs-attention = Vor dem Export zu korrigieren: { $message }
list-edit = Bearbeiten
list-delete-ellipsis = Löschen…
list-outbox-empty-title = Der Postausgang ist leer
list-outbox-empty-detail = Antworten und neue Nachrichten bleiben hier, bis sie als Antwortpaket exportiert werden.

# Taglines

taglines-read-error =
    Taglines konnten nicht gelesen werden:
    { $error }
taglines-save-error = Taglines konnten nicht gespeichert werden: { $error }
taglines-count-none = Noch keine Taglines
taglines-count-one = 1 Tagline
taglines-count-many = { $count } Taglines
taglines-title-choose = Tagline auswählen
taglines-title-manage = Taglines
taglines-edit-heading = Tagline bearbeiten
taglines-new-heading = Neue Tagline
taglines-edit-hint = Ein witziger Einzeiler
taglines-save = Speichern
taglines-cancel = Abbrechen
taglines-shown-as = Wird unter der Nachricht als „... Text“ angezeigt · { $count }/{ $limit }
taglines-filter-hint = Taglines filtern
taglines-new-button = Neue Tagline
taglines-new-tooltip = Tagline zur Liste hinzufügen
taglines-empty-title = Noch keine Taglines
taglines-empty-help = Lieblingssprüche mit „Neue Tagline“ hinzufügen oder beim Lesen T drücken, um die Tagline einer Nachricht zu übernehmen.
taglines-no-filter-match = Keine Tagline enthält „{ $filter }“
taglines-delete-tooltip = Löschen (Entf)
taglines-edit-tooltip = Bearbeiten
taglines-random = Zufällig
taglines-random-tooltip = Zufällige Tagline aus der Liste auswählen
taglines-no-tagline = Keine Tagline
taglines-use = Tagline verwenden
taglines-close = Schließen

# Address book

address-read-error =
    Adressbuch konnte nicht gelesen werden:
    { $error }
address-save-error-multiline =
    Adressbuch konnte nicht gespeichert werden:
    { $error }
address-save-error = Adressbuch konnte nicht gespeichert werden: { $error }
address-sender-already-present = { $name } ist bereits im Adressbuch
address-sender-added = { $name } zum Adressbuch hinzugefügt
address-count-none = Noch keine Kontakte
address-count-one = 1 Kontakt
address-count-many = { $count } Kontakte
address-title-choose = Empfänger auswählen
address-title-manage = Adressbuch
address-edit-heading = Kontakt bearbeiten
address-new-heading = Neuer Kontakt
address-name-label = Name
address-name-hint = Name in der BBS
address-address-label = Adresse
address-address-hint = Optionale Netmail- oder Internetadresse
address-save = Speichern
address-cancel = Abbrechen
address-filter-hint = Nach Name oder Adresse filtern
address-new-button = Neuer Kontakt
address-new-tooltip = Kontakt zum Adressbuch hinzufügen
address-empty-title = Das Adressbuch ist leer
address-empty-help = Personen mit „Neuer Kontakt“ hinzufügen oder beim Lesen Shift+A drücken, um den Absender einer Nachricht hinzuzufügen.
address-no-filter-match = Kein Kontakt passt zu „{ $filter }“
address-delete-tooltip = Löschen (Entf)
address-edit-tooltip = Bearbeiten
address-use-recipient = Als Empfänger verwenden
address-write-message = Nachricht schreiben
address-write-message-tooltip = Neue Nachricht an diesen Kontakt beginnen
address-close = Schließen
address-duplicate-name = Ein Kontakt mit diesem Namen ist bereits im Adressbuch

# Draft validation

draft-field-conference = Konferenz
draft-field-from = Absender
draft-field-to = Empfänger
draft-field-subject = Betreff
draft-field-body = Nachrichtentext
draft-field-tagline = Tagline
draft-issue-conference-not-in-packet = Konferenz { $conference } ist nicht Teil dieses Pakets
draft-issue-field-required = { $field } muss angegeben werden
draft-issue-field-too-long = { $field } enthält { $length } Zeichen; QWK erlaubt { $limit }
draft-issue-message-text-required = Nachrichtentext muss angegeben werden
draft-issue-control-character = { $field } enthält ein Steuerzeichen (U+{ $code })
draft-issue-no-cp437-equivalent = { $field } enthält „{ $character }“, wofür es keine CP437-Entsprechung gibt
draft-issue-qwk-line-break = { $field } enthält „{ $character }“, das QWK als Zeilenumbruch reserviert

# Packet and reader

packet-error-unknown-archive-format = Kein Nachrichtenpaket: unbekanntes Archivformat
packet-error-control-dat-not-found = CONTROL.DAT im Archiv nicht gefunden
packet-error-control-dat-parse-failed = CONTROL.DAT konnte nicht verarbeitet werden: { $error }
packet-error-messages-dat-not-found = MESSAGES.DAT im Archiv nicht gefunden
packet-unreadable-message-subject = <nicht lesbare Nachricht #{ $number }>
packet-error-message-index-out-of-range = Nachrichtenindex außerhalb des gültigen Bereichs
packet-conference-fallback-name = Konferenz { $number }
packet-all-conferences = Alle Konferenzen
packet-user-data-directory-unavailable = Kein Benutzerdatenverzeichnis verfügbar

# Text and settings

text-configuration-directory-unavailable = Kein Konfigurationsverzeichnis verfügbar

# Window titles and startup
window-title = Icy Mail { $version }
window-packet-title = { $name } - Icy Mail
app-error-wgpu-unavailable = wgpu-Renderer nicht verfügbar
