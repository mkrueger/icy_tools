font-editor-table = Taula de caràcters 0-{ $length }:

unsaved-title=Sense títol

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

menu-file=Fitxer
menu-new=Nou…
menu-open=Obrir…
menu-open_recent=Obrir recent
menu-open_recent_clear=Netejar
menu-no_recent_files=Cap fitxer recent
menu-clear_recent_files=Netejar llista
menu-save=Desar
menu-edit-sauce=Editar informació de Sauce…
menu-9px-font=Font de 9px
menu-aspect-ratio=Relació d'aspecte llegat
menu-set-canvas-size=Establir mida del llenç…
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
menu-close=Tancar
menu-save-as=Desar com…
menu-export=Exportar…
menu-edit-font-outline=Contorn de la font…
menu-show_settings=Configuració…

menu-edit=Editar
menu-undo=Desfer
menu-redo=Refer
menu-undo-op=Desfer: { $op }
menu-redo-op=Refer: { $op }

menu-cut=Retallar
menu-copy=Copiar
menu-paste=Enganxar
menu-delete=Eliminar
menu-rename=Reanomenar
menu-paste-as=Enganxar com
menu-paste-as-new-image=Nova imatge
menu-paste-as-brush=Pincell
menu-erase=Esborrar
menu-flipx=Invertir X
menu-flipy=Invertir Y
menu-justifyleft=Justificar a l'esquerra
menu-justifyright=Justificar a la dreta
menu-justifycenter=Centrat
menu-crop=Retallar
menu-justify_line_center=Centrat de línia
menu-justify_line_left=Justificar línia a l'esquerra
menu-justify_line_right=Justificar línia a la dreta
menu-insert_row=Inserir fila
menu-delete_row=Eliminar fila
menu-insert_colum=Inserir columna
menu-delete_colum=Eliminar columna
menu-erase_row=Esborrar fila
menu-erase_row_to_start=Esborrar fila fins al començament
menu-erase_row_to_end=Esborrar fila fins al final
menu-erase_column=Esborrar columna
menu-erase_column_to_start=Esborrar columna fins al començament
menu-erase_column_to_end=Esborrar columna fins al final
menu-scroll_area_up=Desplaçar àrea cap amunt
menu-scroll_area_down=Desplaçar àrea cap avall
menu-scroll_area_left=Desplaçar àrea cap a l'esquerra
menu-scroll_area_right=Desplaçar àrea cap a la dreta
menu-mirror_mode=Mode mirall
menu-area_operations=Àrea

menu-selection=Selecció
menu-select-all=Seleccionar tot
menu-select_nothing=Desseleccionar
menu-inverse_selection=Invertir selecció

menu-colors=Colors
menu-ice-mode=Mode Ice
menu-ice-mode-unrestricted=Sense restriccions
menu-ice-mode-blink=Parpelleig
menu-ice-mode-ice=Ice
menu-palette-mode=Mode paleta
menu-palette-mode-unrestricted=Sense restriccions
menu-palette-mode-dos=Dos 16
menu-palette-mode-free=Lliure 16
menu-palette-mode-free8=Lliure 8

menu-select_palette=Seleccionar paleta
menu-next_fg_color=Següent color de primer pla
menu-next_bg_color=Següent color de fons
menu-prev_fg_color=Color de primer pla anterior
menu-prev_bg_color=Color de fons anterior

menu-view=Vista
menu-reference-image=Obrir imatge de referència…
menu-toggle-reference-image=Alternar imatge de referència
menu-clear-reference-image=Netejar
menu-toggle_fullscreen=Pantalla completa
menu-zoom=Zoom
menu-zoom_reset=Restablir zoom
menu-zoom_in=Augmentar zoom
menu-zoom_out=Reduir zoom
menu-guides=Guies
menu-raster=Quadrícula
menu-zoom-fit_size=Ajustar mida
menu-show_layer_borders=Mostrar fronteres de capa
menu-show_line_numbers=Mostrar números de línia
menu-toggle_guide=Alternar guies
menu-toggle_raster=Alternar quadrícula
menu-toggle_left_pane=Alternar panell esquerre
menu-toggle_right_pane=Alternar panell dret

