font-editor-table = Karakter táblázat 0-{ $length }:

unsaved-title=Nevezetlen

# CLI (clap)
app-about = Icy Draw — create ANSI/ASCII art, edit fonts, and collaborate in real time
arg-path-help = File to open on startup
arg-mcp-port-help = Start an MCP server on the given port (e.g. --mcp-port 8080)
cmd-host-about = Host a real-time collaboration session (Moebius-compatible)
arg-host-port-help = Port to listen on (default: 8000)
arg-host-bind-help = Bind address (default: 0.0.0.0)
arg-host-password-help = Session password (optional)
arg-host-max-users-help = Maximum number of users (0 = unlimited)
arg-host-file-help = File to host (optional; starts with an empty 80x25 canvas if omitted)
arg-host-backup-folder-help = Folder for autosave backups (enables autosave if set)
arg-host-interval-help = Autosave interval in minutes (default: 60)

# Server banner
server-title = Icy Draw Collaboration Server
server-bind-address = Bind Address
server-password = Password
server-document = Document
server-max-users = Max Users
server-connect-with = Connect with
server-stop-hint = Press Ctrl+C to stop the server
server-none = (none)
server-unlimited = unlimited

menu-file=&Fájl
menu-new=Új…
menu-open=Megnyitás…
menu-open_recent=Legutóbbi megnyitása
menu-open_recent_clear=Törlés
menu-no_recent_files=Nincs legutóbbi fájl
menu-clear_recent_files=Lista törlése
menu-save=Mentés
menu-edit-sauce=Sauce információ szerkesztése…
menu-9px-font=9px betűtípus
menu-aspect-ratio=Régi képarány
menu-set-canvas-size=Vászon méretének beállítása…
menu-file-settings=File Settings…

file-settings-dialog-title=File Settings
file-settings-canvas-size=Canvas Size
file-settings-format=Format
file-settings-sauce=SAUCE
file-settings-title=Title
file-settings-author=Author
file-settings-group=Group
file-settings-ice=Ice
file-settings-legacy-ar=Legacy Aspect Ratio
file-settings-9px-font=9px Font
file-settings-comments-button=Comments…
file-settings-settings-button=Settings…
file-settings-comments-title=SAUCE Comments
file-settings-comments-info=Max 255 lines, 64 characters per line
menu-close=Bezárás
menu-save-as=Mentés másként…
menu-export=Exportálás…
menu-edit-font-outline=Betűtípus körvonalának szerkesztése…
menu-show_settings=Beállítások…

menu-edit=&Szerkesztés
menu-undo=Visszavonás
menu-redo=Újra
menu-undo-op=Visszavonás: { $op }
menu-redo-op=Újra: { $op }

menu-cut=Kivágás
menu-copy=Másolás
menu-paste=Beillesztés
menu-delete=Törlés
menu-rename=Átnevezés
menu-paste-as=Beillesztés mint
menu-paste-as-new-image=Új kép
menu-paste-as-brush=Ecset
menu-erase=Törlés
menu-flipx=Vízszintes tükrözés
menu-flipy=Függőleges tükrözés
menu-justifyleft=Balra igazítás
menu-justifyright=Jobbra igazítás
menu-justifycenter=Középre igazítás
menu-crop=Kivágás
menu-justify_line_center=Középre igazítás
menu-justify_line_left=Balra igazítás
menu-justify_line_right=Jobbra igazítás
menu-insert_row=Sor beszúrása
menu-delete_row=Sor törlése
menu-insert_colum=Oszlop beszúrása
menu-delete_colum=Oszlop törlése
menu-erase_row=Sor törlése
menu-erase_row_to_start=Sor törlése az elejéig
menu-erase_row_to_end=Sor törlése a végéig
menu-erase_column=Oszlop törlése
menu-erase_column_to_start=Oszlop törlése az elejéig
menu-erase_column_to_end=Oszlop törlése a végéig
menu-scroll_area_up=Terület felgörgetése
menu-scroll_area_down=Terület legörgetése
menu-scroll_area_left=Terület balra görgetése
menu-scroll_area_right=Terület jobbra görgetése
menu-mirror_mode=Tükrözés mód
menu-area_operations=Terület

menu-selection=&Kijelölés
menu-select-all=Összes kijelölése
menu-select_nothing=Kijelölés megszüntetése
menu-inverse_selection=Fordított kijelölés

