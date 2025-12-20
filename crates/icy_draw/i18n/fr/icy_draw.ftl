font-editor-table = Table des caractères 0-{ $length }:

unsaved-title=Sans titre

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

menu-file=Fichier
menu-new=Nouvelle…
menu-open=Ouvrir…
menu-open_recent=Ouvrir récent
menu-open_recent_clear=Effacer
menu-save=Enregistrer
menu-edit-sauce=Modifier les informations Sauce…
menu-9px-font=Police de 9px
menu-aspect-ratio=Ratio d'aspect hérité
menu-set-canvas-size=Définir la taille de la toile…
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
menu-close=Fermer
menu-save-as=Enregistrer sous…
menu-export=Exporter…
menu-edit-font-outline=Contour de la police…
menu-show_settings=Paramètres…

menu-edit=Éditer
menu-undo=Annuler
menu-redo=Rétablir
menu-undo-op=Annuler: { $op }
menu-redo-op=Rétablir: { $op }

menu-cut=Couper
menu-copy=Copier
menu-paste=Coller
menu-delete=Supprimer
menu-rename=Renommer
menu-paste-as=Coller comme
menu-paste-as-new-image=Nouvelle image
menu-paste-as-brush=Pinceau
menu-erase=Effacer
menu-flipx=Retourner X
menu-flipy=Retourner Y
menu-justifyleft=Aligner à gauche
menu-justifyright=Aligner à droite
menu-justifycenter=Centrer
menu-crop=Recadrer
menu-justify_line_center=Centrer la ligne
menu-justify_line_left=Aligner la ligne à gauche
menu-justify_line_right=Aligner la ligne à droite
menu-insert_row=Insérer une ligne
menu-delete_row=Supprimer une ligne
menu-insert_colum=Insérer une colonne
menu-delete_colum=Supprimer une colonne
menu-erase_row=Effacer la ligne
menu-erase_row_to_start=Effacer la ligne jusqu'au début
menu-erase_row_to_end=Effacer la ligne jusqu'à la fin
menu-erase_column=Effacer la colonne
menu-erase_column_to_start=Effacer la colonne jusqu'au début
menu-erase_column_to_end=Effacer la colonne jusqu'à la fin
menu-scroll_area_up=Faire défiler la zone vers le haut
menu-scroll_area_down=Faire défiler la zone vers le bas
menu-scroll_area_left=Faire défiler la zone vers la gauche
menu-scroll_area_right=Faire défiler la zone vers la droite
menu-mirror_mode=Mode miroir
menu-area_operations=Zone

menu-selection=Sélection
menu-select-all=Tout sélectionner
menu-select_nothing=Désélectionner
menu-inverse_selection=Inverser la sélection

menu-colors=Couleurs
menu-ice-mode=Mode Ice
menu-ice-mode-unrestricted=Sans restrictions
menu-ice-mode-blink=Clignotement
menu-ice-mode-ice=Ice
menu-palette-mode=Mode palette
menu-palette-mode-unrestricted=Sans restrictions
menu-palette-mode-dos=Dos 16
menu-palette-mode-free=Libre 16
menu-palette-mode-free8=Libre 8

menu-select_palette=Sélectionner une palette
menu-next_fg_color=Couleur de premier plan suivante
menu-next_bg_color=Couleur de fond suivante
menu-prev_fg_color=Couleur de premier plan précédente
menu-prev_bg_color=Couleur de fond précédente

menu-view=Vue
menu-reference-image=Ouvrir l'image de référence…
menu-toggle-reference-image=Basculer l'image de référence
menu-clear-reference-image=Effacer
menu-toggle_fullscreen=Plein écran
menu-zoom=Zoom
menu-zoom_reset=Réinitialiser le zoom
menu-zoom_in=Agrandir
menu-zoom_out=Rétrécir
menu-guides=Guides
menu-raster=Grille
menu-zoom-fit_size=Ajuster la taille
menu-show_layer_borders=Afficher les bordures des calques
menu-show_line_numbers=Afficher les numéros de ligne
menu-toggle_guide=Basculer les guides
menu-toggle_raster=Basculer la grille
menu-toggle_left_pane=Basculer le panneau gauche
menu-toggle_right_pane=Basculer le panneau droit