menu-pick_attribute_under_caret=Recollir atribut
menu-default_color=Color per defecte
menu-toggle_color=Canviar color de primer pla/fons

menu-fonts=Fonts
menu-font-mode=Mode de font
menu-font-mode-unrestricted=Sense restriccions
menu-font-mode-sauce=Sauce
menu-font-mode-single=Simple
menu-font-mode-dual=Doble
menu-open_font_selector=Seleccionar font…
menu-add_fonts=Afegir fonts…
menu-open_font_manager=Editar fonts del buffer…
menu-open_font_directoy=Obrir directori de fonts…
menu-open_palettes_directoy=Obrir directori de paletes…

menu-help=Ajuda
menu-discuss=Discutir
menu-open_log_file=Obrir fitxer de registre
menu-report-bug=Informar d'un error
menu-about=Quant a…
menu-plugins=Plugins
menu-open_plugin_directory=Obrir directori de plugins…

menu-upgrade_version=Actualitzar a { $version }

tool-fg=Primer pla
tool-bg=Fons
tool-solid=Sòlid
tool-character=Caràcter
tool-shade=Ombra
tool-colorize=Coloritzar
tool-size-label=Mida
tool-full-block=Bloc complet
tool-half-block=Bloc mig
tool-outline=Contorn
tool-custom-brush=Pincell personalitzat

tool-select-label=Mode de selecció
tool-select-normal=Rectangle
tool-select-character=Caràcter
tool-select-attribute=Atribut
tool-select-foreground=Primer pla
tool-select-background=Fons
tool-select-description=Mantingueu premut Maj per afegir a una selecció. Control/Cmd per eliminar.

tool-fill-exact_match_label=Coincidència exacta
tool-flip_horizontal=Horitzontal
tool-flip_vertical=Vertical

tool-paint_brush_name=Pincell de pintura
tool-paint_brush_tooltip=Pintar traços amb un pincell
tool-click_name=Entrada de text
tool-click_tooltip=Introduir text i seleccions rectangulars
tool-ellipse_name=El·lipse
tool-ellipse_tooltip=Dibuixar el·lipse
tool-filled_ellipse_name=El·lipse plena
tool-filled_ellipse_tooltip=Dibuixar el·lipse plena
tool-rectangle_name=Rectangle
tool-rectangle_tooltip=Dibuixar rectangle
tool-filled_rectangle_name=Rectangle ple
tool-filled_rectangle_tooltip=Dibuixar rectangle ple
tool-eraser_name=Goma d'esborrar
tool-eraser_tooltip=Esborrar al fons amb un pincell
tool-fill_name=Omplir
tool-fill_tooltip=Omplir àrea amb color o caràcter
tool-flip_name=Commutador
tool-flip_tooltip=Canviar blocs mig verticals o horitzontals
tool-tdf_name=Fonts The Draw
tool-tdf_tooltip=Entrada de text amb fonts The Draw
tool-line_name=Dibuixar línia
tool-line_tooltip=Dibuixar línies
tool-move_layer_name=Moure capa
tool-move_layer_tooltip=Moure capes
tool-pencil_name=Llapis
tool-pencil_tooltip=Pintar traços amb un llapis
tool-pipette_name=Selector de color
tool-pipette_tooltip=Recollir un color
tool-select_name=Eina de selecció
tool-select_tooltip=Seleccions múltiples i no rectangulars
tool-tag_name=Eina d'etiquetes
tool-tag_tooltip=Les etiquetes s'utilitzen per expandir cadenes en la sortida
tool-tag_show=Show Tags
tool-tag_edit_button={ menu-edit }

