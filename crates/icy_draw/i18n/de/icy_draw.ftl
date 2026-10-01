font-editor-table = Zeichentabelle 0-{ $length }:

unsaved-title=Unbenannt

# CLI (clap)
app-about = Icy Draw — ANSI/ASCII-Kunst erstellen, Fonts bearbeiten und in Echtzeit zusammenarbeiten
arg-path-help = Datei zum Öffnen beim Start
arg-mcp-port-help = MCP-Server auf dem angegebenen Port starten (z.B. --mcp-port 8080)
cmd-host-about = Kollaborations-Session in Echtzeit hosten (Moebius-kompatibel)
arg-host-port-help = Port zum Lauschen (Standard: 8000)
arg-host-bind-help = Bind-Adresse (Standard: 0.0.0.0)
arg-host-password-help = Session-Passwort (optional)
arg-host-max-users-help = Maximale Benutzerzahl (0 = unbegrenzt)
arg-host-file-help = Datei, die gehostet wird (optional; sonst leere 80x25-Leinwand)
arg-host-backup-folder-help = Ordner für Autosave-Backups (Standard: aktuelles Verzeichnis)
arg-host-interval-help = Autosave-Intervall in Minuten (Standard: 60, 0 = nur bei Shutdown speichern)

# Server banner
server-title = Icy Draw Kollaborations-Server
server-bind-address = Adresse
server-password = Passwort
server-document = Dokument
server-max-users = Max. Benutzer
server-connect-with = Verbinden mit
server-stop-hint = Drücke Strg+C um den Server zu beenden
server-none = (keines)
server-unlimited = unbegrenzt

# Server messages
server-error-unknown-format = Fehler: Unbekanntes Dateiformat für '{ $path }'
server-starting-empty-canvas = Starte mit leerer 80x25-Leinwand
server-loaded = Geladen: { $path } ({ $cols }x{ $rows })
server-error-loading-file = Fehler beim Laden der Datei '{ $path }': { $error }
server-error-invalid-bind-address = Fehler: Ungültige Bind-Adresse '{ $addr }': { $error }
server-error-runtime = Fehler beim Erstellen der Tokio-Laufzeitumgebung
server-error = Server-Fehler: { $error }

menu-file=&Datei
menu-new=Neu…
menu-open=Datei öffnen…
menu-open_recent=Zuletzt geöffnet
menu-open_recent_clear=Liste leeren
menu-no_recent_files=Keine zuletzt geöffneten Dateien
menu-clear_recent_files=Liste leeren
menu-save=Speichern
menu-import=Importieren
menu-edit-sauce=SAUCE Info bearbeiten…
menu-9px-font=9px Font
menu-aspect-ratio=Klassisches Seitenverhältnis
menu-set-canvas-size=Leinwandgröße ändern…
menu-file-settings=Datei-Einstellungen…
menu-import-fonts=Schriften importieren…
menu-export-font=Schrift exportieren…

file-settings-dialog-title=Datei-Einstellungen
file-settings-canvas-size=Zeichenfläche
file-settings-font-dims=Schriftarten-Zellgröße
file-settings-font-height=Schrifthöhe
file-settings-format=Format
file-settings-sauce=SAUCE
file-settings-title=Titel
file-settings-author=Autor
file-settings-group=Gruppe
file-settings-ice=Ice-Farben
file-settings-legacy-ar=Klassisches Seitenverhältnis
file-settings-9px-font=9px-Schrift
file-settings-comments-button=Kommentare…
file-settings-settings-button=Einstellungen…
file-settings-comments-title=SAUCE-Kommentare
file-settings-comments-info=Max 255 Zeilen, 64 Zeichen pro Zeile
file-settings-format-legacy-dos=16 feste Farben, einzelne Schrift
file-settings-format-xbin=16 Farben, eigene Palette, einzelne Schrift
file-settings-format-xbin-extended=8 VG-Farben, 2 Schriften, eigene Palette
file-settings-format-unrestricted=Volle RGB-Farben, unbegrenzte Schriften
menu-close=Schließen
menu-save-as=Speichern unter…
menu-connect-to-server=Mit Server verbinden…
menu-close-editor=Editor schließen
menu-quit-app=App beenden
menu-toggle-chat=Chat-Panel umschalten

# Mit Server verbinden Dialog
collab-nickname=Nickname
collab-nickname-placeholder=Dein Name
collab-group=Gruppe
collab-group-placeholder=Optional
collab-password=Passwort
collab-password-placeholder=Optional
collab-server-url=Server URL
collab-server-url-placeholder=host:port oder ws://host:port
collab-connect-button=Verbinden
button-cancel=Abbrechen

# Collaboration Verbindungsfehler
collab-connection-lost-title=Verbindung getrennt
collab-connection-lost-message=Die Verbindung zum Collaboration-Server wurde getrennt.
collab-connection-error-title=Verbindungsfehler
collab-connection-error-message=Die Verbindung zum Collaboration-Server ist fehlgeschlagen.

    Fehler: { $error }

# Collaboration Chat-Panel
collab-user-joined={ $nick } ist beigetreten
collab-user-joined-group={ $nick } <{ $group }> ist beigetreten
collab-user-left={ $nick } hat den Chat verlassen
collab-user-left-group={ $nick } <{ $group }> hat den Chat verlassen
collab-no-other-users=Keine anderen Benutzer
collab-you=Du
collab-guest=Gast
collab-no-messages=Noch keine Nachrichten
collab-type-message=Nachricht eingeben...
collab-yesterday=gestern
collab-days-ago=vor { $days }T

# Collaboration Benutzerstatus
collab-status-active=Aktiv
collab-status-idle=Inaktiv
collab-status-away=Abwesend
collab-status-web=Web

# Dateidialoge
file-dialog-save-as-title=Speichern unter
file-dialog-filter-all-files=Alle Dateien
file-dialog-filter-icydraw-files=IcyDraw-Dateien
file-dialog-filter-font-files=Font-Dateien
file-dialog-filter-tdf-files=TDF-Dateien
file-dialog-filter-animation-files=Animationsdateien
menu-export=Exportieren…
menu-edit-font-outline=Font Outline…
menu-show_settings=Einstellungen…
menu-set-font-size=Schriftgröße ändern…

font-size-width=Breite
font-size-height=Höhe

menu-edit=&Bearbeiten
menu-undo=Rückgängig
menu-redo=Wiederherstellen
menu-undo-op=Rückgängig: { $op }
menu-redo-op=Wiederherstellen: { $op }

menu-cut=Ausschneiden
menu-copy=Kopieren
menu-paste=Einfügen
menu-delete=Löschen
menu-rename=Umbenennen
menu-paste-as=Einfügen als
menu-paste-as-new-image=Neues Bild
menu-paste-as-sixel=Sixel Layer
menu-paste-as-brush=Neuer Pinsel
menu-insert-sixel-from-file=Sixel aus Datei einfügen…
menu-erase=Löschen
menu-flipx=X Spiegeln
menu-flipy=Y Spiegeln
menu-justifyleft=Linksbündig
menu-justifyright=Rechtsbündig
menu-justifycenter=Zentrieren
menu-crop=Zuschneiden
menu-justify_line_center=Zeile zentrieren
menu-justify_line_left=Zeile linksbündig
menu-justify_line_right=Zeile rechtsbündig
menu-insert_row=Zeile einfügen
menu-delete_row=Zeile löschen
menu-insert_colum=Spalte einfügen
menu-delete_colum=Spalte löschen
menu-erase_row=Zeile leeren
menu-erase_row_to_start=Zeile bis Anfang leeren
menu-erase_row_to_end=Zeile bis Ende leeren
menu-erase_column=Spalte leeren
menu-erase_column_to_start=Spalte bis Anfang leeren
menu-erase_column_to_end=Spalte bis Ende leeren
menu-scroll_area_up=Hoch scrollen
menu-scroll_area_down=Runter scrollen
menu-scroll_area_left=Links scrollen
menu-scroll_area_right=Rechts scrollen
menu-mirror_mode=Spiegelmodus
menu-area_operations=Bereichsoperationen

menu-selection=Au&swahl
menu-select-all=Alles auswählen
menu-select_nothing=Nichts
menu-inverse_selection=Invertieren

menu-colors=&Farben
menu-ice-mode=Ice-Modus
menu-ice-mode-unrestricted=Unbegrenzt
menu-ice-mode-blink=Blinken
menu-ice-mode-ice=Ice
menu-palette-mode=Palette
menu-palette-mode-unrestricted=Unbegrenzt
menu-palette-mode-dos=Dos 16
menu-palette-mode-free=Frei 16
menu-palette-mode-free8=Frei 8

menu-color-mode-ext-colors=256 Farben
menu-next_fg_color=Nächste Vordergrundfarbe
menu-next_bg_color=Nächste Hintergrundfarbe
menu-prev_fg_color=Vorherige Vordergrundfarbe
menu-prev_bg_color=Vorherige Hintergrundfarbe

menu-color-mode-reset=Farben zurücksetzen
menu-color-mode-reset-palette=Palette zurücksetzen
menu-color-mode-set-colors=Farben setzen
menu-color-mode-set-palette=Palette setzen

menu-view=&Ansicht
menu-reference-image=Referenzbild setzen…
menu-toggle-reference-image=Referenzbild ein/ausblenden
menu-clear-reference-image=Löschen
menu-toggle_fullscreen=Vollbildmodus
menu-zoom=Vergrößerung
menu-zoom_reset=Vergrößerung zurücksetzen
menu-zoom_in=Vergrößern
menu-zoom_out=Verkleinern
menu-guides=Hilfslinien
menu-raster=Gitter
menu-zoom-fit_size=Größe anpassen
menu-show_layer_borders=Ebenenrahmen anzeigen
menu-show_line_numbers=Zeilennummern anzeigen
menu-toggle_guide=Linien umschalten
menu-toggle_raster=Gitter umschalten
menu-toggle_left_pane=Linke Seitenleiste umschalten
menu-toggle_right_pane=Rechte Seitenleiste umschalten

menu-pick_attribute_under_caret=Attribut aufheben
menu-default_color=Standardfarbe
menu-toggle_color=Farbe umschalten

menu-fonts=Fonts
menu-font-mode=Font Modus
menu-font-mode-unrestricted=Unbegrenzt
menu-font-mode-sauce=Sauce
menu-font-mode-single=Einer
menu-font-mode-dual=Zwei
menu-open_font_selector=Schrift wählen…
menu-open_font_slot_manager=Font Slots verwalten…
menu-add_fonts=Schrift hinzufügen…
menu-open_font_manager=Bufferfonts bearbeiten…
menu-open_font_directoy=Text-Art-Fontverzeichnis öffnen…

menu-edit_palette=Palette bearbeiten…

menu-help=&Hilfe
menu-discuss=Diskussion
menu-open_log_file=Logdatei öffnen
menu-report-bug=Fehler melden
menu-about=Info…
menu-plugins=&Erweiterungen
menu-open_plugin_directory=Erweiterungsverzeichnis öffnen…
menu-upgrade_version=Neue Version { $version }
menu-update-available=⬆ Update verfügbar: { $version }

# BitFont Editor Menü
menu-clear-glyph=Zeichen löschen
menu-inverse-glyph=Zeichen invertieren
menu-fill-glyph=Zeichen füllen
menu-flip-x=Horizontal spiegeln
menu-flip-y=Vertikal spiegeln
menu-deselect=Auswahl aufheben

# BitFont Editor Commands
cmd-bitfont-clear-action = Löschen
cmd-bitfont-clear-desc = Aktuelles Zeichen löschen
cmd-bitfont-clear-menu = Löschen

cmd-bitfont-fill-action = Füllen
cmd-bitfont-fill-desc = Aktuelle Auswahl füllen
cmd-bitfont-fill-menu = Füllen

cmd-bitfont-inverse-action = Invertieren
cmd-bitfont-inverse-desc = Aktuelles Zeichen invertieren
cmd-bitfont-inverse-menu = Invertieren

cmd-bitfont-flip_x-action = X spiegeln
cmd-bitfont-flip_x-desc = Horizontal spiegeln
cmd-bitfont-flip_x-menu = Horizontal spiegeln

cmd-bitfont-flip_y-action = Y spiegeln
cmd-bitfont-flip_y-desc = Vertikal spiegeln
cmd-bitfont-flip_y-menu = Vertikal spiegeln

cmd-bitfont-toggle_letter_spacing-action = 8/9-Punkt Modus
cmd-bitfont-toggle_letter_spacing-desc = 9-Punkt Zellenmodus umschalten (VGA Zeichenabstand)
cmd-bitfont-toggle_letter_spacing-menu = 8/9-Punkt Zellenmodus

cmd-bitfont-swap_chars-action = Zeichen tauschen
cmd-bitfont-swap_chars-desc = Ausgewähltes Zeichen mit Zeichen am Cursor tauschen
cmd-bitfont-swap_chars-menu = Zeichen tauschen

cmd-bitfont-duplicate_line-action = Zeile duplizieren
cmd-bitfont-duplicate_line-desc = Aktuelle Zeile in allen Zeichen duplizieren
cmd-bitfont-duplicate_line-menu = Zeile duplizieren