menu-pick_attribute_under_caret=Sélectionner l'attribut
menu-default_color=Couleur par défaut
menu-toggle_color=Changer premier plan/fond

menu-fonts=Polices
menu-font-mode=Mode police
menu-font-mode-unrestricted=Sans restrictions
menu-font-mode-sauce=Sauce
menu-font-mode-single=Simple
menu-font-mode-dual=Double
menu-open_font_selector=Sélectionner une police…
menu-add_fonts=Ajouter des polices…
menu-open_font_manager=Modifier les polices du buffer…
menu-open_font_directoy=Ouvrir le répertoire des polices…
menu-open_palettes_directoy=Ouvrir le répertoire des palettes…

menu-help=Aide
menu-discuss=Discuter
menu-open_log_file=Ouvrir le fichier journal
menu-report-bug=Signaler un bug
menu-about=À propos…
menu-plugins=Plugins
menu-open_plugin_directory=Ouvrir le répertoire des plugins…

menu-upgrade_version=Mettre à jour vers { $version }

tool-fg=Premier plan
tool-bg=Fond
tool-solid=Solide
tool-character=Caractère
tool-shade=Ombre
tool-colorize=Colorier
tool-size-label=Taille
tool-full-block=Bloc complet
tool-half-block=Demi-bloc
tool-outline=Contour
tool-custom-brush=Pinceau personnalisé

tool-select-label=Mode de sélection
tool-select-normal=Rectangle
tool-select-character=Caractère
tool-select-attribute=Attribut
tool-select-foreground=Premier plan
tool-select-background=Fond
tool-select-description=Maintenez shift pour ajouter à la sélection. Control/Cmd pour retirer.

tool-fill-exact_match_label=Correspondance exacte
tool-flip_horizontal=Horizontal
tool-flip_vertical=Vertical

tool-paint_brush_name=Pinceau
tool-paint_brush_tooltip=Peindre des traits avec un pinceau
tool-click_name=Entrée de texte
tool-click_tooltip=Entrer du texte et des sélections rectangulaires
tool-ellipse_name=Ellipse
tool-ellipse_tooltip=Dessiner une ellipse
tool-filled_ellipse_name=Ellipse remplie
tool-filled_ellipse_tooltip=Dessiner une ellipse remplie
tool-rectangle_name=Rectangle
tool-rectangle_tooltip=Dessiner un rectangle
tool-filled_rectangle_name=Rectangle rempli
tool-filled_rectangle_tooltip=Dessiner un rectangle rempli
tool-eraser_name=Gomme
tool-eraser_tooltip=Effacer au fond avec un pinceau
tool-fill_name=Remplir
tool-fill_tooltip=Remplir une zone avec une couleur ou un caractère
tool-flip_name=Inverser
tool-flip_tooltip=Inverser les blocs moyens verticalement ou horizontalement
tool-tdf_name=The Draw Fonts
tool-tdf_tooltip=Entrée de texte avec The Draw Fonts
tool-line_name=Dessiner une ligne
tool-line_tooltip=Dessiner des lignes
tool-move_layer_name=Déplacer le calque
tool-move_layer_tooltip=Déplacer les calques
tool-pencil_name=Crayon
tool-pencil_tooltip=Peindre des traits avec un crayon
tool-pipette_name=Sélecteur de couleur
tool-pipette_tooltip=Sélectionner une couleur
tool-select_name=Outil de sélection
tool-select_tooltip=Sélections multiples et non rectangulaires
tool-tag_name=Outil d'étiquettes
tool-tag_tooltip=Les étiquettes sont utilisées pour étendre les chaînes dans la sortie
tool-tag_show=Show Tags
tool-tag_edit_button={ menu-edit }