menu-colors=&Színek
menu-ice-mode=Ice mód
menu-ice-mode-unrestricted=Korlátlan
menu-ice-mode-blink=Villogás
menu-ice-mode-ice=Ice
menu-palette-mode=Paletta mód
menu-palette-mode-unrestricted=Korlátlan
menu-palette-mode-dos=Dos 16
menu-palette-mode-free=Szabad 16
menu-palette-mode-free8=Szabad 8

menu-select_palette=Paletta kiválasztása
menu-next_fg_color=Következő előtér szín
menu-next_bg_color=Következő háttér szín
menu-prev_fg_color=Előző előtér szín
menu-prev_bg_color=Előző háttér szín

menu-view=&Nézet
menu-reference-image=Referencia kép megnyitása…
menu-toggle-reference-image=Referencia kép váltása
menu-clear-reference-image=Törlés
menu-toggle_fullscreen=Teljes képernyő
menu-zoom=Nagyítás
menu-zoom_reset=Nagyítás visszaállítása
menu-zoom_in=Nagyítás
menu-zoom_out=Kicsinyítés
menu-guides=Vezetők
menu-raster=Rács
menu-zoom-fit_size=Illesztés méretre
menu-show_layer_borders=Réteg határainak megjelenítése
menu-show_line_numbers=Sorszámok megjelenítése
menu-toggle_guide=Vezetők váltása
menu-toggle_raster=Rács váltása
menu-toggle_left_pane=Bal panel váltása
menu-toggle_right_pane=Jobb panel váltása

menu-pick_attribute_under_caret=Attribútum kiválasztása
menu-default_color=Alapértelmezett szín
menu-toggle_color=Előtér/Háttér váltása

menu-fonts=Betűtípusok
menu-font-mode=Betűtípus mód
menu-font-mode-unrestricted=Korlátlan
menu-font-mode-sauce=Sauce
menu-font-mode-single=Egyes
menu-font-mode-dual=Kettős
menu-open_font_selector=Betűtípus kiválasztása…
menu-add_fonts=Betűtípusok hozzáadása…
menu-open_font_manager=Betűtípusok kezelése…
menu-open_font_directoy=Betűtípus könyvtár megnyitása…
menu-open_palettes_directoy=Paletta könyvtár megnyitása…

menu-help=&Súgó
menu-discuss=Beszélgetés
menu-open_log_file=Naplófájl megnyitása
menu-report-bug=Hiba jelentése
menu-about=Névjegy…
menu-plugins=&Pluginok
menu-open_plugin_directory=Plugin könyvtár megnyitása…

menu-upgrade_version=Frissítés { $version } verzióramenu-update-available=⬆ Elérhető frissítés: { $version }
tool-fg=Előtér
tool-bg=Háttér
tool-solid=Szilárd
tool-character=Karakter
tool-shade=Árnyék
tool-colorize=Színezés
tool-size-label=Méret
tool-full-block=Teljes blokk
tool-half-block=Fél blokk
tool-outline=Körvonal
tool-custom-brush=Egyedi ecset

tool-select-label=Kijelölési mód
tool-select-normal=Téglalap
tool-select-character=Karakter
tool-select-attribute=Attribútum
tool-select-foreground=Előtér
tool-select-background=Háttér
tool-select-description=Tartsd lenyomva a shiftet a kijelöléshez. Control/Cmd a törléshez.

tool-fill-exact_match_label=Pontos egyezés
tool-flip_horizontal=Vízszintes
tool-flip_vertical=Függőleges

tool-paint_brush_name=Festő ecset
tool-paint_brush_tooltip=Festés ecsettel
tool-click_name=Szövegbevitel
tool-click_tooltip=Szöveg bevitele és téglalap kijelölések
tool-ellipse_name=Ellipszis
tool-ellipse_tooltip=Ellipszis rajzolása
tool-filled_ellipse_name=Kitöltött ellipszis
tool-filled_ellipse_tooltip=Kitöltött ellipszis rajzolása
tool-rectangle_name=Téglalap
tool-rectangle_tooltip=Téglalap rajzolása
tool-filled_rectangle_name=Kitöltött téglalap
tool-filled_rectangle_tooltip=Kitöltött téglalap rajzolása
tool-eraser_name=Radír
tool-eraser_tooltip=Háttér törlése ecsettel
tool-fill_name=Kitöltés
tool-fill_tooltip=Terület kitöltése színnel vagy karakterrel
tool-flip_name=Kapcsoló
tool-flip_tooltip=Függőleges vagy vízszintes fél blokkok kapcsolása
tool-tdf_name=The Draw betűtípusok
tool-tdf_tooltip=Szövegbevitel The Draw betűtípusokkal
tool-line_name=Vonal rajzolása
tool-line_tooltip=Vonalak rajzolása
tool-move_layer_name=Réteg mozgatása
tool-move_layer_tooltip=Rétegek mozgatása
tool-pencil_name=Ceruza
tool-pencil_tooltip=Festés ceruzával
tool-pipette_name=Színválasztó
tool-pipette_tooltip=Szín kiválasztása
tool-select_name=Kijelölő eszköz
tool-select_tooltip=Többszörös és nem téglalap kijelölések
tool-tag_name=Címke eszköz
tool-tag_tooltip=Címkék használata a kimenet bővítéséhez
tool-tag_show=Show Tags
tool-tag_edit_button={ menu-edit }