cmd-bitfont-show_preview-action = Vorschau anzeigen
cmd-bitfont-show_preview-desc = Schriftvorschau anzeigen
cmd-bitfont-show_preview-menu = Vorschau anzeigen

cmd-category-bitfont = BitFont

# Selection Commands
cmd-select-none-action = Auswahl aufheben
cmd-select-none-desc = Aktuelle Auswahl aufheben
cmd-select-none-menu = Auswahl aufheben

cmd-select-inverse-action = Auswahl umkehren
cmd-select-inverse-desc = Auswahl umkehren
cmd-select-inverse-menu = Auswahl umkehren

cmd-select-erase-action = Löschen
cmd-select-erase-desc = Ausgewählten Bereich löschen
cmd-select-erase-menu = Löschen

cmd-select-flip_x-action = X spiegeln
cmd-select-flip_x-desc = Auswahl horizontal spiegeln
cmd-select-flip_x-menu = X spiegeln

cmd-select-flip_y-action = Y spiegeln
cmd-select-flip_y-desc = Auswahl vertikal spiegeln
cmd-select-flip_y-menu = Y spiegeln

cmd-select-crop-action = Zuschneiden
cmd-select-crop-desc = Auf Auswahl zuschneiden
cmd-select-crop-menu = Zuschneiden

cmd-select-justify_left-action = Linksbündig
cmd-select-justify_left-desc = Auswahl linksbündig ausrichten
cmd-select-justify_left-menu = Linksbündig

cmd-select-justify_center-action = Zentriert
cmd-select-justify_center-desc = Auswahl zentrieren
cmd-select-justify_center-menu = Zentriert

cmd-select-justify_right-action = Rechtsbündig
cmd-select-justify_right-desc = Auswahl rechtsbündig ausrichten
cmd-select-justify_right-menu = Rechtsbündig

cmd-category-selection = Auswahl

# Area Operations Commands
cmd-area-justify_line_left-action = Zeile linksbündig
cmd-area-justify_line_left-desc = Aktuelle Zeile linksbündig ausrichten
cmd-area-justify_line_left-menu = Zeile linksbündig

cmd-area-justify_line_center-action = Zeile zentrieren
cmd-area-justify_line_center-desc = Aktuelle Zeile zentrieren
cmd-area-justify_line_center-menu = Zeile zentrieren

cmd-area-justify_line_right-action = Zeile rechtsbündig
cmd-area-justify_line_right-desc = Aktuelle Zeile rechtsbündig ausrichten
cmd-area-justify_line_right-menu = Zeile rechtsbündig

cmd-area-insert_row-action = Zeile einfügen
cmd-area-insert_row-desc = Neue Zeile an Cursorposition einfügen
cmd-area-insert_row-menu = Zeile einfügen

cmd-area-delete_row-action = Zeile löschen
cmd-area-delete_row-desc = Zeile an Cursorposition löschen
cmd-area-delete_row-menu = Zeile löschen

cmd-area-insert_column-action = Spalte einfügen
cmd-area-insert_column-desc = Neue Spalte an Cursorposition einfügen
cmd-area-insert_column-menu = Spalte einfügen

cmd-area-delete_column-action = Spalte löschen
cmd-area-delete_column-desc = Spalte an Cursorposition löschen
cmd-area-delete_column-menu = Spalte löschen

cmd-area-erase_row-action = Zeile leeren
cmd-area-erase_row-desc = Gesamte Zeile am Cursor leeren
cmd-area-erase_row-menu = Zeile leeren

cmd-area-erase_row_to_start-action = Zeile bis Anfang leeren
cmd-area-erase_row_to_start-desc = Von Zeilenanfang bis Cursor leeren
cmd-area-erase_row_to_start-menu = Zeile bis Anfang leeren

cmd-area-erase_row_to_end-action = Zeile bis Ende leeren
cmd-area-erase_row_to_end-desc = Von Cursor bis Zeilenende leeren
cmd-area-erase_row_to_end-menu = Zeile bis Ende leeren

cmd-area-erase_column-action = Spalte leeren
cmd-area-erase_column-desc = Gesamte Spalte am Cursor leeren
cmd-area-erase_column-menu = Spalte leeren

cmd-area-erase_column_to_start-action = Spalte bis Anfang leeren
cmd-area-erase_column_to_start-desc = Von Spaltenanfang bis Cursor leeren
cmd-area-erase_column_to_start-menu = Spalte bis Anfang leeren

cmd-area-erase_column_to_end-action = Spalte bis Ende leeren
cmd-area-erase_column_to_end-desc = Von Cursor bis Spaltenende leeren
cmd-area-erase_column_to_end-menu = Spalte bis Ende leeren

cmd-area-scroll_up-action = Bereich nach oben rollen
cmd-area-scroll_up-desc = Ausgewählten Bereich nach oben rollen
cmd-area-scroll_up-menu = Bereich nach oben rollen

cmd-area-scroll_down-action = Bereich nach unten rollen
cmd-area-scroll_down-desc = Ausgewählten Bereich nach unten rollen
cmd-area-scroll_down-menu = Bereich nach unten rollen

cmd-area-scroll_left-action = Bereich nach links rollen
cmd-area-scroll_left-desc = Ausgewählten Bereich nach links rollen
cmd-area-scroll_left-menu = Bereich nach links rollen

cmd-area-scroll_right-action = Bereich nach rechts rollen
cmd-area-scroll_right-desc = Ausgewählten Bereich nach rechts rollen
cmd-area-scroll_right-menu = Bereich nach rechts rollen

cmd-category-area = Bereichsoperationen

# BitFont Werkzeuge Menü
menu-tools=Werkzeuge
menu-tool-click=Klick-Werkzeug
menu-tool-select=Auswahl-Werkzeug
menu-tool-rectangle=Rechteck-Werkzeug
menu-tool-fill=Füll-Werkzeug
menu-next-char=Nächstes Zeichen
menu-prev-char=Vorheriges Zeichen

tool-fg=Fg
tool-bg=Bg
tool-solid=Solid
tool-character=Zeichen
tool-shade=Schattieren
tool-colorize=Färben
tool-size-label=Größe
tool-full-block=Block
tool-half-block=Halbblock
tool-outline=Outline
tool-custom-brush=Benutzerdefinierter Pinsel

tool-select-label=Auswahlmodus
tool-select-normal=Rechteck
tool-select-character=Zeichen
tool-select-attribute=Attribute
tool-select-foreground=Vordergund
tool-select-background=Hintergrund
tool-select-description=Shift halten, um Auswahl hinzuzufügen. Control/Cmd zum Entfernen.

tool-fill-exact_match_label=Exakte Übereinstimmung
tool-flip_horizontal=Horizontal
tool-flip_vertical=Vertical

tool-paint_brush_name=Pinsel
tool-paint_brush_tooltip=Strecken mit einem Pinsel zeichnen
tool-click_name=Texteingabe
tool-click_tooltip=Text & rechteckige Auswahl
tool-ellipse_name=Ellipse
tool-ellipse_tooltip=Ellipsen malen
tool-filled_ellipse_name=Gefüllte Ellipse
tool-filled_ellipse_tooltip=Gefüllte Ellipsen malen
tool-rectangle_name=Rechteck
tool-rectangle_tooltip=Rechtecke malen
tool-filled_rectangle_name=Gefülltes Rechteck
tool-filled_rectangle_tooltip=Gefüllte Rechtecke malen
tool-eraser_name=Radierer
tool-eraser_tooltip=Bis zum Hintergrund löschen
tool-fill_name=Füllen
tool-fill_tooltip=Auswahl mit Farbe oder Zeichen füllen
tool-flip_name=Schalter
tool-flip_tooltip=Vertikale oder horizontale Halbblöcke umschalten
tool-tdf_name=The Draw Fonts
tool-tdf_tooltip=Texteingabe mit The Draw Fonts
tool-line_name=Linie
tool-line_tooltip=Linien malen
tool-move_layer_name=Ebene verschieben
tool-move_layer_tooltip=Ebenen verschieben
tool-pencil_name=Stift
tool-pencil_tooltip=Strecken mit Stift zeichnen
tool-pipette_name=Farbpipette
tool-pipette_tooltip=Farben von einer Position aufnehmen
tool-select_name=Auswahl
tool-select_tooltip=Mehrfachauswahl oder nicht rechteckige Auswahl
tool-tag_name=Tag Tool
tool-tag_tooltip=Tags werden im Output ersetzt
tool-tag_show=Tags anzeigen
tool-tag_edit_button={ menu-edit }

toolbar-new=Neu

new-file-title=Neue Datei
new-file-width=Breite
new-file-height=Höhe
new-file-ok=Ok
new-file-cancel=Abbrechen
new-file-create=Erstellen

edit-sauce-title=SAUCE Info
edit-sauce-title-label=Titel
edit-sauce-title-label-length=(35 Zeichen)
edit-sauce-author-label=Author
edit-sauce-author-label-length=(20 Zeichen)
edit-sauce-group-label=Gruppe
edit-sauce-group-label-length=(20 Zeichen)
edit-sauce-comments-label=Kommentare (64 Zeichen pro Zeile):
edit-sauce-letter-spacing=9 Pixel Modus
edit-sauce-aspect-ratio=Klassisches Seitenverhältnis

edit-canvas-size-title=Leinwandgröße
edit-canvas-size-width-label=Breite
edit-canvas-size-height-label=Höhe
edit-canvas-size-resize=Größe ändern
edit-canvas-size-resize_layers-label=Ebenen anpassen

toolbar-size = {$colums ->
     [1] 1 Spalte
*[other] {$colums} Spalten
} x { $rows ->
     [1] 1 Zeile
*[other] { $rows } Zeilen
}

toolbar-position = Zeile { $line }, Spalte { $column }
toolbar-layer_offset = Ebenen-Offset: { $line }x{ $column }

add_layer_tooltip = Neue Ebene
move_layer_up_tooltip = Ebene hoch
move_layer_down_tooltip = Ebene runter
delete_layer_tooltip = Ebene löschen
anchor_layer_tooltip = Ebene verankern

pipette-foreground = VG { $index }
pipette-background = HG { $index }
pipette-hover_hint = Über den Canvas fahren um Farben zu wählen
pipette-help = ⇧: Nur VG   ⌃: Nur HG

glyph-char-label=Zeichen
glyph-font-label=Schriftart

color-is_blinking=Blinken

export-title=Export
export-button-title=Export
export-file-label=Datei:
export-path-label=Pfad:
export-video-preparation-label=Video Vorbereitung:
export-video-preparation-None=Keine
export-video-preparation-Clear=Bildschirm löschen
export-video-preparation-Home=Cursor zurücksetzen
export-utf8-output-label=UTF-8 Ausgabe
export-save-sauce-label=SAUCE Info speichern
export-compression-label=Ausgabe komprimieren
export-limit-output-line-length-label=Maximale Ausgabe-Zeilenlänge
export-maximum_line_length=Maximale Zeilenlänge
export-use_repeat_sequences=Benutze `CSI Pn b`Sequenzen 
export-save_full_line_length=Abschließende Weißzeichen speichern
export-format-label=Format:
export-compression-level-label=Kompressionsgrad:

select-character-title=Zeichen auswählen

select-outline-style-title=Outline Stil auswählen

about-dialog-title=Über Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Icy Draw ist ein Tool, um Ansis & Asciis zu erstellen.
    Entwickelt wurde es mit Rust und egui.

    Icy Draw is freie Software unter der Apache 2 Lizenz.
    Die Homepage ist unter www.github.com/mkrueger/icy_draw
about-dialog-created_by =
    Erstellt von { $authors }
    Help & testing: NuSkooler, Grymmjack
edit-layer-dialog-title=Ebene bearbeiten
edit-layer-dialog-name-label=Name
edit-layer-dialog-is-visible-checkbox=Sichtbar
edit-layer-dialog-is-edit-locked-checkbox=Edit gesperrt
edit-layer-dialog-is-position-locked-checkbox=Position gesperrt
edit-layer-dialog-is-x-offset-label=X Versatz
edit-layer-dialog-is-y-offset-label=Y Versatz
edit-layer-dialog-has-alpha-checkbox=Hat Alphakanal
edit-layer-dialog-is-alpha-locked-checkbox=Alphakanal gesperrt

error-load-file=Fehler während des Dateiladens: { $error }

select-font-dialog-title=Font auswählen ({ $fontcount} verfügbar)
add-font-dialog-title=Font hinzufügen ({ $fontcount} verfügbar)
select-font-dialog-select=Auswählen
add-font-dialog-select=Hinzufügen
select-font-dialog-filter-text=Filter
select-font-dialog-no-fonts=Keine Fonts gefunden
select-font-dialog-no-fonts-installed=Keine Fonts installiert
select-font-dialog-color-font=FARBE
select-font-dialog-block-font=BLOCK
select-font-dialog-outline-font=OUTLINE
select-font-dialog-preview-text=HALLO
select-font-dialog-edit-button=Font bearbeiten…