toolbar-new=Nouvelle

new-file-title=Nouvelle fichier
new-file-width=Largeur
new-file-height=Hauteur
new-file-ok=Ok
new-file-cancel=Annuler
new-file-create=Créer

edit-sauce-title=Modifier les informations Sauce
edit-sauce-title-label=Titre
edit-sauce-title-label-length=(35 caractères)
edit-sauce-author-label=Auteur
edit-sauce-author-label-length=(20 caractères)
edit-sauce-group-label=Groupe
edit-sauce-group-label-length=(20 caractères)
edit-sauce-comments-label=Commentaires (limite de 64 caractères par ligne)
edit-sauce-letter-spacing=Utiliser le mode 9px
edit-sauce-aspect-ratio=Simuler le ratio d'aspect classique

edit-canvas-size-title=Définir la taille de la toile
edit-canvas-size-width-label=Largeur
edit-canvas-size-height-label=Hauteur
edit-canvas-size-resize=Redimensionner
edit-canvas-size-resize_layers-label=Redimensionner les calques

toolbar-size = { $colums ->
     [1] 1 colonne
*[other] { $colums } colonnes
} x { $rows ->
     [1] 1 ligne
*[other] { $rows } lignes
}

toolbar-position = Ln { $line }, Col { $column }
toolbar-layer_offset = Décalage de calque: { $line }x{ $column }
add_layer_tooltip = Ajouter un nouveau calque
move_layer_up_tooltip = Déplacer le calque vers le haut
move_layer_down_tooltip = Déplacer le calque vers le bas
delete_layer_tooltip = Supprimer le calque
anchor_layer_tooltip = Ancrer le calque

glyph-char-label=Caractère
glyph-font-label=Police

color-is_blinking=Clignotement

export-title=Exporter
export-button-title=Exporter
export-file-label=Nom du fichier:
export-video-preparation-label=Préparation de la vidéo:
export-video-preparation-None=Aucun
export-video-preparation-Clear=Effacer l'écran
export-video-preparation-Home=Curseur au début
export-utf8-output-label=Format de terminal moderne (utf8)
export-save-sauce-label=Enregistrer les informations Sauce
export-compression-label=Compresser la sortie
export-limit-output-line-length-label=Limiter la longueur de la ligne de sortie
export-maximum_line_length=Longueur maximale de la ligne
export-use_repeat_sequences=Utiliser les séquences de répétition CSI Pn b
export-save_full_line_length=Enregistrer les espaces blancs à la fin
export-format-label=Format:
export-path-label=Chemin:

select-character-title=Sélectionner un caractère

select-outline-style-title=Type de style de contour de police

about-dialog-title=À propos de Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Icy Draw est un outil pour créer de l'art ANSI et ASCII.
    Il est écrit en Rust et utilise la bibliothèque EGUI.

    Icy Draw est un logiciel libre, sous licence Apache 2.
    Le code source est disponible sur www.github.com/mkrueger/icy_draw
about-dialog-created_by =
    Créé par { $authors }
    Aide et tests: NuSkooler, Grymmjack

edit-layer-dialog-title=Propriétés du calque
edit-layer-dialog-name-label=Nom
edit-layer-dialog-is-visible-checkbox=Visible
edit-layer-dialog-is-edit-locked-checkbox=Édition verrouillée
edit-layer-dialog-is-position-locked-checkbox=Position verrouillée
edit-layer-dialog-is-x-offset-label=Décalage X
edit-layer-dialog-is-y-offset-label=Décalage Y
edit-layer-dialog-has-alpha-checkbox=A un alpha
edit-layer-dialog-is-alpha-locked-checkbox=Alpha verrouillé

error-load-file=Erreur lors du chargement du fichier: { $error }