toolbar-new=Új

new-file-title=Új fájl
new-file-width=Szélesség
new-file-height=Magasság
new-file-ok=Ok
new-file-cancel=Mégse
new-file-create=Létrehozás

edit-sauce-title=Sauce információ szerkesztése
edit-sauce-title-label=Cím
edit-sauce-title-label-length=(35 karakter)
edit-sauce-author-label=Szerző
edit-sauce-author-label-length=(20 karakter)
edit-sauce-group-label=Csoport
edit-sauce-group-label-length=(20 karakter)
edit-sauce-comments-label=Hozzászólások (64 karakter soronként)
edit-sauce-letter-spacing=9px mód használata
edit-sauce-aspect-ratio=Klasszikus képarány szimulálása

edit-canvas-size-title=Vászon méretének beállítása
edit-canvas-size-width-label=Szélesség
edit-canvas-size-height-label=Magasság
edit-canvas-size-resize=Átméretezés
edit-canvas-size-resize_layers-label=Rétegek átméretezése

toolbar-size = { $colums ->
     [1] 1 oszlop
*[other] { $colums } oszlop
} x { $rows ->
     [1] 1 sor
*[other] { $rows } sor
}

toolbar-position = Sor { $line }, Oszlop { $column }
toolbar-layer_offset = Réteg eltolás: { $line }x{ $column }
add_layer_tooltip = Új réteg hozzáadása
move_layer_up_tooltip = Réteg feljebb mozgatása
move_layer_down_tooltip = Réteg lejjebb mozgatása
delete_layer_tooltip = Réteg törlése
anchor_layer_tooltip = Réteg rögzítése

glyph-char-label=Karakter
glyph-font-label=Betűtípus

color-is_blinking=Villogás

export-title=Exportálás
export-button-title=Exportálás
export-file-label=Fájlnév:
export-video-preparation-label=Videó előkészítése:
export-video-preparation-None=Nincs
export-video-preparation-Clear=Képernyő törlése
export-video-preparation-Home=Kezdő kurzor
export-utf8-output-label=Modern terminál formátum (utf8)
export-save-sauce-label=Sauce információ mentése
export-compression-label=Kimenet tömörítése
export-limit-output-line-length-label=Kimeneti sorhossz korlátozása
export-maximum_line_length=Maximális sorhossz
export-use_repeat_sequences=CSI Pn b ismétlési szekvenciák használata
export-save_full_line_length=Végső szóközök mentése
export-format-label=Formátum:
export-path-label=Útvonal:

select-character-title=Karakter kiválasztása

select-outline-style-title=Körvonal betűtípus stílus típusa

about-dialog-title=Névjegy Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Az Icy Draw egy eszköz ANSI és ASCII művészet létrehozásához.
    Rust nyelven íródott és az EGUI könyvtárat használja.

    Az Icy Draw szabad szoftver, az Apache 2 licenc alatt.
    A forráskód elérhető a www.github.com/mkrueger/icy_draw oldalon.
about-dialog-created_by =
    Készítette: { $authors }
    Segítség és tesztelés: NuSkooler, Grymmjack

edit-layer-dialog-title=Réteg tulajdonságai
edit-layer-dialog-name-label=Név
edit-layer-dialog-is-visible-checkbox=Látható
edit-layer-dialog-is-edit-locked-checkbox=Szerkesztés zárolva
edit-layer-dialog-is-position-locked-checkbox=Pozíció zárolva
edit-layer-dialog-is-x-offset-label=X eltolás
edit-layer-dialog-is-y-offset-label=Y eltolás
edit-layer-dialog-has-alpha-checkbox=Alpha van
edit-layer-dialog-is-alpha-locked-checkbox=Alpha zárolva