layer_tool_title=Ebenen
layer_tool_menu_layer_properties=Ebeneneigenschaften…
layer_tool_menu_resize_layer=Ebenengröße…
layer_tool_menu_new_layer=Neue Ebene
layer_tool_menu_duplicate_layer=Duplizieren
layer_tool_menu_merge_layer=Ebene zusammenführen
layer_tool_menu_delete_layer=Ebene löschen
layer_tool_menu_clear_layer=Ebene leeren

channel_tool_title=Kanäle
channel_tool_fg=Vordergrund
channel_tool_bg=Hintergrund

font_tool_select_outline_button=Outline
font_tool_current_font_label=Aktueller TDF Font
font_tool_no_font=<nichts>
font_tool_no_fonts_label=
    Keine Fonts gefunden
    Installiere Fonts in das Text-Art-Fontverzeichnis
font_tool_open_directory_button=Text-Art-Fontverzeichnis öffnen

# Font Tool (neue Strings)
font-tool-no_fonts = Keine Text-Art-Fonts installiert
font-tool-open_directory = Text-Art-Fontverzeichnis öffnen
font-tool-select_font = Font auswählen…
font-tool-outline = Outline:
font-tool-outline_normal = Normal
font-tool-outline_round = Rund
font-tool-outline_square = Eckig
font-tool-outline_shadow = Schatten
font-tool-outline_3d = 3D
font-tool-outline_block1 = Block 1
font-tool-outline_block2 = Block 2
font-tool-outline_block3 = Block 3
font-tool-outline_block4 = Block 4
font-tool-outline_fancy1 = Fancy 1
font-tool-outline_fancy2 = Fancy 2
font-tool-outline_fancy3 = Fancy 3
font-tool-outline_fancy4 = Fancy 4
font-tool-outline_fancy5 = Fancy 5
font-tool-outline_fancy6 = Fancy 6
font-tool-outline_fancy7 = Fancy 7
font-tool-outline_fancy8 = Fancy 8
font-tool-outline_fancy9 = Fancy 9
font-tool-outline_fancy10 = Fancy 10

# TDF Font Selector Dialog
tdf-font-selector-filter_placeholder = Fonts filtern…
tdf-font-selector-type_outline = Kontur
tdf-font-selector-type_block = Block
tdf-font-selector-type_color = Farbe
tdf-font-selector-type_figlet = Figlet
tdf-font-selector-font_count = { $count } Fonts
tdf-font-selector-export = Exportieren…
tdf-font-selector-export_title = Font exportieren

# Set Font Dialog
set-font-load-font=Font laden…
set-font-filter-fonts=Font-Dateien
set-font-filter-all=Alle Dateien
set-font-load-error=Fehler beim Laden des Fonts
set-font-xbin-select-title=Font aus XBin-Datei auswählen
set-font-xbin-font=Font { $slot }
set-font-xbin-no-fonts=Keine Fonts in XBin-Datei gefunden

pipette_tool_char_code=Code { $code }
pipette_tool_foreground=Vordergrund { $fg }
pipette_tool_background=Hintergrund { $bg }
pipette_tool_keys=
    Shift halten für
    Vordergrund Farbe

    Strg halten für 
    Hintergrund Farbe

char_table_tool_title=Zeichentabelle
minimap_tool_title=Übersicht

no_document_selected=Kein Dokument ausgewählt

undo-draw-ellipse=Ellipse
undo-draw-rectangle=Rechteck
undo-paint-brush=Pinsel
undo-pencil=Stift
undo-eraser=Radierer
undo-bucket-fill=Füllen
undo-line=Linie
undo-cut=Ausschneiden
undo-paste-glyph=Zeichen einfügen
undo-bitfont-flip-y=Y spiegeln
undo-bitfont-flip-x=X spiegeln
undo-bitfont-move-down=Runter
undo-bitfont-move-up=Hoch
undo-bitfont-move-left=Links
undo-bitfont-move-right=Rechts
undo-bitfont-inverse=Invertieren
undo-bitfont-clear=Leeren
undo-bitfont-edit=Editieren
undo-bitfont-resize=Größe ändern
undo-bitfont-insert-line=Zeile einfügen
undo-bitfont-delete-line=Zeile löschen
undo-bitfont-insert-column=Spalte einfügen
undo-bitfont-delete-column=Spalte löschen
undo-bitfont-swap-chars=Zeichen tauschen
undo-bitfont-duplicate-line=Zeile duplizieren
undo-delete=Entfernen
undo-backspace=Rücktaste
undo-animation-edit=Editieren

undo-render_character=Zeichen rendern
undo-delete_character=Zeichen löschen
undo-select=Auswahl
undo-plugin=Erweiterung { $title }

font_selector-ansi_font=ANSI
font_selector-library_font=LIBRARY
font_selector-file_font=FILE
font_selector-sauce_font=SAUCE

select-palette-dialog-title=Palette auswählen ({ $count } verfügbar)
select-palette-dialog-builtin_palette=BUILTIN
select-palette-dialog-no-matching-palettes=Keine Paletten gefunden, die die Suche erfüllen

palette-editor-import=Importieren…
palette-editor-export=Exportieren…
palette-editor-invalid-hex=Ungültiger Hex-Wert

autosave-dialog-title=Autosave
autosave-dialog-description=Icy Draw hat eine Autosave Datei gefunden.
autosave-dialog-question=Was möchtest du tun?
autosave-dialog-load_autosave_button=Autosave laden
autosave-dialog-discard_autosave_button=Verwerfen

paste_mode-description=Einfügemodos. Im Layer Tool neue Ebene hinzufügen oder verankern.
paste_mode-stamp=Stempel
paste_mode-rotate=Rotieren
paste_mode-flipx=X Spiegeln
paste_mode-flipy=Y Spiegeln
paste_mode-transparent=Transparent

ask_close_file_dialog-description=Sollen die Änderungen in { $filename } gespeichert werden?
ask_close_file_dialog-subdescription=Alle Änderungen gehen beim Verwerfen verloren.
ask_close_file_dialog-dont_save_button=Verwerfen
ask_close_file_dialog-save_button=Speichern

tab-context-menu-close=Schließen
tab-context-menu-close_others=Andere schließen
tab-context-menu-close_all=Alle schließen
tab-context-menu-copy_path=Pfad kopieren

font-view-char_label=Zeichen
font-view-ascii_label=ASCII
font-view-font_label=Font
font-view-font_page_label=Font Seite:

font-editor-tile_area=Kacheln
font-editor-clear=Löschen
font-editor-inverse=Invertieren
font-editor-flip_x=X Spiegeln
font-editor-flip_y=Y Spiegeln

animation_editor_path_label=Datei:
animation_editor_export_button=Export
animation_editor_ansi_label=Ansimation
animation_encoding_frame=Berechne Bild { $cur } von { $total }
animation_of_frame_count=von { $total }
animation_icy_play_note=Für Animationen in der Konsole/BBS (oder zum Ansi honvertieren) braucht man:

new-file-template-cp437-title=CP437 ANSI
new-file-template-cp437-description=
    Ein neues DOS Ansi erstellen
    Limitiert zu 16 DOS Farben, Sauce Font, Blinken (kann umgestellt werden)
new-file-template-ice-title=CP437 Ice ANSI
new-file-template-ice-description=
    Ein neues DOS Ansi erstellen
    Limitiert zu 16 DOS Farben, Sauce Font, kein Blinken (kann umgestellt werden)
new-file-template-xb-title=XB 16 Colors
new-file-template-xb-description=
    Ein neues XB File erstellen
    Freie 16 Farben, 1 Font, kein Blinken (kann umgestellt werden)
new-file-template-xb-ext-title=XB Extended Font
new-file-template-xb-ext-description=
    Ein neues XB File mit 2 Fonts erstellen
    Freie 16 Farben, 8 Vordergrund, 16 Hintergrund, 2 Font, kein Blinken
new-file-template-ansi-title=Modernes ANSI
new-file-template-ansi-description=
    Ein neues Ansi erstellen, ohne Restriktionen
    Freie Palette, mehrere Fonts, Blinken
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Ein neues Atascii file erstellen

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=FILE_ID.DIZ erstellen
new-file-template-ansimation-title=Ansimation
new-file-template-ansimation-description=Eine Ansi-Animation erstellen
new-file-template-bit_font-title=Bit Font
new-file-template-bit_font-description=Einen neuen Bit-Font erstellen

new-file-category-ansi=ANSI Art
new-file-category-fonts=Schriften
new-file-category-animation=Animation

new-file-editor-ansi=ANSI Art
new-file-editor-atascii=ATASCII
new-file-editor-vt52=VT52 (Atari ST)
new-file-editor-petscii=PETSCII (C64/C128)
new-file-editor-bitfont=Bit Font
new-file-editor-tdf=TDF Font
new-file-editor-animation=Animation
new-file-size-section=Größe

new-file-template-color_font-title=TDF Farb-Font
new-file-template-color_font-description=Einen neuen TDF Farb-Font erstellen
new-file-template-block_font-title=TDF Block-Font
new-file-template-block_font-description=Einen neuen TDF Block-Font erstellen
new-file-template-outline_font-title=TDF Outline-font
new-file-template-outline_font-description=Einen neuen TDF Outline-Font erstellen
new-file-template-ansimation-ui-label=
    Eine IcyDraw Animation ist eine Lua Textdatei, die eine Animationssequenz beschreibt.
    Für eine vollständige Beschreibung der Syntax klicke auf diesen Link:
new-file-template-bitfont-ui-label=
    Ein Bit-Font ist eine Sammlung von Zeichen, die in einem Raster angeordnet sind.
    Bit-Fonts können in ANSI-Editoren verwendet werden.

new-file-template-thedraw-ui-label=
    TheDraw Fonts können in ANSI-Editoren verwendet werden, um größere Zeichen zu definieren.
    TheDraw definiert drei Font-Typen: Farbe, Block und Outline.

    Eine große Font-Sammlung kann hier heruntergeladen werden:

manage-font-dialog-title=Bufferfonts bearbeiten
manage-font-used_font_label=Verwendete Fonts
manage-font-copy_font_button=Kopieren
manage-font-copy_font_button-tooltip=Kopiert den Font als CTerm ANSI Sequenz ins Clipboard. (Für BBS)
manage-font-remove_font_button=Entfernen
manage-font-used_label=verwendet
manage-font-not_used_label=nicht verwendet
manage-font-replace_label=Ersetze Font mit:
manage-font-replace_font_button=Ersetzen
manage-font-change_font_slot_button=Slot ändern

palette_selector-dos_default_palette=VGA 16 Farben
palette_selector-dos_default_low_palette=VGA 8 Farben
palette_selector-c64_default_palette=C64 Farben
palette_selector-ega_default_palette=EGA 64 Farben
palette_selector-xterm_default_palette=XTerm erweiterte Palette
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Erzeugt aus offener Datei

tdf-editor-outline_preview_label=Outline Vorschau
tdf-editor-draw_bg_checkbox=Zeichne Hintergrund
tdf-editor-clone_button=Klonen
tdf-editor-font_name_label=Font-Name:
tdf-editor-spacing_label=Zeichenabstand:
tdf-editor-no_font_selected_label=Kein Font ausgewählt
tdf-editor-font_type_label=Font-Typ:
tdf-editor-font_type_color=Farbe
tdf-editor-font_type_block=Block
tdf-editor-font_type_outline=Outline
tdf-editor-clear_char_button=Zeichen Löschen
tdf-editor-cheat_sheet_key=Taste
tdf-editor-cheat_sheet_code=Code
tdf-editor-cheat_sheet_res=Res

tdf-dialog-add-font-title=Font hinzufügen
tdf-dialog-edit-settings-title=Font-Einstellungen
tdf-dialog-font-type=Font-Typ:
tdf-dialog-font-name=Name:
tdf-dialog-spacing=Abstand:

settings-heading=Einstellungen
settings-reset_button=Reset
settings-monitor-category=Monitor
settings-char-set-category=Zeichensätze
settings-font-outline-category=Font Outline
settings-paths-category=Pfade
settings-markers-guides-category=Markierungen
settings-keybindings-category=Tasten
settings-font-outline-header=Kontur-Stil
settings-charset-header=F-Tasten-Zeichensatz
settings-charset-set=Satz
settings-paths-header=Pfade
settings-paths-config-dir=Konfig-Verzeichnis:
settings-paths-config-file=Konfig-Datei:
settings-paths-log-file=Log-Datei:
settings-paths-font-dir=Schriftarten-Verzeichnis:
settings-paths-plugin-dir=Plugin-Verzeichnis:
settings-paths-taglists-dir=Taglisten:
settings-paths-open=Öffnen
settings-reference-alpha=Referenzbild Alpha
settings-raster-label=Gitterfarbe:
settings-alpha=alpha
settings-guide-label=Hilfslinienfarbe:
settings-set-label=Set { $set }
settings-key_filter_preview_text=Tastenzuweisung filtern
settings-char_set_list_label=Zeichnsatzliste:

edit-tag-title=Tag
edit-tag-filter=Tags filtern
edit-tag-preview-label=Vorschau:
edit-tag-replacement-label=Ersatz:
edit-tag-alignment-label=Ausrichtung:
edit-tag-length-label=Länge:
edit-tag-alignment-left=Links
edit-tag-alignment-right=Rechts
edit-tag-alignment-center=Zentriert
edit-tag-placement-label=Platzierung:
edit-tag-placement-in_line=In Zeile
edit-tag-placement-after=Mit GotoXY
edit-tag-role-label=Rolle:
edit-tag-role-displaycode=Darstellungscode
edit-tag-role-hyperlink=Hyperlink