select-font-dialog-title=Sélectionner une police ({ $fontcount} disponibles)
add-font-dialog-title=Ajouter une police ({ $fontcount} disponibles)
select-font-dialog-select=Sélectionner
add-font-dialog-select=Ajouter
select-font-dialog-filter-text=Filtrer les polices
select-font-dialog-no-fonts=Aucune police ne correspond au filtre
select-font-dialog-no-fonts-installed=Aucune police installée
select-font-dialog-color-font=COULEUR
select-font-dialog-block-font=BLOC
select-font-dialog-outline-font=CONTOUR
select-font-dialog-figlet-font=FIGLET
select-font-dialog-preview-text=BONJOUR
select-font-dialog-edit-button=Modifier la police…

layer_tool_title=Calques
layer_tool_menu_layer_properties=Propriétés du calque
layer_tool_menu_resize_layer=Redimensionner le calque
layer_tool_menu_new_layer=Nouvelle calque
layer_tool_menu_duplicate_layer=Dupliquer le calque
layer_tool_menu_merge_layer=Fusionner le calque
layer_tool_menu_delete_layer=Supprimer le calque
layer_tool_menu_clear_layer=Effacer le calque

channel_tool_title=Canaux
channel_tool_fg=Premier plan
channel_tool_bg=Fond

font_tool_select_outline_button=Contour
font_tool_current_font_label=Police TDF actuelle
font_tool_no_font=<aucune>
font_tool_no_fonts_label=
    Aucune police tdf trouvée.
    Installez de nouvelles polices dans le répertoire des polices
font_tool_open_directory_button=Ouvrir le répertoire des polices

pipette_tool_char_code=Code { $code }
pipette_tool_foreground=Premier plan { $fg }
pipette_tool_background=Fond { $bg }
pipette_tool_keys=
    Maintenez shift pour sélectionner
    la couleur du premier plan

    Maintenez control pour sélectionner
    la couleur du fond

char_table_tool_title=Table des caractères
minimap_tool_title=Aperçu

no_document_selected=Aucun document sélectionné

undo-draw-ellipse=Dessiner une ellipse
undo-draw-rectangle=Dessiner un rectangle
undo-paint-brush=Pinceau
undo-pencil=Crayon
undo-eraser=Gomme
undo-bucket-fill=Remplissage au seau
undo-line=Ligne
undo-cut=Couper
undo-paste-glyph=Coller un glyphe
undo-bitfont-flip-y=Retourner Y
undo-bitfont-flip-x=Retourner X
undo-bitfont-move-down=Déplacer vers le bas
undo-bitfont-move-up=Déplacer vers le haut
undo-bitfont-move-left=Déplacer vers la gauche
undo-bitfont-move-right=Déplacer vers la droite
undo-bitfont-inverse=Inverser
undo-bitfont-clear=Effacer
undo-bitfont-edit=Modifier
undo-bitfont-resize=Redimensionner
undo-delete=Supprimer
undo-backspace=Retour arrière

undo-render_character=Rendre un caractère
undo-delete_character=Supprimer un caractère
undo-select=Sélectionner
undo-plugin=Plugin { $title }

font_selector-ansi_font=ANSI
font_selector-library_font=BIBLIOTHÈQUE
font_selector-file_font=FICHIER
font_selector-sauce_font=SAUCE

select-palette-dialog-title=Sélectionner une palette ({ $count } disponibles)
select-palette-dialog-builtin_palette=INCORPORÉE
select-palette-dialog-no-matching-palettes=Aucune palette ne correspond aux critères de recherche.

autosave-dialog-title=Sauvegarde automatique
autosave-dialog-description=Une sauvegarde automatique a été trouvée pour ce fichier.
autosave-dialog-question=Voulez-vous utiliser le fichier original ou charger la sauvegarde automatique?
autosave-dialog-load_autosave_button=Charger depuis la sauvegarde automatique
autosave-dialog-discard_autosave_button=Ignorer la sauvegarde automatique