toolbar-new=Nou

new-file-title=Nou fitxer
new-file-width=Amplada
new-file-height=Alçada
new-file-ok=D'acord
new-file-cancel=Cancel·lar
new-file-create=Crear

edit-sauce-title=Editar informació de Sauce
edit-sauce-title-label=Títol
edit-sauce-title-label-length=(35 caràcters)
edit-sauce-author-label=Autor
edit-sauce-author-label-length=(20 caràcters)
edit-sauce-group-label=Grup
edit-sauce-group-label-length=(20 caràcters)
edit-sauce-comments-label=Comentaris (64 caràcters per línia)
edit-sauce-letter-spacing=Utilitzar mode de 9px
edit-sauce-aspect-ratio=Simular relació d'aspecte clàssica

edit-canvas-size-title=Establir mida del llenç
edit-canvas-size-width-label=Amplada
edit-canvas-size-height-label=Alçada
edit-canvas-size-resize=Redimensionar
edit-canvas-size-resize_layers-label=Redimensionar capes

toolbar-size = { $colums ->
     [1] 1 Columna
*[other] { $colums } Columnes
} x { $rows ->
     [1] 1 Fila
*[other] { $rows } Files
}

toolbar-position = Ln { $line }, Col { $column }
toolbar-layer_offset = Desplaçament de capa: { $line }x{ $column }
add_layer_tooltip = Afegir nova capa
move_layer_up_tooltip = Moure capa amunt
move_layer_down_tooltip = Moure capa avall
delete_layer_tooltip = Eliminar capa
anchor_layer_tooltip = Ancorar capa

glyph-char-label=Caràcter
glyph-font-label=Font

color-is_blinking=Parpelleig

export-title=Exportar
export-button-title=Exportar
export-file-label=Nom del fitxer:
export-video-preparation-label=Preparació de vídeo:
export-video-preparation-None=Cap
export-video-preparation-Clear=Netejar pantalla
export-video-preparation-Home=Cursor a casa
export-utf8-output-label=Format de terminal modern (utf8)
export-save-sauce-label=Desar informació de Sauce
export-compression-label=Comprimir sortida
export-limit-output-line-length-label=Limitar longitud de línia de sortida
export-maximum_line_length=Longitud màxima de línia
export-use_repeat_sequences=Utilitzar seqüències de repetició CSI Pn b
export-save_full_line_length=Desar espais en blanc finals
export-format-label=Format:
export-path-label=Camí:

select-character-title=Seleccionar caràcter

select-outline-style-title=Tipus d'estil de contorn de font

about-dialog-title=Quant a Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Icy Draw és una eina per crear art ANSI i ASCII.
    Està escrita en Rust i utilitza la biblioteca EGUI.

    Icy Draw és programari lliure, llicenciat sota la llicència Apache 2.
    El codi font està disponible a www.github.com/mkrueger/icy_draw
about-dialog-created_by =
    Creat per { $authors }
    Ajuda i proves: NuSkooler, Grymmjack

edit-layer-dialog-title=Propietats de la capa
edit-layer-dialog-name-label=Nom
edit-layer-dialog-is-visible-checkbox=Visible
edit-layer-dialog-is-edit-locked-checkbox=Bloquejat per edició
edit-layer-dialog-is-position-locked-checkbox=Bloquejat per posició
edit-layer-dialog-is-x-offset-label=Desplaçament X
edit-layer-dialog-is-y-offset-label=Desplaçament Y
edit-layer-dialog-has-alpha-checkbox=Té alfa
edit-layer-dialog-is-alpha-locked-checkbox=Bloquejat per alfa

error-load-file=Error en carregar el fitxer: { $error }