add_tag_tooltip=Tag hinzufügen
delete_tag_tooltip=Tag löschen
clone_tag_tooltip=Tag duplizieren

ask_unsaved_file_dialog-description=Sollen die Änderungen in {
    $number ->
        [1] dieser Datei gespeichert werden?
        *[other] {$number} Dateien gespeichert werden?
    }
ask_unsaved_file_dialog-subdescription=Alle Änderungen gehen verloren, wenn nicht gespeichert wird.
ask_unsaved_file_dialog-save_all_button=Alle Speichern
ask_unsaved_file_dialog-dont_save_button=Nicht Speichern

# Save Changes Dialog (single file)
save-changes-title=Änderungen in "{ $filename }" speichern?
save-changes-description=Alle Änderungen gehen verloren, wenn nicht gespeichert wird.

# Paste Tool Toolbar
paste-tool-stamp=Stempeln (S)
paste-tool-rotate=Drehen (R)
paste-tool-flip-x=X spiegeln
paste-tool-flip-y=Y spiegeln
paste-tool-transparent=Transparent (T)
paste-tool-hint=Enter: Verankern | Esc: Abbrechen | Pfeile: Bewegen
paste-image-title=Bild einfügen
paste-image-hint=Enter: Verankern | Esc: Abbrechen | Pfeile: Bewegen

# Animation Export Dialog
animation-export-format=Format
animation-export-path=Exportieren nach
animation-export-no-path=Kein Pfad ausgewählt
animation-export-success=Export erfolgreich abgeschlossen
animation-export-exporting-frame=Exportiere Frame { $current } / { $total }
animation-export-encoding=Kodiere Video…
animation-export-cancelled=Export abgebrochen
animation-export-no-frames=Keine Frames zum Exportieren
animation-export-failed=Export fehlgeschlagen: { $error }

# Animation Editor
animation-compiling=Kompiliere Skript…
animation-no-frames=Keine Frames generiert
animation-preparing=Bereite Vorschau vor…
animation-no-log=Keine Log-Einträge
animation-frame-display=Frame { $current } / { $total }

# Reference Image Dialog
reference-image-dialog-title=Referenzbild
reference-image-path=Pfad
reference-image-path-placeholder=Bilddatei auswählen…
reference-image-browse=Durchsuchen…
reference-image-alpha=Deckkraft
reference-image-info=Das Referenzbild wird als Overlay auf der Leinwand angezeigt.
reference-image-clear=Löschen

# Tag Tool Toolbar
tag-toolbar-add=Hinzufügen
tag-toolbar-tags=Tags…
tag-toolbar-edit=Bearbeiten
tag-toolbar-delete=Löschen
tag-toolbar-delete-selected={ $count } Ausgewählte löschen
tag-toolbar-selected-tags={ $count } Tags ausgewählt
tag-toolbar-no-replacement=(kein Ersetzungstext)
tag-toolbar-add-hint=Klicken zum Platzieren, ESC zum Abbrechen

# Tag List Dialog
tag-list-title=Tags
tag-list-preview=Vorschau
tag-list-pos=Pos
tag-list-placement=Platzierung
tag-list-replacement=Ersetzung
tag-list-no-tags=Keine Tags
tag-list-in-text=Im Text
tag-list-with-gotoxy=Mit GotoXY

# Tag Edit Dialog
tag-edit-preview=Vorschau
tag-edit-replacement=Ersetzung
tag-edit-position=Position
tag-edit-filter=Filter…

# Tool / brush discoverability hints (status bar)
tool-hint-click=Klick  •  Zeichen tippen oder rechteckige Auswahl ziehen
tool-hint-select=Auswählen  •  Ziehen zum Auswählen, Shift = hinzufügen, Alt = abziehen
tool-hint-pencil=Stift  •  { $brush }
tool-hint-line=Linie  •  { $brush }
tool-hint-rectangle=Rechteck  •  { $brush }
tool-hint-filled_rectangle=Gefülltes Rechteck  •  { $brush }
tool-hint-ellipse=Ellipse  •  { $brush }
tool-hint-filled_ellipse=Gefüllte Ellipse  •  { $brush }
tool-hint-pipette=Farbpipette  •  Klicken um Vorder-/Hintergrund/Zeichen zu übernehmen
tool-hint-fill=Füllen  •  { $brush }
tool-hint-font=Schrift  •  TDF/Figlet-Cursor setzen, dann tippen
tool-hint-tag=Tag  •  Klicken um einen expandierenden Tag zu setzen
tool-hint-paste=Einfügen  •  Klicken zum Übernehmen, Esc zum Abbrechen
brush-hint-char=Zeichenmodus — malt '{ $ch }'
brush-hint-half_block=Halbblock-Modus (2× vertikale Auflösung)
brush-hint-shading=Schattier-Modus (LMT heller, RMT dunkler)
brush-hint-replace=Ersetzen-Modus (färbt vorhandene Zeichen um)
brush-hint-blink=Blink-Modus (schaltet Blink-Attribut um)
brush-hint-colorize=Einfärben-Modus (ändert nur Farben)

## egui editor
menu-document=&Dokument
menu-keyboard-shortcuts=Tastenkürzel…
menu-github=Icy Draw auf GitHub
menu-disconnect=Verbindung trennen
menu-new-window=Neues Fenster
menu-insert-image=Bild aus Datei einfügen…
menu-area-operations=Bereichsoperationen
menu-edit-bitmap-font=Bitmap-Schrift bearbeiten…
menu-zoom-fit_window=An Fenster anpassen
menu-zoom-fit_width=An Breite anpassen
menu-off=Aus
menu-show-guide=Hilfslinien anzeigen
menu-show-raster=Raster anzeigen
menu-character-grid=Zeichenraster
menu-side-panel=Seitenleiste
menu-chat-panel=Chat-Bereich
menu-appearance=Erscheinungsbild
menu-character-table=Zeichentabelle…
menu-monitor-settings=Monitor-Einstellungen…
menu-run-script=Lua-Skript ausführen…
menu-reload-plugins=Erweiterungen neu laden
error-bitmap-font-width=Der Bitmap-Schrifteditor unterstützt derzeit Zeichen mit bis zu 8 Pixeln Breite.
error-load-image={ $path } konnte nicht geladen werden: { $error }
error-load-reference-image=Referenzbild { $path } konnte nicht geladen werden
shortcuts-dialog-title=Tastenkürzel
shortcut-new-document=Neues Dokument
shortcut-open=Öffnen
shortcut-save=Speichern
shortcut-save-as=Speichern unter
shortcut-export=Exportieren
shortcut-new-window=Neues Fenster
shortcut-quit=Beenden
shortcut-undo=Rückgängig
shortcut-redo=Wiederholen
shortcut-cut=Ausschneiden
shortcut-copy=Kopieren
shortcut-paste=Als schwebende Ebene einfügen
shortcut-select-all=Alles auswählen
shortcut-deselect=Auswahl aufheben
shortcut-invert-selection=Auswahl umkehren
shortcut-erase-selection=Auswahl löschen
shortcut-cancel-stroke=Strich abbrechen / Auswahl aufheben
shortcut-zoom-in=Vergrößern
shortcut-zoom-out=Verkleinern
shortcut-zoom-fit=An Fenster anpassen
shortcut-zoom-actual=Originalgröße (100%)
shortcut-zoom=Zoom
shortcut-toggle-grid=Zeichenraster ein/aus
shortcut-toggle-line-numbers=Zeilennummern ein/aus
shortcut-toggle-guide=Hilfslinien ein/aus
shortcut-reference-image=Referenzbild
shortcut-toggle-reference-image=Referenzbild ein/aus
shortcut-toggle-side-panel=Seitenleiste ein/aus
shortcut-fullscreen=Vollbild
shortcut-next-fg=Nächste Vordergrundfarbe
shortcut-prev-fg=Vorherige Vordergrundfarbe
shortcut-next-bg=Nächste Hintergrundfarbe
shortcut-prev-bg=Vorherige Hintergrundfarbe
shortcut-pick-attribute=Attribut unter dem Cursor übernehmen
shortcut-swap-colors=Vorder-/Hintergrund tauschen
shortcut-group-text=Textwerkzeug
shortcut-fkey-type=Zeichen aus dem aktiven F-Tasten-Satz eingeben
shortcut-fkey-set-low=F-Tasten-Satz 1–10 wählen
shortcut-fkey-set-high=F-Tasten-Satz 11–20 wählen
shortcut-fkey-prev=Vorheriger F-Tasten-Satz
shortcut-fkey-next=Nächster F-Tasten-Satz
shortcut-fkey-default=Standard-F-Tasten-Satz
shortcut-extend-selection=Auswahl erweitern
shortcut-toggle-insert=Einfügen / Überschreiben umschalten
shortcut-hard-blank=Festes Leerzeichen (0xFF)
shortcut-tab-stop=Nächster Tabstopp
shortcut-group-brushes=Pinsel & Formen
shortcut-brush-larger=Größerer Pinsel
shortcut-brush-smaller=Kleinerer Pinsel
shortcut-brush-reset=Pinselgröße zurücksetzen
shortcut-group-paste=Schwebendes Einfügen
shortcut-paste-move=Verschieben
shortcut-paste-anchor=Verankern
shortcut-paste-stamp=Kopie stempeln
shortcut-paste-rotate=Drehen
shortcut-paste-flip-x=Horizontal spiegeln
shortcut-paste-flip-y=Vertikal spiegeln
shortcut-paste-transparent=Transparenz umschalten
shortcut-paste-cancel=Abbrechen
shortcut-toggle-fg=Vordergrundfarbe 1–7 umschalten (dunkel / hell)
shortcut-toggle-bg=Hintergrundfarbe 0–7 umschalten (dunkel / hell)
shortcut-default-colors=Standardfarben
shortcut-ice-colors=iCE-Farben umschalten
shortcut-letter-spacing=9px-Schrift umschalten
shortcut-canvas-size=Leinwandgröße festlegen
shortcut-mirror-mode=Spiegelmodus umschalten
shortcut-group-selection=Auswahlwerkzeug
shortcut-block-move=Block verschieben (schwebend, darunter löschen)
shortcut-block-copy=Block kopieren (schwebende Kopie)
shortcut-block-fill=Mit Vordergrundfarbe füllen
shortcut-block-erase=Löschen
shortcut-crop=Auf Auswahl zuschneiden
shortcut-brush-char=Pinselzeichen aus dem aktiven F-Tasten-Satz
shortcut-group-tags=Tag-Werkzeug
shortcut-tag-nudge=Ausgewählte Tags verschieben
shortcut-tag-nudge-far=Ausgewählte Tags um 10 Zellen verschieben
shortcut-tag-next=Nächsten / vorherigen Tag auswählen
shortcut-tag-clipboard=Tags kopieren, ausschneiden und am Cursor einfügen
shortcut-tag-duplicate=Ausgewählte Tags eine Zeile tiefer duplizieren
shortcut-tag-delete=Ausgewählte Tags löschen
shortcut-group-modes=Werkzeugtasten (außerhalb der Textwerkzeuge)
shortcut-mode-keyboard=Tastatur (Textcursor)
shortcut-mode-brush=Pinsel
shortcut-mode-shifter=Schattierungspinsel
shortcut-mode-fill=Farbeimer
shortcut-paste-center=Horizontal zentrieren
reference-image-image=Bild
reference-image-not-found=Datei nicht gefunden
reference-image-display=Anzeige
reference-image-mode=Modus
reference-image-mode-stretch=Auf Leinwand strecken
reference-image-mode-contain=An Leinwand anpassen
reference-image-mode-fit_width=An Breite anpassen
reference-image-mode-fit_height=An Höhe anpassen
reference-image-mode-original=Originalgröße
reference-image-mode-tile=Kacheln
reference-image-scale=Skalierung
reference-image-offset=Versatz
outline-style-label=Stil { $style }
outline-style-choose=Outline-Stil wählen
outline-style-tooltip=Stil { $style } ({ $key })
new-kind-ansi-description=Textmodus-Leinwand mit Ebenen
new-kind-atascii-description=Atari-8-Bit-Textbildschirm
new-kind-vt52-description=Atari-ST-Textbildschirm mit Farben
new-kind-petscii-description=Commodore-Bildschirm mit 40 Zeichen
new-kind-animation-description=Lua-gesteuerte ANSI-Animation
new-kind-bitfont-description=8 × 16 Pixel Konsolenschrift
new-kind-color_font-description=Zeichen mit Farben
new-kind-block_font-description=Blockzeichen, eine Farbe
new-kind-outline_font-description=Outline-Platzhalter
file-dialog-filter-images=Bilder
file-dialog-filter-artwork=Unterstützte Zeichenkunstformate
file-dialog-filter-palette=Palette
error-export-overwrites-document=Der Export darf das aktuell bearbeitete Dokument nicht überschreiben. Bitte einen anderen Dateinamen wählen.
tdf-font-number=Font { $index }
tdf-duplicate-font=Font duplizieren
tdf-delete-font=Font löschen
tdf-new-font-name=Neuer Font
tdf-edit-glyph='{ $ch }' bearbeiten
tdf-create-glyph='{ $ch }' erstellen
paste-tool-anchor=Verankern (Enter)
paste-tool-keep=Als Ebene behalten
paste-tool-cancel=Einfügen abbrechen (Escape)
brush-mode-replace=Ersetzen
brush-mode-char-tooltip=Mit dem gewählten Zeichen und den Farben malen
brush-mode-half_block-tooltip=Halbblock-Pixel mit doppelter vertikaler Auflösung malen
brush-mode-shading-tooltip=Zeichen schattieren: Linksklick heller, Rechtsklick dunkler
brush-mode-replace-tooltip=Vorhandene Zeichen ersetzen, Farben beibehalten
brush-mode-blink-tooltip=Blink-Attribut umschalten
brush-mode-colorize-tooltip=Nur die Farben vorhandener Zeichen ändern
brush-char-tooltip=Pinselzeichen – klicken, um es aus der Zeichentabelle zu wählen
brush-size=Pinselgröße
brush-size-tooltip=Pinselgröße in Zellen (Alt+Plus / Alt+Minus)
brush-apply=Anwenden
brush-apply-fg-tooltip=Vordergrundfarbe anwenden
brush-apply-bg-tooltip=Hintergrundfarbe anwenden
brush-exact-tooltip=Nur Zellen füllen, die in Zeichen und Farben exakt übereinstimmen
fkey-set-of=Satz { $set } von { $count }
fkey-set-tooltip=F-Tasten-Zeichensatz (Strg+Komma / Strg+Punkt zum Wechseln, Rechtsklick auf eine Taste zum Neubelegen)
select-mode-normal-tooltip=Rechteckige Auswahl aufziehen
select-mode-character-tooltip=Alle Zellen mit dem angeklickten Zeichen auswählen
select-mode-attribute-tooltip=Alle Zellen mit den angeklickten Farben auswählen
select-mode-foreground-tooltip=Alle Zellen mit der angeklickten Vordergrundfarbe auswählen
select-mode-background-tooltip=Alle Zellen mit der angeklickten Hintergrundfarbe auswählen
select-copy=Auswahl kopieren
select-deselect=Auswahl aufheben
select-justify=Ausrichten
select-mode-add=Zur Auswahl hinzufügen
select-mode-subtract=Von Auswahl abziehen
tag-new=Neuer Tag
tag-edit=Tag bearbeiten
tag-delete-selected=Ausgewählte Tags löschen
tag-place-hint=Doppelklick oder in einer Zeile ziehen fügt einen Tag hinzu · Tag ziehen verschiebt ihn · Umschalt+Ziehen wählt aus
tag-empty=(leer)
tag-info=bei { $x }, { $y }  ·  { $length } Zeichen
pipette-modifier-hint=Umschalt: nur Vordergrund  ·  Strg/Rechtsklick: nur Hintergrund
palette-title=Palette
tag-new-menu=Neuer Tag…
tag-properties-menu=Eigenschaften…
tag-duplicate=Duplizieren
error-export-font=Font konnte nicht exportiert werden: { $error }
font-preview-text-hint=Vorschautext
font-favorites=Favoriten
font-filter-min=min
font-filter-max=max
font-filter-any=0 = beliebig
monitor-save-defaults=Als Standard speichern
script-group=Lua-Skript
script-hint=-- Läuft auf dem aktuellen Dokument, z. B.
    -- buf:set_char(0, 0, "A")