paste_mode-description=Vous êtes maintenant en mode collage. Utilisez l'outil calque pour ajouter ou ancrer le calque.
paste_mode-stamp=Tampon
paste_mode-rotate=Tourner
paste_mode-flipx=Retourner X
paste_mode-flipy=Retourner Y
paste_mode-transparent=Transparent

ask_close_file_dialog-description=Voulez-vous enregistrer les modifications apportées à { $filename }?
ask_close_file_dialog-subdescription=Vos modifications seront perdues si vous ne les enregistrez pas.
ask_close_file_dialog-dont_save_button=Ne pas enregistrer
ask_close_file_dialog-save_button=Enregistrer

tab-context-menu-close=Fermer
tab-context-menu-close_others=Fermer les autres
tab-context-menu-close_all=Fermer tout
tab-context-menu-copy_path=Copier le chemin

font-view-char_label=Caractère
font-view-ascii_label=ASCII
font-view-font_label=Police
font-view-font_page_label=Page de police:

font-editor-tile_area=Zone de tuiles
font-editor-clear=Effacer
font-editor-inverse=Inverser
font-editor-flip_x=Retourner X
font-editor-flip_y=Retourner Y

animation_editor_path_label=Chemin:
animation_editor_export_button=Exporter
animation_editor_ansi_label=Animation
animation_encoding_frame=Encodage de la trame { $cur } de { $total }
animation_of_frame_count=de { $total }
animation_icy_play_note=Note: Pour lire l'animation dans la console/bbs ou la conversion ANSI, utilisez:

new-file-template-cp437-title=CP437 ANSI
new-file-template-cp437-description=
    Créez un nouveau fichier ANSI avec 16 couleurs DOS
    Limité à 16 couleurs DOS et police Sauce, a un clignotement (modifiable)
new-file-template-ice-title=CP437 Ice ANSI
new-file-template-ice-description=
    Créez un nouveau fichier ANSI avec 16 couleurs DOS en mode Ice
    Limité à 16 couleurs DOS et police Sauce, sans clignotement (modifiable)
new-file-template-xb-title=XB 16 Couleurs
new-file-template-xb-description=
    Créez un nouveau fichier XB
    Palette libre de 16 couleurs, 1 police, sans clignotement (modifiable)
new-file-template-xb-ext-title=Police étendue XB
new-file-template-xb-ext-description=
    Créez un nouveau fichier XB contenant deux polices
    Palette libre de 16 couleurs, 8 fg, 16 bg, 2 polices, sans clignotement
new-file-template-ansi-title=ANSI moderne
new-file-template-ansi-description=
    Créez un nouveau fichier ANSI sans restrictions
    Palette illimitée, multiples polices, clignotement
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Créez un nouveau fichier Atascii

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=Créer un nouveau fichier FILE_ID.DIZ
new-file-template-ansimation-title=Animation
new-file-template-ansimation-description=Créer un nouveau fichier d'animation ANSI
new-file-template-bit_font-title=Police de pixels
new-file-template-bit_font-description=Créer un nouveau fichier de police de pixels
new-file-template-color_font-title=Police de couleur TDF
new-file-template-color_font-description=Créer une nouvelle police de couleur TheDraw
new-file-template-block_font-title=Police de blocs TDF
new-file-template-block_font-description=Créer une nouvelle police de blocs TheDraw
new-file-template-outline_font-title=Police de contour TDF
new-file-template-outline_font-description=Créer une nouvelle police de contour TheDraw
new-file-template-ansimation-ui-label=
    Une animation IcyDraw est un fichier texte lua décrivant une séquence d'animation.
    Pour une description de la syntaxe, cliquez sur ce lien:
new-file-template-bitfont-ui-label=
    Une police de pixels est utilisée par les anciens ordinateurs pour afficher du texte.