error-load-file=Hiba a fájl betöltésekor: { $error }

select-font-dialog-title=Betűtípus kiválasztása ({ $fontcount} elérhető)
add-font-dialog-title=Betűtípus hozzáadása ({ $fontcount} elérhető)
select-font-dialog-select=Kiválasztás
add-font-dialog-select=Hozzáadás
select-font-dialog-filter-text=Betűtípusok szűrése
select-font-dialog-no-fonts=Nincs a szűrőnek megfelelő betűtípus
select-font-dialog-no-fonts-installed=Nincs telepített betűtípus
select-font-dialog-color-font=SZÍNES
select-font-dialog-block-font=BLOKK
select-font-dialog-outline-font=KÖRVONAL
select-font-dialog-figlet-font=FIGLET
select-font-dialog-preview-text=HELLO
select-font-dialog-edit-button=Betűtípus szerkesztése…

layer_tool_title=Rétegek
layer_tool_menu_layer_properties=Réteg tulajdonságai
layer_tool_menu_resize_layer=Réteg átméretezése
layer_tool_menu_new_layer=Új réteg
layer_tool_menu_duplicate_layer=Réteg másolása
layer_tool_menu_merge_layer=Réteg egyesítése
layer_tool_menu_delete_layer=Réteg törlése
layer_tool_menu_clear_layer=Réteg törlése

channel_tool_title=Csatornák
channel_tool_fg=Előtér
channel_tool_bg=Háttér

font_tool_select_outline_button=Körvonal
font_tool_current_font_label=Aktuális TDF betűtípus
font_tool_no_font=<nincs>
font_tool_no_fonts_label=
    Nincs tdf betűtípus található.
    Új betűtípusok telepítése a betűtípus könyvtárba
font_tool_open_directory_button=Betűtípus könyvtár megnyitása

pipette_tool_char_code=Kód { $code }
pipette_tool_foreground=Előtér { $fg }
pipette_tool_background=Háttér { $bg }
pipette_tool_keys=
    Tartsd lenyomva a shiftet az előtér szín kiválasztásához

    Tartsd lenyomva a controlt a háttér szín kiválasztásához

char_table_tool_title=Karakter táblázat
minimap_tool_title=Előnézet

no_document_selected=Nincs kiválasztott dokumentum

undo-draw-ellipse=Ellipszis rajzolása
undo-draw-rectangle=Téglalap rajzolása
undo-paint-brush=Festő ecset
undo-pencil=Ceruza
undo-eraser=Radír
undo-bucket-fill=Vödör kitöltése
undo-line=Vonal
undo-cut=Kivágás
undo-paste-glyph=Karakter beillesztése
undo-bitfont-flip-y=Függőleges tükrözés
undo-bitfont-flip-x=Vízszintes tükrözés
undo-bitfont-move-down=Le mozgatás
undo-bitfont-move-up=Fel mozgatás
undo-bitfont-move-left=Balra mozgatás
undo-bitfont-move-right=Jobbra mozgatás
undo-bitfont-inverse=Fordított
undo-bitfont-clear=Törlés
undo-bitfont-edit=Szerkesztés
undo-bitfont-resize=Átméretezés
undo-delete=Törlés
undo-backspace=Backspace

undo-render_character=Karakter megjelenítése
undo-delete_character=Karakter törlése
undo-select=Kijelölés
undo-plugin=Plugin { $title }

font_selector-ansi_font=ANSI
font_selector-library_font=KÖNYVTÁR
font_selector-file_font=FÁJL
font_selector-sauce_font=SAUCE

select-palette-dialog-title=Paletta kiválasztása ({ $count } elérhető)
select-palette-dialog-builtin_palette=BEÉPÍTETT
select-palette-dialog-no-matching-palettes=Nincs a keresési feltételeknek megfelelő paletta.

autosave-dialog-title=Automatikus mentés
autosave-dialog-description=Automatikus mentést találtunk ehhez a fájlhoz.
autosave-dialog-question=Az eredeti fájlt szeretné használni, vagy az automatikus mentést betölteni?
autosave-dialog-load_autosave_button=Betöltés az automatikus mentésből
autosave-dialog-discard_autosave_button=Automatikus mentés elvetése