script-output=Ausgabe
script-run=Ausführen
tag-list-empty-hint=Hier hinzufügen oder mit dem Tag-Werkzeug platzieren.
tag-double-click-edit=Doppelklick zum Bearbeiten
tag-disabled=deaktiviert
tag-edit-menu=Tag bearbeiten…
tag-add-menu=Tag hinzufügen…
tag-enabled=Aktiviert
tag-preview-hint=Wird im Editor angezeigt
tag-replacement-hint=Wird vom BBS eingesetzt
tag-replacement-browse=Aus einer Ersetzungsliste wählen
tag-replacements-title=Ersetzungen
tag-replacements-none=Keine passenden Ersetzungen
tag-replacements-example=Beispiel: { $example }
tag-replacements-notes=Hinweise
tag-replacements-back=Zurück
tag-replacements-import=Importieren…
tag-replacements-new=Neue Liste
tag-replacements-new-tooltip=Legt eine Liste aus einer Vorlage an und öffnet sie im Editor
tag-replacements-open-folder=Ordner öffnen
tag-replacements-custom-hint=Eigene Listen sind TOML-Dateien in diesem Ordner; sie erscheinen beim nächsten Öffnen der Liste
tag-replacements-filter=Ersetzungslisten
tag-replacements-no-folder=Es gibt keinen Ordner für Ersetzungslisten.
tag-replacements-import-failed={ $file } konnte nicht importiert werden: { $error }
tag-role=Rolle
tag-layout=Layout
tag-column=Spalte
tag-row=Zeile
tag-length=Länge
tag-alignment=Ausrichtung
unit-characters=Zeichen
unit-lines=Zeilen
button-apply=Übernehmen
replace-export-file=Exportdatei ersetzen?
replace-font-file=Fontdatei ersetzen?
replace-file=Datei ersetzen?
sauce-information=SAUCE-Informationen
sauce-limit-hint=Bis zu { $limit } Zeichen
sauce-comments=Kommentare
export-file-format=Dateiformat
export-ansi-options=ANSI-Optionen
export-compatibility=Kompatibilität
export-rgb-colors=24-Bit-RGB-Farben
export-output=Ausgabe
export-screen-preparation=Bildschirmvorbereitung
characters-title=Zeichen
characters-assign-fkey=F{ $key } belegen
new-file-type=Typ
new-file-custom-size=Benutzerdefiniert
new-file-preset=Vorlage
canvas-current-size=Aktuelle Größe
canvas-fixed-width=Der Bildschirmmodus legt die Breite fest
atascii-new-mode=Bildschirm
atascii-mode-antic=Atari · { $columns } × { $rows }
atascii-mode-xep80=XEP80 · { $columns } × { $rows }
atascii-new-hint=Ein Zeichensatz und zwei Farben für den ganzen Bildschirm; inverse Zeichen gehören zum Zeichensatz. Längere Bilder scrollen.
atascii-inverse=Invers
atascii-inverse-tooltip=Invers schreiben, wie mit der Invers-Taste des Atari
atascii-brush-tooltip=Pinselzeichen #{ $code }
atascii-fkeys-previous=Vorherige Funktionstasten-Belegung
atascii-fkeys-next=Nächste Funktionstasten-Belegung
atascii-fkeys-set=Belegung { $set } von { $count }
atascii-characters=Zeichen
atascii-screen=Bildschirm
atascii-background=Hintergrund
atascii-background-tooltip=Die Bildschirmfarbe (COLOR2)
atascii-text-luminance=Text
atascii-text-luminance-tooltip=Der Text hat den Farbton des Hintergrunds mit eigener Helligkeit (COLOR1)
atascii-colors-hint=Die Farben gehören zum Bildschirm; .ata-Dateien speichern sie nicht.
atascii-font=Zeichensatz
atascii-font-load=Atari-Zeichensatz laden…
atascii-font-load-tooltip=Ein Atari-Zeichensatz (.fnt, 1024 Bytes) oder ein 8 × 8-Zeichensatz wie PSF
atascii-font-filter=Atari-Zeichensätze
atascii-font-size=Atari-Zeichensätze sind 8 × 8 Pixel groß; dieser ist { $width } × { $height }.
atascii-pixels=Pixel
atascii-pixels-tooltip=2 × 2 Pixel pro Zeichen mit Viertelblöcken zeichnen; die rechte Maustaste radiert
atascii-inverse-pen=Invertieren
atascii-inverse-pen-tooltip=Zeichen invers machen; die rechte Maustaste macht sie wieder normal
atascii-screen-mode=Bildschirmmodus
atascii-pipette-hint=Ein Zeichen anklicken, um damit zu malen
atascii-normal=Normal
atascii-normal-tooltip=Normale Zeichen schreiben und auswählen
atascii-mode-antic-tooltip=Der eigene 40-Zeichen-Textbildschirm des Atari, mit Bildschirmfarben
atascii-mode-xep80-tooltip=Der 80-Zeichen-Bildschirm des XEP80: weiß auf schwarz
atascii-xep80-colors=Der XEP80 zeigt weiße Schrift auf Schwarz.
vt52-new-resolution=Auflösung
vt52-new-hint=Jedes Zeichen hat eine Schrift- und eine Hintergrundfarbe aus der Palette der Auflösung. Der Zeichensatz des ST hat keine Block- oder Liniengrafik. Längere Bilder scrollen.
vt52-resolution-low=Niedrig
vt52-resolution-medium=Mittel
vt52-resolution-high=Hoch
vt52-resolution-detail={ $columns } × { $rows } · { $colors } Farben
vt52-colors=Farben
vt52-colors-hint=Klick: Schriftfarbe · Rechtsklick: Hintergrund
vt52-swap-colors=Schrift- und Hintergrundfarbe tauschen
vt52-pipette-hint=Ein Zeichen anklicken, um damit und mit seinen Farben zu malen
vt52-brush-char=Zeichen
vt52-brush-char-tooltip=Das Zeichen in Schrift- und Hintergrundfarbe malen
vt52-brush-color=Farbe
vt52-brush-color-tooltip=Nur die Farben ändern, die Zeichen bleiben
vt52-apply-text=Schrift
vt52-apply-background=Hintergrund
petscii-new-screen=Bildschirm
petscii-new-hint=Der ganze Bildschirm zeigt einen Zeichensatz und eine Hintergrundfarbe; jedes Zeichen hat eine Schriftfarbe. Reverse Zeichen sind die obere Hälfte des Zeichensatzes. Export als SEQ.
petscii-case-upper=Groß/Grafik
petscii-case-lower=Klein/Groß
petscii-case-tooltip=Schaltet den ganzen Bildschirm um, wie der Rechner
petscii-case-vdc-tooltip=Der Zeichensatz für neue Zeichen; der VDC wählt ihn pro Zeichen
petscii-blink=Blinken
petscii-blink-tooltip=Neue Zeichen blinken (VDC-Attribut)
petscii-underline=Unterstreichen
petscii-underline-tooltip=Neue Zeichen sind unterstrichen (VDC-Attribut)
petscii-charset=Zeichensatz
petscii-reverse=Revers
petscii-reverse-tooltip=Reverse Zeichen schreiben und auswählen, wie mit RVS ON
petscii-colors-hint=Klick: Schriftfarbe · Rechtsklick: Bildschirmfarbe
petscii-border=Rahmen { $color }
petscii-screen=Bildschirm { $color }
petscii-border-tooltip=Die Rahmenfarbe: zum Auswählen klicken
petscii-screen-tooltip=Die Bildschirmfarbe: zum Auswählen klicken
petscii-colors-hint-vic20=Klick: Schriftfarbe (die ersten acht) · Rechtsklick: Bildschirmfarbe
petscii-colors-hint-c16=Zeilen sind Helligkeiten, Spalten Farbtöne. Klick: Schriftfarbe · Rechtsklick: Bildschirmfarbe
petscii-machine=Rechner
petscii-monitor=Monitor
petscii-monitor-green=Grün
petscii-monitor-white=Weiß
petscii-monitor-amber=Bernstein
petscii-monitor-hint=Der PET hat keine Farben; sein Monitor zeigt allen Text in einer Farbe.
petscii-brush-color-tooltip=Nur die Schriftfarbe ändern, die Zeichen bleiben
petscii-reverse-pen-tooltip=Zeichen revers machen; die rechte Maustaste macht sie wieder normal
petscii-fade=Abstufen
petscii-fade-tooltip=Mit dichteren Blockzeichen abdunkeln; die rechte Maustaste hellt auf
petscii-corners-square=Eckig
petscii-corners-round=Rund
petscii-corners-round-tooltip=Einfache Linien mit runden Ecken
petmate-no-screen=Der Petmate-Arbeitsbereich enthält keinen Bildschirm.
petmate-unsupported-charset=Petmate-Bildschirme mit dem Zeichensatz „{ $charset }“ lassen sich noch nicht öffnen.
petmate-screen=Bildschirm { $number }
petmate-pick-screen=Der Arbeitsbereich hat { $count } Bildschirme; welcher soll geöffnet werden?
petmate-open=Öffnen
size-preset-standard=Standard
size-preset-vga50=VGA 50 Zeilen
size-preset-wide=Breit
size-preset-wide50=Breit, 50 Zeilen
size-preset-40columns=40 Spalten
start-ansi-subtitle=80 × 25 Leinwand
start-custom=Benutzerdefiniert…
start-custom-subtitle=Typ und Größe wählen
start-tdf-subtitle=TheDraw-Font
start-tagline=ANSI-Art, TheDraw-Fonts, Bitmap-Fonts und Animationen erstellen.
start-new=Neu
start-drop-hint=oder eine Datei irgendwo in dieses Fenster ziehen
start-recent=Zuletzt verwendet
color-switcher-tooltip=Vorder-/Hintergrund – klicken, um eine Palettenfarbe zu wählen
palette-click-hint=Linksklick: Vordergrund · Rechtsklick: Hintergrund
layer-hide=Ebene ausblenden
layer-show=Ebene einblenden
layer-lock=Ebene sperren
layer-unlock=Ebene entsperren
layer-properties-menu=Ebeneneigenschaften…
layer-mode-normal=Normal
layer-mode-chars=Zeichen
layer-mode-attributes=Attribute
status-unknown-font=Unbekannt
status-canvas-size-tooltip=Leinwandgröße in Zeichen
status-selection=Auswahl { $width } × { $height }
status-selection-tooltip=Auswahl: { $left }, { $top } bis { $right }, { $bottom }
status-caret-tooltip=Cursorposition (Spalte, Zeile)
status-font-tooltip=Font: { $font }
    Klicken, um einen anderen Font zu wählen