new-file-template-thedraw-ui-label=
    Les polices TheDraw sont utilisées pour rendre du texte grand dans les éditeurs ANSI.
    TheDraw définit trois types de polices: Couleur, Bloc et Contour.

    Un grand fichier de polices peut être téléchargé depuis:

manage-font-dialog-title=Gérer les polices
manage-font-used_font_label=Polices utilisées
manage-font-copy_font_button=Copier la police
manage-font-copy_font_button-tooltip=Copie la police comme séquence ANSI CTerm dans le presse-papiers. (pour utilisation dans BBS)
manage-font-remove_font_button=Supprimer
manage-font-used_label=utilisée
manage-font-not_used_label=non utilisée
manage-font-replace_label=Remplacer l'utilisation par l'emplacement
manage-font-replace_font_button=Remplacer
manage-font-change_font_slot_button=Changer l'emplacement de la police

palette_selector-dos_default_palette=Palette VGA 16 couleurs
palette_selector-dos_default_low_palette=Palette VGA 8 couleurs
palette_selector-c64_default_palette=Couleurs C64
palette_selector-ega_default_palette=Palette EGA 64 couleurs
palette_selector-xterm_default_palette=Couleurs étendues XTerm
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Extrait du buffer

tdf-editor-outline_preview_label=Aperçu du glyphe contourné
tdf-editor-draw_bg_checkbox=Utiliser le fond
tdf-editor-clone_button=Cloner
tdf-editor-font_name_label=Nom de la police:
tdf-editor-spacing_label=Espacement:
tdf-editor-no_font_selected_label=Aucune police sélectionnée
tdf-editor-font_type_label=Type de police:
tdf-editor-font_type_color=Couleur
tdf-editor-font_type_block=Bloc
tdf-editor-font_type_outline=Contour
tdf-editor-clear_char_button=Effacer le caractère
tdf-editor-cheat_sheet_key=Touche
tdf-editor-cheat_sheet_code=Code
tdf-editor-cheat_sheet_res=Rés

settings-heading=Paramètres
settings-reset_button=Réinitialiser
settings-monitor-category=Moniteur
settings-char-set-category=Jeux de caractères
settings-font-outline-category=Contour de police
settings-markers-guides-category=Marqueurs et guides
settings-keybindings-category=Touches
settings-reference-alpha=Alpha de l'image de référence
settings-raster-label=Couleur de la grille:
settings-alpha=alpha
settings-guide-label=Couleur du guide:
settings-set-label=Jeu { $set }
settings-key_filter_preview_text=Filtrer les assignations de touches
settings-char_set_list_label=Jeux de caractères:

edit-tag-title=Étiquette
edit-tag-filter=Filtrer les étiquettes
edit-tag-preview-label=Aperçu:
edit-tag-replacement-label=Remplacement:
edit-tag-alignment-label=Alignement:
edit-tag-length-label=Longueur:
edit-tag-alignment-left=Gauche
edit-tag-alignment-right=Droite
edit-tag-alignment-center=Centré
edit-tag-placement-label=Placement:
edit-tag-placement-in_line=En ligne
edit-tag-placement-after=Avec GotoXY
edit-tag-role-label=Rôle:
edit-tag-role-displaycode=Code d'affichage
edit-tag-role-hyperlink=Hyperlien

add_tag_tooltip=Ajouter une étiquette
delete_tag_tooltip=Supprimer une étiquette

ask_unsaved_file_dialog-description=Voulez-vous enregistrer les modifications apportées au fichier suivant {
    $number ->
        [1] ?
        *[other] {$number} fichiers?
    }
ask_unsaved_file_dialog-subdescription=Vos modifications seront perdues si vous ne les enregistrez pas.
ask_unsaved_file_dialog-save_all_button=Tout enregistrer
ask_unsaved_file_dialog-dont_save_button=Ne pas enregistrer