select-font-dialog-title=Seleccionar font ({ $fontcount} disponibles)
add-font-dialog-title=Afegir font ({ $fontcount} disponibles)
select-font-dialog-select=Seleccionar
add-font-dialog-select=Afegir
select-font-dialog-filter-text=Filtrar fonts
select-font-dialog-no-fonts=No hi ha fonts que coincideixin amb el filtre
select-font-dialog-no-fonts-installed=No hi ha fonts instal·lades
select-font-dialog-color-font=COLOR
select-font-dialog-block-font=BLOC
select-font-dialog-outline-font=CONTORN
select-font-dialog-figlet-font=FIGLET
select-font-dialog-preview-text=HOLA
select-font-dialog-edit-button=Editar font…

layer_tool_title=Capes
layer_tool_menu_layer_properties=Propietats de la capa
layer_tool_menu_resize_layer=Redimensionar capa
layer_tool_menu_new_layer=Nova capa
layer_tool_menu_duplicate_layer=Duplicar capa
layer_tool_menu_merge_layer=Fusionar capa
layer_tool_menu_delete_layer=Eliminar capa
layer_tool_menu_clear_layer=Netejar capa

channel_tool_title=Canals
channel_tool_fg=Primer pla
channel_tool_bg=Fons

font_tool_select_outline_button=Contorn
font_tool_current_font_label=Font TDF actual
font_tool_no_font=<cap>
font_tool_no_fonts_label=
    No s'han trobat fonts tdf.
    Instal·leu noves fonts al directori de fonts
font_tool_open_directory_button=Obrir directori de fonts

pipette_tool_char_code=Codi { $code }
pipette_tool_foreground=Primer pla { $fg }
pipette_tool_background=Fons { $bg }
pipette_tool_keys=
    Mantingueu premut Maj per recollir
    color de primer pla

    Mantingueu premut Control per recollir
    color de fons

char_table_tool_title=Taula de caràcters
minimap_tool_title=Previsualització

no_document_selected=No s'ha seleccionat cap document

undo-draw-ellipse=Dibuixar el·lipse
undo-draw-rectangle=Dibuixar rectangle
undo-paint-brush=Pincell de pintura
undo-pencil=Llapis
undo-eraser=Goma d'esborrar
undo-bucket-fill=Omplir amb cubell
undo-line=Línia
undo-cut=Retallar
undo-paste-glyph=Enganxar glif
undo-bitfont-flip-y=Invertir Y
undo-bitfont-flip-x=Invertir X
undo-bitfont-move-down=Moure avall
undo-bitfont-move-up=Moure amunt
undo-bitfont-move-left=Moure a l'esquerra
undo-bitfont-move-right=Moure a la dreta
undo-bitfont-inverse=Invertir
undo-bitfont-clear=Netejar
undo-bitfont-edit=Editar
undo-bitfont-resize=Redimensionar
undo-delete=Eliminar
undo-backspace=Retrocedir

undo-render_character=Renderitzar caràcter
undo-delete_character=Eliminar caràcter
undo-select=Seleccionar
undo-plugin=Plugin { $title }

font-selector-ansi_font=ANSI
font-selector-library_font=BIBLIOTECA
font-selector-file_font=FITXER
font-selector-sauce_font=SAUCE

select-palette-dialog-title=Seleccionar paleta ({ $count } disponibles)
select-palette-dialog-builtin_palette=INCORPORADA
select-palette-dialog-no-matching-palettes=No s'han trobat paletes que coincideixin amb els criteris de cerca.

autosave-dialog-title=Desament automàtic
autosave-dialog-description=S'ha trobat un desament automàtic per aquest fitxer.
autosave-dialog-question=Voleu utilitzar el fitxer original o carregar el desament automàtic?
autosave-dialog-load_autosave_button=Carregar desament automàtic
autosave-dialog-discard_autosave_button=Descartar desament automàtic

paste_mode-description=Ara esteu en mode d'enganxament. Utilitzeu l'eina de capa per afegir o ancorar la capa.
paste_mode-stamp=Segell
paste_mode-rotate=Rotar
paste_mode-flipx=Invertir X
paste_mode-flipy=Invertir Y
paste_mode-transparent=Transparent