status-dos-aspect=DOS-Seitenverhältnis
status-square-pixels=Quadratische Pixel
status-dos-aspect-tooltip=Pixel werden wie auf einem 4:3-DOS-Monitor gestreckt.
    Klicken für quadratische Pixel.
status-square-pixels-tooltip=Pixel sind quadratisch.
    Klicken, um sie wie auf einem 4:3-DOS-Monitor zu strecken.
status-9px-font=9-px-Font
status-8px-font=8-px-Font
status-9px-font-tooltip=Zeichen sind 9 Pixel breit (VGA-Zeichenabstand).
    Klicken für 8 Pixel breite Zeichen.
status-8px-font-tooltip=Zeichen sind 8 Pixel breit.
    Klicken, um die 9. Pixelspalte des VGA-Textmodus hinzuzufügen.
status-ice-colors=iCE-Farben
status-blinking=Blinkend
status-ice-colors-tooltip=Das Blink-Bit wählt 8 zusätzliche helle Hintergrundfarben (iCE-Farben).
    Klicken, um stattdessen blinken zu lassen.
status-blinking-tooltip=Das Blink-Bit lässt Zeichen blinken.
    Klicken, um es für 8 helle Hintergrundfarben (iCE-Farben) zu nutzen.
collab-someone=Jemand
collab-connect-title=Mit Server verbinden
collab-connect-subtitle=Einer Moebius-kompatiblen Collaboration-Sitzung beitreten.
collab-server=Server
collab-recent-servers=Zuletzt verwendete Server
collab-hide-password=Passwort verbergen
collab-show-password=Passwort anzeigen
collab-identity=Identität
collab-wrong-password=Falsches Passwort
collab-refused=Der Collaboration-Server hat die Verbindung abgelehnt.
collab-changed-sauce={ $nick } hat die SAUCE-Daten geändert
collab-changed-size={ $nick } hat die Leinwandgröße auf { $columns } × { $rows } geändert
collab-ice-on={ $nick } hat iCE-Farben eingeschaltet
collab-ice-off={ $nick } hat iCE-Farben ausgeschaltet
collab-spacing-on={ $nick } hat den 9-Pixel-Zeichenabstand eingeschaltet
collab-spacing-off={ $nick } hat den 9-Pixel-Zeichenabstand ausgeschaltet
collab-changed-font={ $nick } hat den Font auf { $font } geändert
collab-changed-background={ $nick } hat den Hintergrund geändert
collab-jump-to-user=Klicken, um zu dessen Cursor zu springen
collab-user-count={ $count ->
    [one] { $count } Benutzer
   *[other] { $count } Benutzer
}
collab-status-tooltip=Collaboration-Sitzung – klicken, um den Chat ein- oder auszublenden
collab-connecting=Verbinde mit { $server }…
error-font-psf-extension=Bitmap-Fonts bitte mit der Endung .psf speichern.
font-editor-pixels=Pixel
font-editor-select-pixels=Pixel auswählen
font-editor-copy-glyph=Zeichen kopieren
font-editor-discard-question=Ungespeicherte Font-Änderungen verwerfen?
font-editor-keep-editing=Weiter bearbeiten
font-editor-save-font=Font speichern…
font-editor-apply=Auf Dokument anwenden
animation-format-gif=GIF-Animation
animation-format-cast=Asciicast v2
animation-export-extension=Bitte einen anderen Dateinamen mit der Endung .{ $extension } wählen.
animation-compile=Kompilieren
animation-previous-frame=Vorheriges Bild
animation-next-frame=Nächstes Bild
animation-play-pause=Abspielen / Pause
animation-loop=Schleife
animation-loop-tooltip=Nach dem letzten Bild wieder beim ersten beginnen
animation-speed=Wiedergabegeschwindigkeit
animation-frame=Bild
animation-format-av1=AV1-Video (IVF)
animation-first-frame=Erstes Bild
animation-last-frame=Letztes Bild
animation-restart=Neu starten
animation-cursor=Z. { $line }, Sp. { $column }
animation-log=Log
animation-toggle-log=Log ein- oder ausblenden
animation-modified=Geändert
animation-export-browse=Durchsuchen…
animation-export-summary={ $frames } Frames · { $width }×{ $height } Zeichen · { $duration }
ice-mode-unlimited=Unbegrenzt
font-import-preview=Vorschau
font-selector-filter-placeholder=Fonts filtern…
font-selector-no-fonts-match=Kein Font entspricht dem Filter
font-selector-no-selection=Keine Schrift ausgewählt
font-selector-sauce-fonts=SAUCE-Fonts
font-selector-ansi-fonts=ANSI-Fonts

# Bitmap font editor
font-editor-size = Größe
font-editor-preview-tooltip = Einen Beispieltext in der bearbeiteten Schrift zeigen
font-editor-apply-tooltip = Die bearbeitete Schrift in der Zeichnung verwenden
font-editor-close-tooltip = Zur Zeichnung zurückkehren, ohne die Schrift zu übernehmen
font-editor-status-char = Zeichen 0x{ $code } { $char }
font-editor-status-size = Glyphengröße in Pixeln
font-editor-hints = Strg+Pfeile verschieben  ·  Alt+Pfeile Zeile/Spalte einfügen/löschen  ·  +/- nächstes/vorheriges Zeichen  ·  Tab wechselt den Bereich
font-editor-character-set = Zeichensatz
outline-code-fill=Füllmarkierung (@)
outline-code-end=Endmarkierung (&)
outline-code-hole=Outline-Loch (Leerzeichen)
outline-code-placeholder=Outline-Platzhalter { $code }
outline-styles-title=Outline-Stil
status-font-slot-tooltip=Font-Slot { $slot }: { $font }
font-slots-title=Dokument-Fonts
font-slots-predefined=Vordefinierte Fonts
font-slots-custom=Dokument-Fonts
font-slots-active=Aktiver Slot: { $slot }
font-slots-add=Font hinzufügen…
font-slots-replace=Aktiven Slot ersetzen…
font-slots-replace-tip=Ändert den Font in bestehender Grafik, die diesen Slot verwendet
font-slots-full=Alle 256 Font-Slots sind belegt.
    Klicken, um mit diesem Font zu zeichnen, Doppelklick, um einen anderen Font zu wählen
