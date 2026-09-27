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
new-kind-animation-description=Lua-gesteuerte ANSI-Animation
new-kind-bitfont-description=8 × 16 Pixel Konsolenschrift
new-kind-color_font-description=Zeichen mit Farben
new-kind-block_font-description=Blockzeichen, eine Farbe
new-kind-outline_font-description=Outline-Platzhalter
file-dialog-filter-images=Bilder
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
tag-new=Neuer Tag
tag-edit=Tag bearbeiten
tag-delete-selected=Ausgewählte Tags löschen
tag-place-hint=Klicken, um einen Tag zu platzieren, ziehen, um ihn zu verschieben
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
    Klicken, um mit diesem Font zu zeichnen, Doppelklick, um einen anderen Font zu wählen
sauce-subtitle=Metadaten am Dateiende, die Viewer und BBS-Software anzeigen.
sauce-record=Datensatz
sauce-comment-too-long=Zeile { $line } ist länger als { $limit } Zeichen und wird gekürzt.
sauce-display=Darstellung