paste_mode-description=Most beillesztési módban van. Használja a réteg eszközt új réteg hozzáadásához vagy rögzítéséhez.
paste_mode-stamp=Bélyeg
paste_mode-rotate=Forgatás
paste_mode-flipx=Vízszintes tükrözés
paste_mode-flipy=Függőleges tükrözés
paste_mode-transparent=Átlátszó

ask_close_file_dialog-description=Menteni szeretné a { $filename } fájlban végzett módosításokat?
ask_close_file_dialog-subdescription=A módosítások elvesznek,
ask_close_file_dialog-dont_save_button=Ne mentsd
ask_close_file_dialog-save_button=Mentés

tab-context-menu-close=Bezárás
tab-context-menu-close_others=A többi bezárása
tab-context-menu-close_all=Összes bezárása
tab-context-menu-copy_path=Útvonal másolása

font-view-char_label=Karakter
font-view-ascii_label=ASCII
font-view-font_label=Betűtípus
font-view-font_page_label=Betűtípus oldal:

font-editor-tile_area=Mozaik terület
font-editor-clear=Törlés
font-editor-inverse=Fordított
font-editor-flip_x=Vízszintes tükrözés
font-editor-flip_y=Függőleges tükrözés

animation_editor_path_label=Útvonal:
animation_editor_export_button=Exportálás
animation_editor_ansi_label=Ansimation
animation_encoding_frame=Kódoló keret { $cur } / { $total }
animation_of_frame_count=összesen { $total }
animation_icy_play_note=Megjegyzés: Az animáció lejátszásához a konzolon/bbs-en vagy ansi konverzióhoz használja:

new-file-template-cp437-title=CP437 ANSI
new-file-template-cp437-description=
    Hozzon létre egy új DOS 16 színű ANSI fájlt
    Korlátozva 16 DOS színre és Sauce betűtípusra, villogás (kapcsolható)
new-file-template-ice-title=CP437 Ice ANSI
new-file-template-ice-description=
    Hozzon létre egy új DOS 16 színű ice ANSI fájlt
    Korlátozva 16 DOS színre és Sauce betűtípusra, nincs villogás (kapcsolható)
new-file-template-xb-title=XB 16 Színek
new-file-template-xb-description=
    Hozzon létre egy új XB fájlt
    Szabad 16 színű paletta, 1 betűtípus, nincs villogás (kapcsolható)
new-file-template-xb-ext-title=XB Kiterjesztett Betűtípus
new-file-template-xb-ext-description=
    Hozzon létre egy új XB fájlt két betűtípussal
    Szabad 16 színű paletta, 8 előtér, 16 háttér, 2 betűtípus, nincs villogás
new-file-template-ansi-title=Modern ANSI
new-file-template-ansi-description=
    Hozzon létre egy új Ansi fájlt korlátozások nélkül
    Korlátlan paletta, több betűtípus, villogás
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Hozzon létre egy új Atascii fájlt

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=Hozzon létre egy új FILE_ID.DIZ fájlt
new-file-template-ansimation-title=Ansimation
new-file-template-ansimation-description=Hozzon létre egy új ansi animációs fájlt
new-file-template-bit_font-title=Bit Betűtípus
new-file-template-bit_font-description=Hozzon létre egy új bit betűtípus fájlt
new-file-template-color_font-title=TDF Színes Betűtípus
new-file-template-color_font-description=Hozzon létre egy új TheDraw színes betűtípust
new-file-template-block_font-title=TDF Blokk Betűtípus
new-file-template-block_font-description=Hozzon létre egy új TheDraw blokk betűtípust
new-file-template-outline_font-title=TDF Körvonal Betűtípus
new-file-template-outline_font-description=Hozzon létre egy új TheDraw körvonal betűtípust
new-file-template-ansimation-ui-label=
    Az IcyDraw ansimation egy lua szövegfájl, amely egy animációs szekvenciát ír le.
    A szintaxis leírásához kattintson erre a linkre:
new-file-template-bitfont-ui-label=
    A bitfontot a régi számítógépek használják a szöveg megjelenítésére.

new-file-template-thedraw-ui-label=
    A TheDraw betűtípusokat nagyobb szövegek megjelenítésére használják az ANSI szerkesztőkben.
    A TheDraw három betűtípus típust határozott meg: Színes, Blokk és Körvonal. 

    Egy nagy betűtípus archívum letölthető innen:

manage-font-dialog-title=Betűtípusok kezelése
manage-font-used_font_label=Használt betűtípusok
manage-font-copy_font_button=Betűtípus másolása
manage-font-copy_font_button-tooltip=Másolja a betűtípust CTerm ANSI szekvenciaként a vágólapra. (BBS használatra)
manage-font-remove_font_button=Eltávolítás
manage-font-used_label=használt
manage-font-not_used_label=nem használt
manage-font-replace_label=Használat cseréje a hellyel
manage-font-replace_font_button=Csere
manage-font-change_font_slot_button=Betűtípus helyének cseréje

palette_selector-dos_default_palette=VGA 16 szín
palette_selector-dos_default_low_palette=VGA 8 szín
palette_selector-c64_default_palette=C64 színek
palette_selector-ega_default_palette=EGA 64 szín
palette_selector-xterm_default_palette=XTerm kiterjesztett színek
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Kivonva a pufferből

tdf-editor-outline_preview_label=Körvonal karakter előnézet
tdf-editor-draw_bg_checkbox=Háttér használata
tdf-editor-clone_button=Klón
tdf-editor-font_name_label=Betűtípus neve:
tdf-editor-spacing_label=Távolság:
tdf-editor-no_font_selected_label=Nincs kiválasztott betűtípus
tdf-editor-font_type_label=Betűtípus típusa:
tdf-editor-font_type_color=Színes
tdf-editor-font_type_block=Blokk
tdf-editor-font_type_outline=Körvonal
tdf-editor-clear_char_button=Karakter törlése
tdf-editor-cheat_sheet_key=Kulcs
tdf-editor-cheat_sheet_code=Kód
tdf-editor-cheat_sheet_res=Felbontás

settings-heading=Beállítások
settings-reset_button=Visszaállítás
settings-monitor-category=Monitor
settings-char-set-category=Karakterkészletek
settings-font-outline-category=Betűtípus körvonal
settings-markers-guides-category=Jelölők és vezetők
settings-keybindings-category=Billentyűk
settings-reference-alpha=Referencia kép átlátszósága
settings-raster-label=Rács színe:
settings-alpha=átlátszóság
settings-guide-label=Vezető színe:
settings-set-label=Beállítás { $set }
settings-key_filter_preview_text=Billentyűkötések szűrése
settings-char_set_list_label=Karakterkészletek:

edit-tag-title=Címke
edit-tag-filter=Címkék szűrése
edit-tag-preview-label=Előnézet:
edit-tag-replacement-label=Csere:
edit-tag-alignment-label=Igazítás:
edit-tag-length-label=Hossz:
edit-tag-alignment-left=Balra
edit-tag-alignment-right=Jobbra
edit-tag-alignment-center=Középre
edit-tag-placement-label=Elhelyezés:
edit-tag-placement-in_line=Sorban
edit-tag-placement-after=GotoXY-vel
edit-tag-role-label=Szerep:
edit-tag-role-displaycode=Megjelenítési kód
edit-tag-role-hyperlink=Hiperhivatkozás

add_tag_tooltip=Címke hozzáadása
delete_tag_tooltip=Címke törlése

ask_unsaved_file_dialog-description=Szeretné menteni a módosításokat a következő {
    $number ->
        [1] fájlban?
        *[other] {$number} fájlban?
    }
ask_unsaved_file_dialog-subdescription=A módosítások elvesznek, ha nem menti őket.
ask_unsaved_file_dialog-save_all_button=Összes mentése
ask_unsaved_file_dialog-dont_save_button=Ne mentse

# Save Changes Dialog (single file)
save-changes-title=Menti a változtatásokat a(z) "{ $filename }" fájlban?
save-changes-description=A módosítások elvesznek, ha nem menti őket.

# Paste Tool Toolbar
paste-tool-stamp=Bélyegző (S)
paste-tool-rotate=Forgatás (R)
paste-tool-flip-x=X tükrözés
paste-tool-flip-y=Y tükrözés
paste-tool-transparent=Átlátszó (T)
paste-tool-hint=Enter: Rögzít | Esc: Mégse | Nyilak: Mozgatás

# Animation Export Dialog
animation-export-format=Formátum
animation-export-path=Exportálás ide
animation-export-no-path=Nincs útvonal kiválasztva
animation-export-success=Exportálás sikeresen befejeződött
animation-export-exporting-frame=Képkocka exportálása { $current } / { $total }
animation-export-encoding=Videó kódolása…
animation-export-cancelled=Exportálás megszakítva
animation-export-no-frames=Nincs exportálandó képkocka
animation-export-failed=Exportálási hiba: { $error }