sauce-subtitle=Metadaten am Dateiende, die Viewer und BBS-Software anzeigen.
sauce-record=Datensatz
sauce-comment-too-long=Zeile { $line } ist länger als { $limit } Zeichen und wird gekürzt.
sauce-display=Darstellung
shortcuts-dialog-subtitle=Schnellreferenz für Icy Draw
recovery-title=Nicht gespeicherte Arbeit wiederherstellen
recovery-description=Icy Draw wurde unerwartet beendet, während diese Dokumente nicht gespeicherte Änderungen hatten.
recovery-restore=Wiederherstellen
recovery-restore-all=Alle wiederherstellen
recovery-discard=Verwerfen…
recovery-discard-confirm=Endgültig verwerfen
recovery-keep=Behalten
recovery-later=Später entscheiden
recovery-saved-at=Automatisch gesichert { $time }
recovery-kind-ansi=Zeichnung
recovery-kind-charfont=TheDraw-Schrift
recovery-kind-bitfont=Bitmap-Schrift
recovery-kind-animation=Animation
recovery-disk-changed=Die Datei wurde seitdem geändert. Beim Speichern wird vor dem Ersetzen nachgefragt.
recovery-disk-missing=Die Datei existiert nicht mehr.
recovery-damaged=Diese Wiederherstellungsdatei kann nicht gelesen werden: { $error }
recovery-new-window=Wird in einem neuen Fenster wiederhergestellt, da dieses Fenster nicht gespeicherte Änderungen hat.
recovery-unavailable=Automatisches Sichern ist nicht verfügbar, nicht gespeicherte Änderungen können nach einem Absturz nicht wiederhergestellt werden: { $error }
recovery-no-directory=Es wurde kein Datenverzeichnis gefunden
recovery-failed=Automatisches Sichern ist fehlgeschlagen. Bis das behoben ist, können nicht gespeicherte Änderungen nach einem Absturz nicht wiederhergestellt werden: { $error }
recovery-restore-failed=Das Dokument konnte nicht wiederhergestellt werden. Die Wiederherstellungsdatei bleibt erhalten: { $error }
shade-characters-tooltip=Zeichen, die der Schattierungspinsel von hell nach dunkel durchläuft. Rechtsklick hellt auf.
shade-keep-characters=Zeichen behalten
shade-colors-tooltip=Vordergrundfarben, die der Schattierungspinsel durchläuft, ein Schritt pro Strich. Nur aktiv, solange „Vordergrund“ eingeschaltet ist.
shade-brush-color=Pinselfarbe
shade-edit-ramps=Verläufe bearbeiten…
shade-ramps-title=Schattierungsverläufe
shade-ramps-subtitle=Jeder Strich des Schattierungspinsels bewegt eine Zelle einen Schritt im Zeichenverlauf und einen Schritt im Farbverlauf weiter.
shade-character-ramps=Zeichenverläufe
shade-character-ramp-hint=Zeichen von hell nach dunkel, z. B. ░▒▓█
shade-color-ramps=Farbverläufe
shade-add-character-ramp=Zeichenverlauf hinzufügen
shade-add-color-ramp=Farbverlauf hinzufügen
shade-remove-ramp=Entfernen
shade-add-color=Eine Palettenfarbe hinzufügen
shade-remove-color=Farbe { $color } – zum Entfernen klicken
shade-ramp-empty=Leerer Verlauf
shade-invalid-character=„{ $character }“ ist kein verwendbares CP437-Zeichen
rip-editor-title = RIP-Zeichnung
rip-editor-description = RIPscrip-Vektorgrafik bearbeiten
igs-editor-title = IGS-Zeichnung
igs-editor-description = Atari-ST-Grafiken im Instant-Graphics-and-Sound-Format bearbeiten
igs-new-resolution = Auflösung
igs-editor-commands = Befehle
igs-editor-filter = Befehle filtern
igs-editor-delete = Löschen
igs-editor-up = Nach oben
igs-editor-down = Nach unten
igs-editor-preview-through = Vorschau bis zum gewählten Befehl
igs-editor-vertex = Punkt { $index }
igs-editor-vertices = { $count } Punkte
igs-editor-add-vertex = Punkt hinzufügen
igs-editor-remove-vertex = Punkt entfernen
igs-editor-source = IGS-Quelltext
igs-editor-apply-source = Übernehmen
igs-editor-revert-source = Zurücksetzen
igs-editor-source-only = Dieser Befehl wird als IGS-Quelltext bearbeitet.
igs-editor-loop-source = Schleifen werden als IGS-Quelltext bearbeitet.
igs-editor-invalid-escape = Der Quelltext enthält eine ungültige Escape-Folge; erlaubt sind \xNN, \r, \n, \e und \\.
igs-editor-variable-parameter = Ein Zufallswert (r, R) oder Schleifenwert (x, y), im IGS-Quelltext änderbar
igs-tool-select = Auswählen
igs-tool-marker = Polymarker setzen
igs-tool-line = Linie
igs-tool-polyline = Linienzug
igs-tool-rectangle = Rechteck
igs-tool-rounded-rectangle = Abgerundetes Rechteck
igs-tool-filled-rectangle = Gefülltes Rechteck
igs-tool-circle = Kreis
igs-tool-ellipse = Ellipse
igs-tool-arc = Bogen
igs-tool-elliptical-arc = Ellipsenbogen
igs-tool-pie-slice = Kreissegment
igs-tool-elliptical-pie-slice = Ellipsensegment
igs-tool-polygon = Polygon
igs-tool-flood-fill = Füllen
igs-tool-text = Text
igs-command-color = Farbe
igs-command-fill = Füllstil
igs-command-line-style = Linienstil
igs-command-marker-style = Markierungsstil
igs-command-pen-color = Stiftfarbe
igs-command-drawing-mode = Zeichenmodus
igs-command-hollow = Hohl
igs-command-text-effects = Textstil
igs-command-resolution = Auflösung
igs-command-clear = Bildschirm löschen
igs-command-loop = Schleife
igs-command-pause = Pause
igs-command-draw-to = Linie zu
igs-command-text = VT52-Text
igs-command-invalid = Nicht gelesen
igs-command-initialize = Initialisieren
igs-command-scaling = Grafikskalierung
igs-command-restore-sound = Klang zurücksetzen
igs-command-sound-buffer = Soundpuffer
igs-command-midi-buffer = MIDI-Puffer
igs-command-ask = IG abfragen
igs-command-right-mouse = Makro rechte Maustaste
igs-command-left-mouse = Linke Maustaste
igs-command-flow-control = Flusskontrolle
igs-command-delete-lines = Zeilen löschen
igs-command-insert-lines = Zeilen einfügen
igs-command-clear-line = Zeile leeren
igs-command-move-cursor = Cursor bewegen
igs-command-remember-cursor = Cursor merken
igs-command-line-wrap = Zeilenumbruch
igs-init-palette-attributes = Palette und Attribute
igs-init-palette = Palette
igs-init-attributes = Attribute
igs-init-ig-palette = IG-Palette
igs-init-vdi-palette = VDI-Palette
igs-init-resolution = Auflösung und Clipping
igs-scaling-virtual = Virtuell 10000 × 10000
igs-scaling-monochrome = Monochrome Proportionen
igs-pen-color-summary = { $pen } · R{ $red } G{ $green } B{ $blue }
igs-pen = Stift { $pen }
igs-pen-line = Linie
igs-pen-fill = Füllung
igs-pen-text = Text
igs-pen-marker = Polymarker
igs-pen-marker-short = Marker
igs-pen-kind = Stift
igs-pen-number = Stift
igs-color = Farbe
igs-fill = Füllung
igs-fill-pattern-label = Füllmuster
igs-fill-border = Rand
igs-fill-hollow = Hohl
igs-hollows = Hohl zeichnen
igs-hollows-tooltip = IGs H-Befehl: Gefüllte Formen werden als Umriss gezeichnet, mit hohler Füllung, Rand und transparentem Zeichenmodus. Aus stellt wieder volle Füllung im Ersetzen-Modus ein.
igs-fill-solid = Voll
igs-fill-pattern = Muster { $index }
igs-fill-hatch = Schraffur { $index }
igs-fill-user = Eigenes Muster { $index }
igs-fill-random = Zufall
igs-fill-kind-pattern = Muster
igs-fill-kind-hatch = Schraffur
igs-fill-kind-user = Eigenes Muster
igs-filled = Gefüllt
igs-rounded = Abgerundete Ecken
igs-line = Linie
igs-line-solid = Durchgezogen
igs-line-long-dash = Lang gestrichelt
igs-line-dotted = Gepunktet
igs-line-dash-dot = Strichpunkt
igs-line-dashed = Gestrichelt
igs-line-dash-dot-dot = Strich-Punkt-Punkt
igs-line-user = Benutzerdefiniert
igs-line-type = Linientyp
igs-line-pattern = Linienmuster
igs-line-only-solid-wide = Nur durchgezogene Linien können breit sein
igs-thickness = Stärke
igs-end-start = Anfang
igs-end-end = Ende
igs-end-square = Eckig
igs-end-arrow = Pfeil
igs-end-rounded = Rund
igs-marker = Polymarker
igs-marker-point = Punkt
igs-marker-plus = Plus
igs-marker-star = Stern
igs-marker-square = Quadrat
igs-marker-cross = Kreuz
igs-marker-diamond = Raute
igs-marker-size-up = Größer
igs-marker-size-down = Kleiner
igs-marker-point-size = Ein Punkt ist immer ein Pixel groß
igs-size = Größe
igs-size-prefix = Größe{" "}
igs-drawing-mode = Zeichenmodus
igs-mode-replace = Ersetzen
igs-mode-transparent = Transparent
igs-mode-xor = XOR
igs-mode-reverse-transparent = Umgekehrt transparent
igs-text-bold = Fett
igs-text-light = Hell
igs-text-italic = Kursiv
igs-text-underlined = Unterstrichen
igs-text-outlined = Umrandet
igs-text-rotation = Drehung
igs-text-size-summary = Größe { $size }
igs-text-hint = Klicke in die Zeichnung zum Schreiben oder auf Text zum Bearbeiten
igs-text-typing-hint = Tippen, dann Enter drücken oder woanders klicken
igs-poly-hint = Jeden Punkt anklicken; Rechtsklick oder Enter beendet
igs-shift-angle-hint = Umschalt: 45°-Schritte
igs-shift-square-hint = Umschalt: Quadrate und Kreise
igs-select-hint = Klicke eine Form an, um sie auszuwählen
igs-start-angle = Anfang
igs-end-angle = Ende
igs-on = an
igs-off = aus
igs-resolution = Auflösung
igs-resolution-low = Niedrig · 320 × 200 · 16 Farben
igs-resolution-medium = Mittel · 640 × 200 · 4 Farben
igs-resolution-high = Hoch · 640 × 400 · 2 Farben
igs-palette-mode = Palette
igs-clear-mode = Modus
igs-pause-seconds = Sekunden
igs-pause-vsyncs = Bildwechsel
igs-palette-edit = IGS-Palette bearbeiten…
igs-palette-short = Palette…
igs-palette-edit-tooltip = Farben der Stifte mit S-Befehlen setzen
igs-palette-title = IGS-Palette
igs-palette-subtitle = Farben der Stifte in den acht Atari-ST-Stufen je Kanal
igs-palette-pens = Stifte
igs-palette-red = Rot
igs-palette-green = Grün
igs-palette-blue = Blau
igs-palette-revert = Zurücksetzen
igs-tool-copy-area = Bereich kopieren
igs-tool-zone = Mauszone
igs-blit-and = Und
igs-blit-invert = Quelle invertieren
igs-blit-clear = Löschen
igs-blit-and-not = Und nicht Ziel
igs-blit-erase = Radieren
igs-blit-unchanged = Ziel unverändert
igs-blit-nor = Nicht oder
igs-blit-xnor = Nicht XOR
igs-blit-invert-destination = Ziel invertieren
igs-blit-or-not = Oder nicht Ziel
igs-blit-nand = Nicht und
igs-blit-fill = Füllen
igs-blit-mode = Kopiermodus
igs-copy-hint = Den zu kopierenden Bereich aufziehen
igs-copy-place-hint = Klicke, wohin die Kopie soll; Rechtsklick wählt einen anderen Bereich
igs-zone-id = Zone
igs-zone-host = Host-Text
igs-zone-host-tooltip = Wird beim Klick auf die Zone an den Host gesendet
igs-zone-hint = Auf freier Fläche ziehen fügt eine Zone hinzu; eine Zone ziehen verschiebt sie
igs-zone-host-required = Zuerst den Host-Text der Zone eingeben
igs-command-fill-pattern = Füllmuster
igs-command-sound = Sound
igs-command-chip-music = Chipmusik
igs-command-stop-sound = Sound stoppen
igs-command-clear-zones = Zonen löschen
igs-pattern-slot-summary = Platz { $slot }
igs-pattern-edit = Füllmuster…
igs-pattern-draw-user = Eigenes Muster zeichnen…
igs-pattern-edit-tooltip = Eines der acht eigenen Füllmuster zeichnen (X 7)
igs-pattern-title = Füllmuster
igs-pattern-subtitle = Ein 16 × 16-Muster für Füllungen mit eigenem Muster
igs-pattern-slot = Platz
igs-pattern-preview = Gekachelt
igs-pattern-shift = Verschieben
igs-pattern-shift-left = Nach links verschieben
igs-pattern-shift-right = Nach rechts verschieben
igs-pattern-shift-up = Nach oben verschieben
igs-pattern-shift-down = Nach unten verschieben
igs-pattern-clear = Leeren
igs-pattern-invert = Invertieren
igs-sound-play = Sound abspielen
igs-sound-stop = Sound stoppen
playback-animation = Animation
playback-play-pause = Animation abspielen oder pausieren
playback-previous = Vorheriger Befehl
playback-next = Nächster Befehl
playback-stop = Stopp
playback-first = Erster Befehl
playback-last = Letzter Befehl
playback-stop-tooltip = Anhalten und die ganze Zeichnung zeigen
playback-position = { $current } / { $total }
playback-seek = Ziehen, um zu einem Befehl zu springen
playback-speed = Geschwindigkeit
playback-speed-max = Max
igs-group-attributes = Zeichenattribute
igs-group-drawing = Zeichnen
igs-group-screen = Bildschirm
igs-group-colors = Farbregister
igs-group-flow = Schleifen und Pausen
igs-group-text = VT52-Text und Cursor
igs-group-sound = Sound
igs-group-interaction = Eingabe und Mauszonen
igs-template-pen-color = Stiftfarbe setzen
igs-template-pen-palette = Palettenfarbe eines Stifts setzen
igs-template-drawing-mode = Zeichenmodus setzen
igs-template-line-style = Linientyp setzen
igs-template-marker-style = Markierungstyp setzen
igs-template-fill = Füllmuster setzen
igs-template-hollow = Hohl setzen
igs-template-text-effects = Texteffekte setzen
igs-template-loop = Schleife
igs-template-pause-seconds = Pause (Sekunden)
igs-template-pause-vsync = Pause (Bildwechsel)
igs-template-sound = Soundeffekt
igs-template-chip-music = Chipmusik
igs-template-effect-loops = Soundeffekt-Wiederholungen
igs-template-stop-sound = Alle Sounds stoppen
igs-template-clear = Bildschirm löschen
igs-template-initialize = Initialisieren
igs-template-resolution = Auflösung setzen
igs-template-random-range = Zufallsbereich
igs-template-draw-to-start = Startpunkt für Linie zu
igs-template-draw-to = Linie zu
igs-template-clear-zones = Mauszonen löschen
igs-template-cursor-off = Cursor aus
igs-template-position-cursor = Cursor positionieren
igs-template-text-color = VT52-Textfarbe
igs-template-text = VT52-Text
igs-tool-spray = Sprühen
igs-spray-density = Anzahl der Markierungen im Bereich
igs-spray-density-prefix = Dichte{" "}
igs-spray-rotation = Farben der Sprühmarkierungen durchwechseln
igs-spray-rotation-pen = Ab Stift
igs-spray-rotation-summary = ab Stift { $pen }
igs-rotate-start = Erstes Register
igs-rotate-end = Letztes Register
igs-rotate-count = Verschiebungen
igs-rotate-delay = Verzögerung (1/200 s)
igs-rotate-hint = Ist das erste Register kleiner als das letzte, wandern die Farben nach rechts, sonst nach links. 0 Verschiebungen stellen die Farben von vor der Rotation wieder her.
igs-rotate-reset = wiederherstellen
igs-color-register = Register
igs-chip-voice = Stimme
igs-chip-volume = Lautstärke
igs-chip-pitch = Tonhöhe
igs-chip-timing = Dauer (1/200 s)
igs-chip-stop = Danach
igs-chip-stop-none = Weiterspielen
igs-chip-stop-release = Stimme ausklingen lassen
igs-chip-stop-voice = Stimme stoppen
igs-chip-stop-release-all = Alle Stimmen ausklingen lassen
igs-chip-stop-all = Alle Stimmen stoppen
igs-effect-loops = Wiederholungen der Effekte 0–4
igs-cursor = Cursor
igs-cursor-off = Unsichtbar
igs-cursor-on = Sichtbar
igs-cursor-destructive = Löschender Rückschritt
igs-cursor-non-destructive = Nicht löschender Rückschritt
igs-inverse-video = VT52 inverser Text
igs-text-layer = Ebene
igs-text-foreground = Vordergrund
igs-text-background = Hintergrund
igs-input-return = Nach der Eingabe Wagenrücklauf senden
igs-input-kind = Wartet auf
igs-input-output = Eingabe
igs-input-show = Anzeigen und senden
igs-input-hide = Verbergen und senden
igs-input-show-discard = Anzeigen, nicht senden
igs-input-hide-discard = Verbergen, nicht senden
igs-input-key = Eine Taste
igs-input-line = Eine Textzeile
igs-input-zones-marker = Klick in Mauszone, Marker als Zeiger
igs-input-zones = Klick in Mauszone, { $pointer }
igs-pointer-arrow = Pfeil
igs-pointer-hourglass = Sanduhr
igs-pointer-bee = Biene
igs-pointer-finger = Zeigefinger
igs-pointer-hand = flache Hand
igs-pointer-thin-cross = dünnes Fadenkreuz
igs-pointer-thick-cross = dickes Fadenkreuz
igs-pointer-outlined-cross = umrandetes Fadenkreuz
igs-command-spray-rotation = Sprühfarben-Rotation
igs-command-color-rotation = Farbrotation
igs-command-color-register = Farbregister
igs-command-color-registers = Farbregister
igs-command-input = Benutzereingabe
igs-command-blit-memory = BitBlit-Speicher
igs-template-inverse-text = VT52 inverser Text
igs-template-input = Eingabe vom Benutzer
igs-template-color-register = Farbregister setzen
igs-template-color-rotation = Farbregister rotieren
igs-template-color-rotation-reset = Rotierte Farben wiederherstellen
igs-template-spray-rotation = Farbrotation beim Sprühen
igs-template-wipe-blit = BitBlit-Speicher löschen
igs-editor-add = Befehl hinzufügen
igs-editor-add-hint = Wird hinter der Auswahl eingefügt, während die Vorschau bis dorthin läuft, sonst am Ende
igs-editor-duplicate = Duplizieren (Strg+D)
igs-resolution-tooltip = Die Auflösung, mit der die Zeichnung beginnt, gesetzt durch ihren ersten R-Befehl
igs-resolution-mid-warning = Dies ändert die Auflösung nach Zeichenbefehlen; spätere Koordinaten beziehen sich auf die neue Fläche.
rip-editor-commands = Befehle
rip-editor-preview-through = Bis zum gewählten Befehl anzeigen
rip-editor-properties = Befehlsparameter
rip-editor-unsupported-properties = Parameter dieses Befehls sind noch nicht bearbeitbar.
rip-command-color = Farbe
rip-command-line-style = Linienstil
rip-command-fill-style = Füllmuster
rip-command-font-style = Schrift
rip-command-palette = Palette
rip-command-palette-slot = Palettenfarbe
rip-editor-pixel = Pixel
rip-editor-line = Linie
rip-editor-rectangle = Rechteck
rip-editor-bar = Gefülltes Rechteck
rip-editor-circle = Kreis
rip-editor-oval = Gefüllte Ellipse
rip-editor-outline-oval = Ellipsenumriss
rip-editor-text = Text
rip-editor-bezier = Bézierkurve
rip-editor-polygon = Polygon
rip-editor-filled-polygon = Gefülltes Polygon
rip-editor-polyline = Polylinie
rip-editor-arc = Kreisbogen
rip-editor-oval-arc = Ellipsenbogen
rip-editor-pie-slice = Kreisausschnitt
rip-editor-oval-pie-slice = Ellipsenausschnitt
rip-editor-vertex = Eckpunkt
rip-editor-vertices = { $count } Eckpunkte
rip-editor-add-vertex = Eckpunkt hinzufügen
rip-editor-remove-vertex = Letzten Eckpunkt entfernen
rip-editor-start-angle = Start
rip-editor-end-angle = Ende
rip-poly-hint = Eckpunkte anklicken; Rechtsklick oder Eingabe beendet, Esc verwirft.
rip-palette-title = RIP-Palette
rip-palette-subtitle = Jedem der 16 Zeichenfarbplätze eine von 64 EGA-Farben zuweisen.
rip-palette-slots = Farbplätze
rip-palette-slot-tooltip = Platz { $slot } · EGA { $color }
rip-palette-ega = EGA-Farbe für Platz { $slot }
rip-palette-default = Standard wiederherstellen
rip-palette-edit = RIP-Palette bearbeiten…
rip-palette-edit-tooltip = Den 16 RIP-Farbplätzen EGA-Farben zuweisen.
rip-editor-button = Schaltfläche
rip-editor-mouse = Mauszone
rip-command-mouse-fields = Mauszonen löschen
rip-mouse-invert = Beim Klick invertieren
rip-mouse-clear = Textfenster leeren
rip-mouse-host-tooltip = Wird beim Klick an den Host gesendet, z. B. M^m für die Taste M und Enter
rip-mouse-hint = Ziehen fügt eine Zone hinzu; eine Zone anklicken zum Auswählen, Verschieben oder Ändern der Größe.
rip-mouse-no-command = (kein Host-Befehl)
rip-editor-label = Beschriftung
rip-editor-label-required = Vor dem Platzieren einen Text eingeben.
rip-editor-button-size-required = Die Schaltfläche auf eine Größe ziehen.
rip-editor-button-label-delimiter = Beschriftungen dürfen <> nicht enthalten.
rip-editor-color = Zeichen
rip-editor-border-color = Rand
rip-editor-fill-color = Füllung
rip-editor-delete = Befehl löschen
rip-editor-up = Befehl nach oben
rip-editor-down = Befehl nach unten
rip-editor-preserved = Gemischte RIP/ANSI-Befehle bleiben erhalten; neue Befehle unten hinzufügen.
start-connect-tooltip=Einem Moebius-kompatiblen Kollaborationsserver beitreten und gemeinsam zeichnen
rip-bezier-segments = Segmente
rip-bezier-adjust-hint = Anfasser ziehen; Rechtsklick oder Enter schließt ab, Esc bricht ab.
rip-line-solid = Durchgehend
rip-line-dotted = Gepunktet
rip-line-center = Mittellinie
rip-line-dashed = Gestrichelt
rip-line-user = Eigenes
rip-line-thin = 1 px
rip-line-thin-tooltip = Dünne Linien
rip-line-thick = 3 px
rip-line-thick-tooltip = Dicke Linien
rip-fill-pattern = Füllung
rip-fill-empty = Leer
rip-fill-solid = Voll
rip-fill-line = Linien
rip-fill-light-slash = Leichte Schrägstriche
rip-fill-slash = Schrägstriche
rip-fill-backslash = Rückstriche
rip-fill-light-backslash = Leichte Rückstriche
rip-fill-hatch = Schraffur
rip-fill-cross-hatch = Kreuzschraffur
rip-fill-interleave = Verschränkt
rip-fill-wide-dots = Weite Punkte
rip-fill-close-dots = Enge Punkte
rip-fill-user = Eigenes
rip-font = Schrift
rip-font-default = Standard (8 × 8)
rip-font-size = Schriftgröße
rip-font-size-prefix = Größe{" "}
rip-text-horizontal = Horizontal
rip-text-direction = Richtung
rip-text-hint = In die Zeichnung klicken und Text tippen, oder vorhandenen Text anklicken, um ihn zu ändern.
rip-text-typing-hint = Tippen; Eingabe oder ein Klick daneben übernimmt, Esc verwirft.
rip-text-vertical = Vertikal
rip-button-plain = Einfach
rip-button-plain-tooltip = Ein abgeschrägter Button mit Textbeschriftung
rip-button-icon = Icon
rip-button-icon-tooltip = Ein Button mit einer Icon-Datei (.ICN) vom Host
rip-button-clipboard = Zwischenablage
rip-button-clipboard-tooltip = Ein Button mit dem Bild aus der RIP-Zwischenablage
rip-button-icon-required = Icon-Datei des Icon-Buttons eingeben.
rip-button-host-command = Host-Befehl
rip-button-host-command-hint = wird beim Klick gesendet, z. B. ^mMAIN^m
rip-button-icon-file = Icon-Datei
rip-button-hotkey = Tastenkürzel
rip-button-group-number = Gruppe
rip-button-style-open = Button-Stil…
rip-button-style-tooltip = Alle Optionen neuer Buttons: Farben, Schrift, Effekte und Verhalten
rip-button-style-title = Button-Stil
rip-button-style-entry = Button-Stil
rip-button-edit = Button bearbeiten…
rip-button-edit-hint = Button und Stil werden gemeinsam bearbeitet.
rip-button-sample = Beispiel
rip-button-type = Art
rip-button-group-button = Button
rip-button-group-colors = Farben
rip-button-group-layout = Beschriftung und Größe
rip-button-group-effects = Effekte
rip-button-group-behavior = Verhalten
rip-button-label-color = Beschriftung
rip-button-shadow-color = Schatten
rip-button-bright-color = Hell
rip-button-dark-color = Dunkel
rip-button-surface-color = Fläche
rip-button-underline-color = Unterstrich
rip-button-corner-color = Ecke
rip-button-orientation = Beschriftung
rip-button-above = Oben
rip-button-left = Links
rip-button-center = Mitte
rip-button-right = Rechts
rip-button-below = Unten
rip-button-justify = Ausrichtung
rip-button-bevel-size = Abschrägung
rip-button-size = Größe bei Klick
rip-button-size-tooltip = Gilt, wenn der Button per Klick statt Ziehen gesetzt wird; 0 nutzt die Icon- oder Zwischenablagegröße.
rip-button-bevel = Abschrägung
rip-button-chisel = Meißel
rip-button-sunken = Vertieft
rip-button-recessed = Eingelassen
rip-button-shadow = Beschriftung mit Schatten
rip-button-underline-hotkey = Tastenkürzel unterstreichen
rip-button-highlight-hotkey = Tastenkürzel hervorheben
rip-button-center-vertically = Beschriftung vertikal zentrieren
rip-button-hot-icons = Hot-Icons
rip-button-explode = Beim Klick aufklappen
rip-button-mouse = Maus-Button
rip-button-invert = Beim Klick invertieren
rip-button-reset = Bildschirm nach Klick zurücksetzen
rip-button-radio = Optionsgruppe
rip-button-checkbox = Kontrollkästchen
rip-button-stamp = Bild in Zwischenablage stempeln
rip-button-selected = Ausgewählt
rip-button-group-appearance = Aussehen
rip-editor-select = Auswählen
rip-select-hint = Form anklicken zum Auswählen, ziehen zum Verschieben, Anfasser ziehen zum Ändern der Größe. Pfeiltasten verschieben, Entf löscht.
rip-button-create = Button…
rip-button-create-tooltip = Art, Beschriftung und Stil eines neuen Buttons wählen und ihn dann in der Zeichnung platzieren
rip-button-create-title = Button erstellen
rip-button-place = Platzieren
rip-button-place-hint = Klicken setzt den Button mit { $width } × { $height }, Ziehen bestimmt die Größe; Esc bricht ab.