ask_close_file_dialog-description=Voleu desar els canvis que heu fet a { $filename }?
ask_close_file_dialog-subdescription=Els vostres canvis es perdran si no els deseu.
ask_close_file_dialog-dont_save_button=No desar
ask_close_file_dialog-save_button=Desar

tab-context-menu-close=Tancar
tab-context-menu-close_others=Tancar altres
tab-context-menu-close_all=Tancar tot
tab-context-menu-copy_path=Copiar camí

font-view-char_label=Caràcter
font-view-ascii_label=ASCII
font-view-font_label=Font
font-view-font_page_label=Pàgina de font:

font-editor-tile_area=Àrea de rajoles
font-editor-clear=Netejar
font-editor-inverse=Invertir
font-editor-flip_x=Invertir X
font-editor-flip_y=Invertir Y

animation_editor_path_label=Camí:
animation_editor_export_button=Exportar
animation_editor_ansi_label=Ansimation
animation_encoding_frame=Codificant fotograma { $cur } de { $total }
animation_of_frame_count=de { $total }
animation_icy_play_note=Nota: Per reproduir l'animació a la consola/bbs o conversió ansi utilitzeu:

new-file-template-cp437-title=ANSI CP437
new-file-template-cp437-description=
    Crear un nou fitxer ANSI de 16 colors DOS
    Limitat a 16 colors DOS i font Sauce, té parpelleig (es pot canviar)
new-file-template-ice-title=ANSI Ice CP437
new-file-template-ice-description=
    Crear un nou fitxer ANSI de 16 colors DOS Ice
    Limitat a 16 colors DOS i font Sauce, sense parpelleig (es pot canviar)
new-file-template-xb-title=XB 16 Colors
new-file-template-xb-description=
    Crear un nou fitxer XB
    Paleta de 16 colors lliure, 1 font, sense parpelleig (es pot canviar)
new-file-template-xb-ext-title=Font estesa XB
new-file-template-xb-ext-description=
    Crear un nou fitxer XB que conté dues fonts
    Paleta de 16 colors lliure, 8 fg, 16 bg, 2 fonts, sense parpelleig
new-file-template-ansi-title=ANSI modern
new-file-template-ansi-description=
    Crear un nou fitxer Ansi sense restriccions
    Paleta il·limitada, múltiples fonts, parpelleig
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Crear un nou fitxer Atascii

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=Crear un nou fitxer FILE_ID.DIZ
new-file-template-ansimation-title=Ansimation
new-file-template-ansimation-description=Crear un nou fitxer d'animació ansi
new-file-template-bit_font-title=Font de bits
new-file-template-bit_font-description=Crear un nou fitxer de font de bits
new-file-template-color_font-title=Font de color TDF
new-file-template-color_font-description=Crear una nova font de color TheDraw
new-file-template-block_font-title=Font de bloc TDF
new-file-template-block_font-description=Crear una nova font de bloc TheDraw
new-file-template-outline_font-title=Font de contorn TDF
new-file-template-outline_font-description=Crear una nova font de contorn TheDraw
new-file-template-ansimation-ui-label=
    Una ansimació IcyDraw és un fitxer de text lua que descriu una seqüència d'animació.
    Per a una descripció de la sintaxi, feu clic en aquest enllaç:
new-file-template-bitfont-ui-label=
    Una font de bits s'utilitza en ordinadors antics per mostrar text.

new-file-template-thedraw-ui-label=
    Les fonts TheDraw s'utilitzen per renderitzar text més gran en editors ANSI.
    TheDraw defineix tres tipus de fonts: Color, Bloc i Contorn. 

    Es pot descarregar un gran arxiu de fonts des de:

manage-font-dialog-title=Gestionar fonts
manage-font-used_font_label=Fonts utilitzades
manage-font-copy_font_button=Copiar font
manage-font-copy_font_button-tooltip=Copia la font com a seqüència ANSI de CTerm al porta-retalls. (per a ús en BBS)
manage-font-remove_font_button=Eliminar
manage-font-used_label=utilitzat
manage-font-not_used_label=no utilitzat
manage-font-replace_label=Reemplaçar ús amb ranura
manage-font-replace_font_button=Reemplaçar
manage-font-change_font_slot_button=Canviar ranura de font

palette_selector-dos_default_palette=VGA 16 colors
palette_selector-dos_default_low_palette=VGA 8 colors
palette_selector-c64_default_palette=Colors C64
palette_selector-ega_default_palette=EGA 64 colors
palette_selector-xterm_default_palette=Colors ampliats XTerm
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Extret del buffer

tdf-editor-outline_preview_label=Previsualització de glif de contorn
tdf-editor-draw_bg_checkbox=Utilitzar fons
tdf-editor-clone_button=Clonar
tdf-editor-font_name_label=Nom de la font:
tdf-editor-spacing_label=Espaiat:
tdf-editor-no_font_selected_label=No s'ha seleccionat cap font
tdf-editor-font_type_label=Tipus de font:
tdf-editor-font_type_color=Color
tdf-editor-font_type_block=Bloc
tdf-editor-font_type_outline=Contorn
tdf-editor-clear_char_button=Netejar caràcter
tdf-editor-cheat_sheet_key=Clau
tdf-editor-cheat_sheet_code=Codi
tdf-editor-cheat_sheet_res=Res

settings-heading=Configuració
settings-reset_button=Restablir
settings-monitor-category=Monitor
settings-char-set-category=Jocs de caràcters
settings-font-outline-category=Contorn de la font
settings-markers-guides-category=Marcadors i guies
settings-keybindings-category=Tecles
settings-reference-alpha=Alfa de la imatge de referència
settings-raster-label=Color de la quadrícula:
settings-alpha=alfa
settings-guide-label=Color de la guia:
settings-set-label=Establir { $set }
settings-key_filter_preview_text=Filtrar assignacions de tecles
settings-char_set_list_label=Jocs de caràcters:

edit-tag-title=Etiqueta
edit-tag-filter=Filtrar etiquetes
edit-tag-preview-label=Previsualització:
edit-tag-replacement-label=Reemplaçament:
edit-tag-alignment-label=Alineació:
edit-tag-length-label=Longitud:
edit-tag-alignment-left=Esquerra
edit-tag-alignment-right=Dreta
edit-tag-alignment-center=Centre
edit-tag-placement-label=Col·locació:
edit-tag-placement-in_line=En línia
edit-tag-placement-after=Amb GotoXY
edit-tag-role-label=Rol:
edit-tag-role-displaycode=Codi de visualització
edit-tag-role-hyperlink=Enllaç

add_tag_tooltip=Afegir etiqueta
delete_tag_tooltip=Eliminar etiqueta

ask_unsaved_file_dialog-description=Voleu desar els canvis als següents {
    $number ->
        [1] fitxer?
        *[other] {$number} fitxers?
    }
ask_unsaved_file_dialog-subdescription=Els vostres canvis es perdran si no els deseu.
ask_unsaved_file_dialog-save_all_button=Desar tot
ask_unsaved_file_dialog-dont_save_button=No desar

# Save Changes Dialog (single file)
save-changes-title=Desar els canvis a "{ $filename }"?
save-changes-description=Els vostres canvis es perdran si no els deseu.

# Paste Tool Toolbar
paste-tool-stamp=Estampar (S)
paste-tool-rotate=Girar (R)
paste-tool-flip-x=Capgirar X
paste-tool-flip-y=Capgirar Y
paste-tool-transparent=Transparent (T)
paste-tool-hint=Enter: Ancorar | Esc: Cancel·lar | Fletxes: Moure