# Attribute picker (Escape without a selection)
attribute-picker-pair = Vordergrund { $foreground } · Hintergrund { $background }
attribute-picker-foreground = Vordergrund { $color }
attribute-picker-background = Hintergrund { $color }
attribute-picker-keys = ↑↓ Vordergrund · ←→ Hintergrund · Enter oder Esc schließt

# Line tool: outline mode (box-drawing lines)
line-style-outline = Rahmen
line-style-outline-tooltip = Rahmenlinien zeichnen, die sich mit vorhandenen Linien verbinden
line-style-single-tooltip = Einfache Rahmenlinien, die sich mit vorhandenen Linien verbinden
line-style-double-tooltip = Doppelte Rahmenlinien, die sich mit vorhandenen Linien verbinden
line-style-double-horizontal-tooltip = Waagerecht doppelt, senkrecht einfach – verbindet sich mit vorhandenen Linien
line-style-double-vertical-tooltip = Waagerecht einfach, senkrecht doppelt – verbindet sich mit vorhandenen Linien
igs-tune-open = Chip-Tune-Editor…
igs-tune-open-tooltip = Eine Melodie aus Chip-Musik-Noten (n) spielen, aufnehmen und bearbeiten, wie IGs Tap A Tune
igs-tune-edit = Melodie bearbeiten…
igs-tune-edit-tooltip = Öffnet die Chip-Musik-Noten um diese herum im Melodie-Editor
igs-tune-title = Chip-Tune
igs-tune-subtitle = Noten auf den drei Stimmen des Soundchips, geschrieben als n-Befehle
igs-tune-play = ▶ Abspielen
igs-tune-stop = ■ Stopp
igs-tune-play-tooltip = Spielt ab der Abspielposition (Leertaste oder F2); ein Klick auf die Zeitleiste setzt die Position
igs-tune-record = Aufnehmen
igs-tune-record-tooltip = Nimmt die auf der Tastatur gespielten Noten an der Abspielposition auf, während die Melodie läuft (F1)
igs-tune-recording = Aufnahme
igs-tune-practice = Übungsmodus
igs-tune-snap = Raster
igs-tune-snap-off = Aus
igs-tune-zoom = Zoom
igs-tune-tempo = Tempo
igs-tune-tempo-apply = Tempo anwenden
igs-tune-tempo-tooltip = Macht die ganze Melodie schneller oder langsamer, wie IGs Aufnahmegeschwindigkeit
igs-tune-clear = Melodie löschen
igs-tune-clear-tooltip = Entfernt alle Noten (F3)
igs-tune-instrument = Instrument
igs-tune-length = Länge
igs-tune-length-tooltip = Wie lange neue Noten klingen, in 1/200 s (← −10, → +10)
igs-tune-fixed-length = Feste Länge
igs-tune-fixed-length-tooltip = Aufgenommene Noten bekommen die Länge oben statt der Dauer des Tastendrucks, wie IGs Note Release Time
igs-tune-end = Ende
igs-tune-octave = Oktave
igs-tune-voice = Stimme
igs-tune-voice-tooltip = Neue und aufgenommene Noten kommen auf diese Stimme
igs-tune-voice-on = An
igs-tune-voice-on-tooltip = Diese Stimme abspielen und aufnehmen
igs-tune-along = Mit
igs-tune-along-tooltip = Spielt bei jeder Taste auf dieser Stimme mit, um die Halbtöne versetzt, wie IGs Multi Voice
igs-tune-start = Start
igs-tune-selection-hint = Klicken fügt eine Note hinzu, Ziehen verschiebt sie, Ziehen am rechten Ende verlängert sie, Rechtsklick löscht sie
igs-tune-keys-hint = Tasten: untere Reihe Y S X D C V … und obere Reihe Q 2 W 3 E … spielen zwei Oktaven · Bild↑/Bild↓ Oktave, Pos1 Standard · ← → Länge · ↑ Ende · ↓ Lautstärke −1 (Umschalt +1) · Leertaste Abspielen · F1 Aufnahme · F3 Löschen · Entf Note löschen
