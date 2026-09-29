font-editor-table = Char table 0-{ $length }:

unsaved-title=Untitled

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
arg-host-backup-folder-help = Folder for autosave backups (default: current directory)
arg-host-interval-help = Autosave interval in minutes (default: 60, use 0 for shutdown-only saves)

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

# Server messages
server-error-unknown-format = Error: Unknown file format for '{ $path }'
server-starting-empty-canvas = Starting with empty 80x25 canvas
server-loaded = Loaded: { $path } ({ $cols }x{ $rows })
server-error-loading-file = Error loading file '{ $path }': { $error }
server-error-invalid-bind-address = Error: Invalid bind address '{ $addr }': { $error }
server-error-runtime = Failed to create Tokio runtime
server-error = Server error: { $error }

menu-file=&File
menu-new=New…
menu-open=Open…
menu-open_recent=Open Recent
menu-open_recent_clear=Clear
menu-no_recent_files=No Recent Files
menu-clear_recent_files=Clear Recent Files
menu-save=Save
menu-import=Import
menu-edit-sauce=Edit Sauce Info…
menu-9px-font=9px Font
menu-aspect-ratio=Legacy Aspect Ratio
menu-set-canvas-size=Set Canvas Size…
menu-file-settings=File Settings…
menu-import-fonts=Import Fonts…
menu-export-font=Export Font…

file-settings-dialog-title=File Settings
file-settings-canvas-size=Canvas Size
file-settings-font-dims=Font Cell Size
file-settings-font-height=Font Height
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
file-settings-format-legacy-dos=16 fixed colors, single font
file-settings-format-xbin=16 colors, custom palette, single font
file-settings-format-xbin-extended=8 fg colors, dual fonts, custom palette
file-settings-format-unrestricted=Full RGB colors, unlimited fonts
menu-close=Close
menu-save-as=Save As…
menu-connect-to-server=Connect to Server…
menu-close-editor=Close Editor
menu-quit-app=Quit
menu-toggle-chat=Toggle Chat Panel

# Connect to server dialog
collab-nickname=Nickname
collab-nickname-placeholder=Your name
collab-group=Group
collab-group-placeholder=Optional
collab-password=Password
collab-password-placeholder=Optional
collab-server-url=Server URL
collab-server-url-placeholder=host:port or ws://host:port
collab-connect-button=Connect
button-cancel=Cancel

# Collaboration connection errors
collab-connection-lost-title=Collaboration disconnected
collab-connection-lost-message=The connection to the collaboration server was lost.
collab-connection-error-title=Collaboration connection error
collab-connection-error-message=The connection to the collaboration server failed.

    Error: { $error }

# Collaboration chat panel
collab-user-joined={ $nick } has joined
collab-user-joined-group={ $nick } <{ $group }> has joined
collab-user-left={ $nick } has left
collab-user-left-group={ $nick } <{ $group }> has left
collab-no-other-users=No other users
collab-you=You
collab-guest=Guest
collab-no-messages=No messages yet
collab-type-message=Type a message...
collab-yesterday=yesterday
collab-days-ago={ $days }d ago

# Collaboration user status
collab-status-active=Active
collab-status-idle=Idle
collab-status-away=Away
collab-status-web=Web

# File dialogs
file-dialog-save-as-title=Save File As
file-dialog-filter-all-files=All Files
file-dialog-filter-icydraw-files=IcyDraw Files
file-dialog-filter-font-files=Font Files
file-dialog-filter-tdf-files=TDF Files
file-dialog-filter-animation-files=Animation Files
menu-export=Export…
menu-edit-font-outline=Font Outline…
menu-show_settings=Settings…
menu-set-font-size=Set Font Size…

font-size-width=Width
font-size-height=Height

menu-edit=&Edit
menu-undo=Undo
menu-redo=Redo
menu-undo-op=Undo: { $op }
menu-redo-op=Redo: { $op }

menu-cut=Cut
menu-copy=Copy
menu-paste=Paste
menu-delete=Delete
menu-rename=Rename
menu-paste-as=Paste as
menu-paste-as-new-image=New image
menu-paste-as-sixel=Sixel layer
menu-paste-as-brush=Brush
menu-insert-sixel-from-file=Insert Sixel from File…
menu-erase=Erase
menu-flipx=Flip X
menu-flipy=Flip Y
menu-justifyleft=Justify Left
menu-justifyright=Justify Right
menu-justifycenter=Center
menu-crop=Crop
menu-justify_line_center=Center Line
menu-justify_line_left=Left Justify Line
menu-justify_line_right=Right Justify Line
menu-insert_row=Insert Row
menu-delete_row=Delete Row
menu-insert_colum=Insert Column
menu-delete_colum=Delete Column
menu-erase_row=Erase Row
menu-erase_row_to_start=Erase Row to Start
menu-erase_row_to_end=Erase Row to End
menu-erase_column=Erase Column
menu-erase_column_to_start=Erase Column to Start
menu-erase_column_to_end=Erase Column to End
menu-scroll_area_up=Scroll Area Up
menu-scroll_area_down=Scroll Area Down
menu-scroll_area_left=Scroll Area Left
menu-scroll_area_right=Scroll Area Right
menu-mirror_mode=Mirror Mode
menu-area_operations=Area

menu-selection=&Selection
menu-select-all=Select All
menu-select_nothing=Deselect
menu-inverse_selection=Inverse

menu-colors=&Colors

menu-edit_palette=Edit Palette…
menu-next_fg_color=Next Foreground Color
menu-next_bg_color=Next Background Color
menu-prev_fg_color=Previous Foreground Color
menu-prev_bg_color=Previous Background Color

menu-view=&View
menu-reference-image=Open Reference Image…
menu-toggle-reference-image=Toggle Reference Image
menu-clear-reference-image=Clear
menu-toggle_fullscreen=Fullscreen
menu-zoom=Zoom
menu-zoom_reset=Revert Zoom
menu-zoom_in=Zoom In
menu-zoom_out=Zoom Out
menu-guides=Guides
menu-raster=Grid
menu-zoom-fit_size=Fit Size
menu-show_layer_borders=Show Layer Borders
menu-show_line_numbers=Show Line Numbers
menu-toggle_guide=Toggle Guides
menu-toggle_raster=Toggle Grid

menu-pick_attribute_under_caret=Pick up Attribute
menu-default_color=Default Color
menu-toggle_color=Switch Foreground/Background

menu-fonts=Fonts
menu-open_font_selector=Select Font…
menu-open_font_slot_manager=Manage Font Slots…
menu-add_fonts=Add Fonts…
menu-open_font_manager=Edit Buffer Fonts…
menu-open_font_directoy=Open Text-Art Fonts Directory…

# Font Selector Dialog
font-selector-title-single=Select Font
font-selector-title-dual=Select Fonts (2 Slots)
font-selector-title-full=Font Manager
font-selector-available=available
font-selector-filter-placeholder=Filter fonts…
font-selector-sauce=SAUCE
font-selector-ansi=ANSI
font-selector-library=Library
font-selector-document=Document
font-selector-no-fonts-installed=No fonts installed
font-selector-no-fonts-match=No fonts match the filter
font-selector-no-selection=No font selected
font-selector-sauce-fonts=SAUCE Fonts
font-selector-ansi-fonts=ANSI Fonts
font-selector-preview-sample=ABCDEFGHIJ abcdefghij 0123456789 !@#$%

# Set Font Dialog
set-font-load-font=Load Font…
set-font-filter-fonts=Font files
set-font-filter-all=All files
set-font-load-error=Error loading font
set-font-xbin-select-title=Select font from XBin file
set-font-xbin-font=Font { $slot }
set-font-xbin-no-fonts=No fonts found in XBin file

menu-help=&Help
menu-discuss=Discuss
menu-open_log_file=Open log file
menu-report-bug=Report Bug
menu-about=About…
menu-plugins=&Plugins
menu-no_plugins=No plugins found

menu-upgrade_version=Upgrade to { $version }
menu-update-available=⬆ Update available: { $version }

# Plugin errors
error-plugin-title=Plugin Error

# BitFont Editor menu items
menu-clear-glyph=Clear Glyph
menu-inverse-glyph=Inverse Glyph
menu-fill-glyph=Fill Glyph
menu-flip-x=Flip Horizontal
menu-flip-y=Flip Vertical
menu-deselect=Deselect

# Selection Commands
cmd-select-none-action = Deselect
cmd-select-none-desc = Clear the current selection
cmd-select-none-menu = Deselect

cmd-select-inverse-action = Inverse Selection
cmd-select-inverse-desc = Inverse the current selection
cmd-select-inverse-menu = Inverse Selection

cmd-select-erase-action = Erase
cmd-select-erase-desc = Erase the selected area
cmd-select-erase-menu = Erase

cmd-select-flip_x-action = Flip X
cmd-select-flip_x-desc = Flip selection horizontally
cmd-select-flip_x-menu = Flip X

cmd-select-flip_y-action = Flip Y
cmd-select-flip_y-desc = Flip selection vertically
cmd-select-flip_y-menu = Flip Y

cmd-select-crop-action = Crop
cmd-select-crop-desc = Crop to selection
cmd-select-crop-menu = Crop

cmd-select-justify_left-action = Justify Left
cmd-select-justify_left-desc = Align selection to the left
cmd-select-justify_left-menu = Justify Left

cmd-select-justify_center-action = Center
cmd-select-justify_center-desc = Center the selection
cmd-select-justify_center-menu = Center

cmd-select-justify_right-action = Justify Right
cmd-select-justify_right-desc = Align selection to the right
cmd-select-justify_right-menu = Justify Right

# Area Operations Commands
cmd-area-justify_line_left-action = Justify Line Left
cmd-area-justify_line_left-desc = Left justify current line
cmd-area-justify_line_left-menu = Justify Line Left

cmd-area-justify_line_center-action = Center Line
cmd-area-justify_line_center-desc = Center current line
cmd-area-justify_line_center-menu = Center Line

cmd-area-justify_line_right-action = Justify Line Right
cmd-area-justify_line_right-desc = Right justify current line
cmd-area-justify_line_right-menu = Justify Line Right

cmd-area-insert_row-action = Insert Row
cmd-area-insert_row-desc = Insert a new row at cursor position
cmd-area-insert_row-menu = Insert Row

cmd-area-delete_row-action = Delete Row
cmd-area-delete_row-desc = Delete the row at cursor position
cmd-area-delete_row-menu = Delete Row

cmd-area-insert_column-action = Insert Column
cmd-area-insert_column-desc = Insert a new column at cursor position
cmd-area-insert_column-menu = Insert Column

cmd-area-delete_column-action = Delete Column
cmd-area-delete_column-desc = Delete the column at cursor position
cmd-area-delete_column-menu = Delete Column

cmd-area-erase_row-action = Erase Row
cmd-area-erase_row-desc = Erase the entire row at cursor
cmd-area-erase_row-menu = Erase Row

cmd-area-erase_row_to_start-action = Erase Row to Start
cmd-area-erase_row_to_start-desc = Erase from start of row to cursor
cmd-area-erase_row_to_start-menu = Erase Row to Start

cmd-area-erase_row_to_end-action = Erase Row to End
cmd-area-erase_row_to_end-desc = Erase from cursor to end of row
cmd-area-erase_row_to_end-menu = Erase Row to End

cmd-area-erase_column-action = Erase Column
cmd-area-erase_column-desc = Erase the entire column at cursor
cmd-area-erase_column-menu = Erase Column

cmd-area-erase_column_to_start-action = Erase Column to Start
cmd-area-erase_column_to_start-desc = Erase from start of column to cursor
cmd-area-erase_column_to_start-menu = Erase Column to Start

cmd-area-erase_column_to_end-action = Erase Column to End
cmd-area-erase_column_to_end-desc = Erase from cursor to end of column
cmd-area-erase_column_to_end-menu = Erase Column to End

cmd-area-scroll_up-action = Scroll Area Up
cmd-area-scroll_up-desc = Scroll selected area up
cmd-area-scroll_up-menu = Scroll Area Up

cmd-area-scroll_down-action = Scroll Area Down
cmd-area-scroll_down-desc = Scroll selected area down
cmd-area-scroll_down-menu = Scroll Area Down

cmd-area-scroll_left-action = Scroll Area Left
cmd-area-scroll_left-desc = Scroll selected area left
cmd-area-scroll_left-menu = Scroll Area Left

cmd-area-scroll_right-action = Scroll Area Right
cmd-area-scroll_right-desc = Scroll selected area right
cmd-area-scroll_right-menu = Scroll Area Right

# BitFont Editor Commands
cmd-bitfont-clear-action = Clear
cmd-bitfont-clear-desc = Clear the current glyph
cmd-bitfont-clear-menu = Clear

cmd-bitfont-fill-action = Fill
cmd-bitfont-fill-desc = Fill the current selection
cmd-bitfont-fill-menu = Fill

cmd-bitfont-inverse-action = Inverse
cmd-bitfont-inverse-desc = Inverse the current glyph
cmd-bitfont-inverse-menu = Inverse

cmd-bitfont-flip_x-action = Flip X
cmd-bitfont-flip_x-desc = Flip horizontally
cmd-bitfont-flip_x-menu = Flip Horizontal

cmd-bitfont-flip_y-action = Flip Y
cmd-bitfont-flip_y-desc = Flip vertically
cmd-bitfont-flip_y-menu = Flip Vertical

cmd-bitfont-toggle_letter_spacing-action = 8/9-Dot Mode
cmd-bitfont-toggle_letter_spacing-desc = Toggle 9-dot cell mode (VGA letter spacing)
cmd-bitfont-toggle_letter_spacing-menu = 8/9-Dot Cell Mode

cmd-bitfont-swap_chars-action = Swap Characters
cmd-bitfont-swap_chars-desc = Swap selected character with character at cursor
cmd-bitfont-swap_chars-menu = Swap Characters

cmd-bitfont-duplicate_line-action = Duplicate Line
cmd-bitfont-duplicate_line-desc = Duplicate current line in all glyphs
cmd-bitfont-duplicate_line-menu = Duplicate Line

cmd-bitfont-show_preview-action = Show Preview
cmd-bitfont-show_preview-desc = Show font preview screen
cmd-bitfont-show_preview-menu = Show Preview

cmd-category-bitfont = BitFont
cmd-category-selection = Selection
cmd-category-area = Area Operations

# BitFont Tools menu
menu-tools=Tools
menu-tool-click=Click Tool
menu-tool-select=Selection Tool
menu-tool-rectangle=Rectangle Tool
menu-tool-fill=Fill Tool
menu-next-char=Next Character
menu-prev-char=Previous Character

tool-fg=Fg
tool-bg=Bg
tool-solid=Solid
tool-character=Character
tool-shade=Shade
tool-colorize=Colorize
tool-size-label=Size
tool-full-block=Block
tool-half-block=Half Block
tool-outline=Outline
tool-custom-brush=Custom brush

tool-select-label=Selection mode
tool-select-normal=Rectangle
tool-select-character=Character
tool-select-attribute=Attribute
tool-select-foreground=Foreground
tool-select-background=Background
tool-select-description=Hold shift to add to a selection. Control/Cmd to remove.

tool-fill-exact_match_label=Exact match
tool-flip_horizontal=Horizontal
tool-flip_vertical=Vertical

tool-paint_brush_name=Paint Brush
tool-paint_brush_tooltip=Paint strokes using a brush
tool-click_name=Text input
tool-click_tooltip=Input text & rectangular selections
tool-ellipse_name=Ellipse
tool-ellipse_tooltip=Draw ellipse
tool-filled_ellipse_name=Filled ellipse
tool-filled_ellipse_tooltip=Draw filled ellipse
tool-rectangle_name=Rectangle
tool-rectangle_tooltip=Draw rectangle
tool-filled_rectangle_name=Filled rectangle
tool-filled_rectangle_tooltip=Draw filled rectangle
tool-eraser_name=Eraser
tool-eraser_tooltip=Erase to background using a brush
tool-fill_name=Fill
tool-fill_tooltip=Fill area with color or char
tool-flip_name=Switcher
tool-flip_tooltip=Switch vertical or horizontal half blocks
tool-tdf_name=The Draw Fonts
tool-tdf_tooltip=Text input using The Draw Fonts
tool-line_name=Draw line
tool-line_tooltip=Draw lines
tool-move_layer_name=Move Layer
tool-move_layer_tooltip=Move layers
tool-pencil_name=Pencil
tool-pencil_tooltip=Paint strokes using a pencil
tool-pipette_name=Color picker
tool-pipette_tooltip=Pick up a color
tool-select_name=Select Tool
tool-select_tooltip=Mutliple and non rectangular selections
tool-tag_name=Tag Tool
tool-tag_tooltip=Tags are used to expand strings in output
tool-tag_show=Show Tags
tool-tag_edit_button={ menu-edit }

toolbar-new=New

new-file-title=New File
new-file-width=Width
new-file-height=Height
new-file-ok=Ok
new-file-cancel=Cancel
new-file-create=Create

edit-sauce-title=Edit Sauce Info
edit-sauce-title-label=Title
edit-sauce-title-label-length=(35 chars)
edit-sauce-author-label=Author
edit-sauce-author-label-length=(20 chars)
edit-sauce-group-label=Group
edit-sauce-group-label-length=(20 chars)
edit-sauce-comments-label=Comments (64 chars in line limit)
edit-sauce-letter-spacing=Use 9px mode
edit-sauce-aspect-ratio=Simulate classic aspect ratio

edit-canvas-size-title=Set Canvas Size
edit-canvas-size-width-label=Width
edit-canvas-size-height-label=Height
edit-canvas-size-resize=Resize
edit-canvas-size-resize_layers-label=Resize layers

toolbar-size = { $colums ->
     [1] 1 Column
*[other] { $colums } Columns
} x { $rows ->
     [1] 1 Row
*[other] { $rows } Rows
}

toolbar-position = Ln { $line }, Col { $column }
toolbar-layer_offset = Layer offset: { $line }x{ $column }
add_layer_tooltip = Add new layer
move_layer_up_tooltip = Move layer up
move_layer_down_tooltip = Move layer down
delete_layer_tooltip = Delete layer
anchor_layer_tooltip = Anchor layer

pipette-foreground = FG { $index }
pipette-background = BG { $index }
pipette-hover_hint = Hover over canvas to pick colors
pipette-help = ⇧: FG only   ⌃: BG only

# Font Tool
font-tool-no_fonts = No text-art fonts installed
font-tool-open_directory = Open Text-Art Fonts Directory
font-tool-select_font = Select Font…
font-tool-outline = Outline:
font-tool-outline_normal = Normal
font-tool-outline_round = Round
font-tool-outline_square = Square
font-tool-outline_shadow = Shadow
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
tdf-font-selector-filter_placeholder = Filter fonts…
tdf-font-selector-type_outline = Outline
tdf-font-selector-type_block = Block
tdf-font-selector-type_color = Color
tdf-font-selector-type_figlet = Figlet
tdf-font-selector-font_count = { $count } fonts
tdf-font-selector-export = Export…
tdf-font-selector-export_title = Export Font

glyph-char-label=Char
glyph-font-label=Font

color-is_blinking=Blink

export-title=Export
export-button-title=Export
export-file-label=File name:
export-path-label=Path:
export-video-preparation-label=Video Preparation:
export-video-preparation-None=None
export-video-preparation-Clear=Clear Screen
export-video-preparation-Home=Home Cursor
export-utf8-output-label=Modern terminal format (utf8)
export-save-sauce-label=Save sauce info
export-compression-label=Compress output
export-limit-output-line-length-label=Limit output line length
export-maximum_line_length=Maximum line length
export-use_repeat_sequences=Use CSI Pn b repeat sequences
export-save_full_line_length=Save trailing white spaces
export-format-label=Format:

select-character-title=Select Character

select-outline-style-title=Outline Font Style Type

about-dialog-title=About Icy Draw
about-dialog-heading = Icy Draw
about-dialog-description = 
    Icy Draw is a tool for creating ANSI and ASCII art.
    It is written in Rust and uses the EGUI library.

    Icy Draw is free software, licensed under the Apache 2 license.
    Source code is available at www.github.com/mkrueger/icy_draw
about-dialog-created_by =
    Created by { $authors }
    Help & testing: NuSkooler, Grymmjack

edit-layer-dialog-title=Layer properties
edit-layer-dialog-name-label=Name
edit-layer-dialog-is-visible-checkbox=Visible
edit-layer-dialog-is-edit-locked-checkbox=Edit locked
edit-layer-dialog-is-position-locked-checkbox=Position locked
edit-layer-dialog-is-x-offset-label=X offset
edit-layer-dialog-is-y-offset-label=Y offset
edit-layer-dialog-has-alpha-checkbox=Has alpha
edit-layer-dialog-is-alpha-locked-checkbox=Alpha locked

error-load-file=Error loading file: { $error }

select-font-dialog-title=Select Font ({ $fontcount} available)
add-font-dialog-title=Add Font ({ $fontcount} available)
select-font-dialog-select=Select
add-font-dialog-select=Add
select-font-dialog-filter-text=Filter fonts
select-font-dialog-no-fonts=No fonts matches the filter
select-font-dialog-no-fonts-installed=No fonts installed
select-font-dialog-color-font=COLOR
select-font-dialog-block-font=BLOCK
select-font-dialog-outline-font=OUTLINE
select-font-dialog-figlet-font=FIGLET
select-font-dialog-preview-text=HELLO
select-font-dialog-edit-button=Edit font…

layer_tool_title=Layers
layer_tool_menu_layer_properties=Layer properties
layer_tool_menu_resize_layer=Resize layer
layer_tool_menu_new_layer=New layer
layer_tool_menu_duplicate_layer=Duplicate layer
layer_tool_menu_merge_layer=Merge layer
layer_tool_menu_delete_layer=Delete layer
layer_tool_menu_clear_layer=Clear layer

channel_tool_title=Channels
channel_tool_fg=Foreground
channel_tool_bg=Background

font_tool_select_outline_button=Outline
font_tool_current_font_label=Current TDF Font
font_tool_no_font=<none>
font_tool_no_fonts_label=
    No tdf fonts found.
    Install new fonts in the font directory
font_tool_open_directory_button=Open text-art fonts directory

pipette_tool_char_code=Code { $code }
pipette_tool_foreground=Foreground { $fg }
pipette_tool_background=Background { $bg }
pipette_tool_keys=
    Hold shift to pick up
    foreground color

    Hold control to pick up
    background color

char_table_tool_title=Char table
minimap_tool_title=Preview

no_document_selected=No document selected

undo-plugin=Plugin: { $title }
undo-draw-ellipse=Draw ellipse
undo-draw-rectangle=Draw rectangle
undo-paint-brush=Paintbrush
undo-pencil=Pencil
undo-eraser=Eraser
undo-bucket-fill=Bucket fill
undo-line=Line
undo-cut=Cut
undo-paste-glyph=Paste glyph
undo-bitfont-flip-y=Flip Y
undo-bitfont-flip-x=Flip X
undo-bitfont-move-down=Move down
undo-bitfont-move-up=Move up
undo-bitfont-move-left=Move left
undo-bitfont-move-right=Move right
undo-bitfont-inverse=Inverse
undo-bitfont-clear=Clear
undo-bitfont-edit=Edit
undo-bitfont-resize=Resize
undo-bitfont-insert-line=Insert line
undo-bitfont-delete-line=Delete line
undo-bitfont-insert-column=Insert column
undo-bitfont-delete-column=Delete column
undo-bitfont-swap-chars=Swap characters
undo-bitfont-duplicate-line=Duplicate line
undo-delete=Delete
undo-backspace=Backspace

undo-render_character=Render character
undo-delete_character=Delete character
undo-select=Select
undo-animation-edit=Edit

font_selector-ansi_font=ANSI
font_selector-library_font=LIBRARY
font_selector-file_font=FILE
font_selector-sauce_font=SAUCE

select-palette-dialog-title=Select Palette ({ $count } available)
select-palette-dialog-builtin_palette=BUILTIN
select-palette-dialog-no-matching-palettes=No palettes found matching search critearia.

palette-editor-import=Import…
palette-editor-export=Export…
palette-editor-invalid-hex=Invalid hex color

autosave-dialog-title=Autosave
autosave-dialog-description=Found an autosave for this file.
autosave-dialog-question=Do you want to use the original file, or load the autosave?
autosave-dialog-load_autosave_button=Load from autosave
autosave-dialog-discard_autosave_button=Discard autosave

paste_mode-description=You're now in paste mode. Use layer tool to add or anchor the layer.
paste_mode-stamp=Stamp
paste_mode-rotate=Rotate
paste_mode-flipx=Flip X
paste_mode-flipy=Flip Y
paste_mode-transparent=Transparent

ask_close_file_dialog-description=Do you want to save the changes you made to { $filename }?
ask_close_file_dialog-subdescription=Your changes will be lost if you don't save them.
ask_close_file_dialog-dont_save_button=Don't save
ask_close_file_dialog-save_button=Save

tab-context-menu-close=Close
tab-context-menu-close_others=Close others
tab-context-menu-close_all=Close all
tab-context-menu-copy_path=Copy path

font-view-char_label=Char
font-view-ascii_label=ASCII
font-view-font_label=Font
font-view-font_page_label=Font Page:

font-editor-tile_area=Tile area
font-editor-clear=Clear
font-editor-inverse=Inverse
font-editor-flip_x=Flip X
font-editor-flip_y=Flip Y

animation_editor_path_label=Path:
animation_editor_export_button=Export
animation_editor_ansi_label=Ansimation
animation_encoding_frame=Encoding frame { $cur } of { $total }
animation_of_frame_count=of { $total }
animation_icy_play_note=Note: For playing the animation in the console/bbs or ansi conversion use:

new-file-template-cp437-title=CP437 ANSI
new-file-template-cp437-description=
    Create a new DOS 16 color ANSI file
    Limited to 16 DOS colors and Sauce font, has blink (can be switched)
new-file-template-ice-title=CP437 Ice ANSI
new-file-template-ice-description=
    Create a new DOS 16 color ice ANSI file
    Limited to 16 DOS colors and Sauce font, no blink (can be switched)
new-file-template-xb-title=XB 16 Colors
new-file-template-xb-description=
    Create a new XB file
    Free 16 color palette, 1 font, no blink (can be switched)
new-file-template-xb-ext-title=XB Extended Font
new-file-template-xb-ext-description=
    Create a new XB file containing two fonts
    Free 16 color palette, 8 fg, 16 bg, 2 fonts, no blink
new-file-template-ansi-title=Modern ANSI
new-file-template-ansi-description=
    Create a new Ansi file without restrictions
    Unlimited palette, multiple fonts, blink
new-file-template-atascii-title=Atascii
new-file-template-atascii-description=
    Create a new Atascii file

new-file-template-file_id-title=FILE_ID.DIZ
new-file-template-file_id-description=Create a new FILE_ID.DIZ file
new-file-template-ansimation-title=Ansimation
new-file-template-ansimation-description=Create a new ansi animation file
new-file-template-bit_font-title=Bit Font
new-file-template-bit_font-description=Create a new bit font file

new-file-category-ansi=ANSI Art
new-file-category-fonts=Fonts
new-file-category-animation=Animation

new-file-editor-ansi=ANSI Art
new-file-editor-bitfont=Bit Font
new-file-editor-tdf=TDF Font
new-file-editor-animation=Animation
new-file-size-section=Dimensions

new-file-template-color_font-title=TDF Color Font
new-file-template-color_font-description=Create a new TheDraw color font
new-file-template-block_font-title=TDF Block Font
new-file-template-block_font-description=Create a new TheDraw block font
new-file-template-outline_font-title=TDF Outline Font
new-file-template-outline_font-description=Create a new TheDraw outline font
new-file-template-ansimation-ui-label=
    An IcyDraw ansimation is a lua text file describing an animation sequence.
    For a syntax description click this link:
new-file-template-bitfont-ui-label=
    A bitfont is used by legacy computers to display text.

new-file-template-thedraw-ui-label=
    TheDraw fonts are used to render larger text in ANSI editors.
    TheDraw defined three font types: Color, Block and Outline. 

    A big font archive can be downloaded from:

manage-font-dialog-title=Manage Fonts
manage-font-used_font_label=Used Fonts
manage-font-copy_font_button=Copy Font
manage-font-copy_font_button-tooltip=Copies font as CTerm ANSI sequence to clipboard. (for BBS use)
manage-font-remove_font_button=Remove
manage-font-used_label=used
manage-font-not_used_label=not used
manage-font-replace_label=Replace usage with slot
manage-font-replace_font_button=Replace
manage-font-change_font_slot_button=Change font slot

palette_selector-dos_default_palette=VGA 16 colors
palette_selector-dos_default_low_palette=VGA 8 colors
palette_selector-c64_default_palette=C64 colors
palette_selector-ega_default_palette=EGA 64 colors
palette_selector-xterm_default_palette=XTerm extended colors
palette_selector-viewdata_default_palette=Viewdata
palette_selector-extracted_from_buffer_default_label=Extracted from buffer

tdf-editor-outline_preview_label=Outline glyph preview
tdf-editor-draw_bg_checkbox=Use background
tdf-editor-clone_button=Clone
tdf-editor-font_name_label=Font Name:
tdf-editor-spacing_label=Spacing:
tdf-editor-no_font_selected_label=No font selected
tdf-editor-font_type_label=Font Type:
tdf-editor-font_type_color=Color
tdf-editor-font_type_block=Block
tdf-editor-font_type_outline=Outline
tdf-editor-clear_char_button=Clear Char
tdf-editor-cheat_sheet_key=Key
tdf-editor-cheat_sheet_code=Code
tdf-editor-cheat_sheet_res=Res
tdf-editor-outline_style_label=Style

tdf-dialog-add-font-title=Add Font
tdf-dialog-edit-settings-title=Edit Font Settings
tdf-dialog-font-type=Font Type:
tdf-dialog-font-name=Name:
tdf-dialog-spacing=Spacing:

settings-heading=Settings
settings-reset_button=Reset
settings-monitor-category=Monitor
settings-char-set-category=Character Sets
settings-font-outline-category=Font Outline
settings-paths-category=Paths
settings-markers-guides-category=Markers & Guides
settings-keybindings-category=Keys
settings-font-outline-header=Outline Style
settings-charset-header=F-Key Character Set
settings-charset-set=Set
settings-paths-header=Paths
settings-paths-config-dir=Config dir:
settings-paths-config-file=Config file:
settings-paths-log-file=Log file:
settings-paths-font-dir=Text-Art Fonts:
settings-paths-plugin-dir=Plugin dir:
settings-paths-taglists-dir=Taglists:
settings-paths-open=Open
settings-reference-alpha=Reference image alpha
settings-raster-label=Grid color:
settings-alpha=alpha
settings-guide-label=Guide color:
settings-set-label=Set { $set }
settings-key_filter_preview_text=Filter key bindings
settings-char_set_list_label=Character sets:

edit-tag-title=Tag
edit-tag-filter=Filter Tags
edit-tag-preview-label=Preview:
edit-tag-replacement-label=Replacement:
edit-tag-alignment-label=Alignment:
edit-tag-length-label=Length:
edit-tag-alignment-left=Left
edit-tag-alignment-right=Right
edit-tag-alignment-center=Center
edit-tag-placement-label=Placement:
edit-tag-placement-in_line=In Line
edit-tag-placement-after=With GotoXY
edit-tag-role-label=Role:
edit-tag-role-displaycode=Display code
edit-tag-role-hyperlink=Hyperlink

add_tag_tooltip=Add Tag
delete_tag_tooltip=Delete Tag
clone_tag_tooltip=Clone Tag 

ask_unsaved_file_dialog-description=Do you want to save the changes to the following {
    $number ->
        [1] file?
        *[other] {$number} files?
    }
ask_unsaved_file_dialog-subdescription=Your changes will be lost if you don't save them.
ask_unsaved_file_dialog-save_all_button=Save All
ask_unsaved_file_dialog-dont_save_button=Don't Save

# Save Changes Dialog (single file)
save-changes-title=Save changes to "{ $filename }"?
save-changes-description=Your changes will be lost if you don't save them.

# Paste Tool Toolbar
paste-tool-stamp=Stamp (S)
paste-tool-rotate=Rotate (R)
paste-tool-flip-x=Flip X
paste-tool-flip-y=Flip Y
paste-tool-transparent=Transparent (T)
paste-tool-hint=Enter: Anchor | Esc: Cancel | Arrows: Move
paste-image-title=Paste Image
paste-image-hint=Enter: Anchor | Esc: Cancel | Arrows: Move

# Font Import Dialog
menu-import-font=Import Font…
font-import-file=Import File
font-import-file-placeholder=Select a font file…
font-import-browse=Browse…
font-import-button=Import
font-import-preview=Preview
font-import-no-preview=No preview available
font-import-native-info=Native font file
font-import-xb-info=XBin file with embedded font(s)
font-import-xb-font-1=Font 1
font-import-xb-font-2=Font 2
font-import-select-font=Select Font:
font-import-image-info=Image will be converted to a 16×16 character grid
font-import-dithering=Use Dithering

# Font Export Dialog
menu-export-font=Export Font…
font-export-format=Format
font-export-com-format=COM Type
font-export-path=Export to
font-export-no-path=No path selected
font-export-button=Export
font-export-copy-button=Copy to Clipboard
font-export-ansi-dcs-info=Will be copied to clipboard as CTerm DCS sequence

# Animation Export Dialog
animation-export-format=Format
animation-export-path=Export to
animation-export-no-path=No path selected
animation-export-success=Export completed successfully
animation-export-exporting-frame=Exporting frame { $current } / { $total }
animation-export-encoding=Encoding video…
animation-export-cancelled=Export cancelled
animation-export-no-frames=No frames to export
animation-export-failed=Export failed: { $error }

# Animation Editor
animation-compiling=Compiling script…
animation-no-frames=No frames generated
animation-preparing=Preparing preview…
animation-no-log=No log entries
animation-frame-display=Frame { $current } / { $total }

# Reference Image Dialog
reference-image-dialog-title=Reference Image
reference-image-path=Path
reference-image-path-placeholder=Select an image file…
reference-image-browse=Browse…
reference-image-alpha=Opacity
reference-image-info=The reference image will be displayed as an overlay on the canvas.
reference-image-clear=Clear

# Tag Tool Toolbar
tag-toolbar-add=Add
tag-toolbar-tags=Tags…
tag-toolbar-edit=Edit
tag-toolbar-delete=Delete
tag-toolbar-delete-selected=Delete { $count } Selected
tag-toolbar-selected-tags={ $count } tags selected
tag-toolbar-no-replacement=(no replacement)
tag-toolbar-add-hint=Click to place tag, ESC to cancel

# Tag List Dialog
tag-list-title=Tags
tag-list-preview=Preview
tag-list-pos=Pos
tag-list-placement=Placement
tag-list-replacement=Replacement
tag-list-no-tags=No tags
tag-list-in-text=In text
tag-list-with-gotoxy=With GotoXY

# Tag Edit Dialog
tag-edit-preview=Preview
tag-edit-replacement=Replacement
tag-edit-position=Position
tag-edit-filter=Filter…

# Tool / brush discoverability hints (status bar)
tool-hint-click=Click  •  Type characters or drag a rectangular selection
tool-hint-select=Select  •  Drag to select, Shift to add, Alt to subtract
tool-hint-pencil=Pencil  •  { $brush }
tool-hint-line=Line  •  { $brush }
tool-hint-rectangle=Rectangle  •  { $brush }
tool-hint-filled_rectangle=Filled rectangle  •  { $brush }
tool-hint-ellipse=Ellipse  •  { $brush }
tool-hint-filled_ellipse=Filled ellipse  •  { $brush }
tool-hint-pipette=Color picker  •  Click to sample fg/bg/char
tool-hint-fill=Fill  •  { $brush }
tool-hint-font=Font  •  Place a TDF/Figlet caret, then type
tool-hint-tag=Tag  •  Click to place an expandable tag
tool-hint-paste=Paste  •  Click to commit, Esc to cancel
brush-hint-char=Char mode — paints '{ $ch }'
brush-hint-half_block=Half-block mode (2× vertical resolution)
brush-hint-shading=Shading mode (LMB lighter, RMB darker)
brush-hint-replace=Replace mode (recolors existing characters)
brush-hint-blink=Blink mode (toggles blink attribute)
brush-hint-colorize=Colorize mode (changes only colors)

## egui editor
menu-document=&Document
menu-keyboard-shortcuts=Keyboard Shortcuts…
menu-github=Icy Draw on GitHub
menu-disconnect=Disconnect
menu-new-window=New Window
menu-insert-image=Insert Image from File…
menu-area-operations=Area Operations
menu-edit-bitmap-font=Edit Bitmap Font…
menu-zoom-fit_window=Fit to Window
menu-zoom-fit_width=Fit Width
menu-off=Off
menu-show-guide=Show Guide
menu-show-raster=Show Grid
menu-character-grid=Character Grid
menu-side-panel=Side Panel
menu-chat-panel=Chat Panel
menu-appearance=Appearance
menu-character-table=Character Table…
menu-monitor-settings=Monitor Settings…
menu-run-script=Run Lua Script…
menu-reload-plugins=Reload Plugins
error-bitmap-font-width=The bitmap font editor currently supports glyphs up to 8 pixels wide.
error-load-image=Could not load { $path }: { $error }
error-load-reference-image=Could not load reference image { $path }
shortcuts-dialog-title=Keyboard Shortcuts
shortcut-new-document=New document
shortcut-open=Open
shortcut-save=Save
shortcut-save-as=Save as
shortcut-export=Export
shortcut-new-window=New window
shortcut-quit=Quit
shortcut-undo=Undo
shortcut-redo=Redo
shortcut-cut=Cut
shortcut-copy=Copy
shortcut-paste=Paste as floating layer
shortcut-select-all=Select all
shortcut-deselect=Deselect
shortcut-invert-selection=Invert selection
shortcut-erase-selection=Erase selection
shortcut-cancel-stroke=Cancel stroke / clear selection
shortcut-zoom-in=Zoom in
shortcut-zoom-out=Zoom out
shortcut-zoom-fit=Fit to window
shortcut-zoom-actual=Actual size (100%)
shortcut-zoom=Zoom
shortcut-toggle-grid=Toggle character grid
shortcut-toggle-line-numbers=Toggle line numbers
shortcut-toggle-guide=Toggle guide
shortcut-reference-image=Reference image
shortcut-toggle-reference-image=Toggle reference image
shortcut-toggle-side-panel=Toggle side panel
shortcut-fullscreen=Fullscreen
shortcut-next-fg=Next foreground color
shortcut-prev-fg=Previous foreground color
shortcut-next-bg=Next background color
shortcut-prev-bg=Previous background color
shortcut-pick-attribute=Pick attribute under caret
shortcut-swap-colors=Swap foreground / background
shortcut-group-text=Text Cursor Tool
shortcut-fkey-type=Type character from the active F-key set
shortcut-fkey-set-low=Choose F-key set 1–10
shortcut-fkey-set-high=Choose F-key set 11–20
shortcut-fkey-prev=Previous F-key set
shortcut-fkey-next=Next F-key set
shortcut-fkey-default=Default F-key set
shortcut-extend-selection=Extend selection
shortcut-toggle-insert=Toggle insert / overwrite
shortcut-hard-blank=Hard blank (0xFF)
shortcut-tab-stop=Next tab stop
shortcut-group-brushes=Brushes & Shapes
shortcut-brush-larger=Larger brush
shortcut-brush-smaller=Smaller brush
shortcut-brush-reset=Reset brush size
shortcut-group-paste=Floating Paste
shortcut-paste-move=Move
shortcut-paste-anchor=Anchor
shortcut-paste-stamp=Stamp copy
shortcut-paste-rotate=Rotate
shortcut-paste-flip-x=Flip horizontally
shortcut-paste-flip-y=Flip vertically
shortcut-paste-transparent=Toggle transparency
shortcut-paste-cancel=Cancel
shortcut-toggle-fg=Toggle foreground color 1–7 (dark / bright)
shortcut-toggle-bg=Toggle background color 0–7 (dark / bright)
shortcut-default-colors=Default colors
shortcut-ice-colors=Toggle iCE colors
shortcut-letter-spacing=Toggle 9px font
shortcut-canvas-size=Set canvas size
shortcut-mirror-mode=Toggle mirror mode
shortcut-group-selection=Selection Tool
shortcut-block-move=Move block (float and erase below)
shortcut-block-copy=Copy block (float a copy)
shortcut-block-fill=Fill with foreground color
shortcut-block-erase=Erase
shortcut-crop=Crop to selection
shortcut-brush-char=Brush character from the active F-key set
shortcut-group-tags=Tag Tool
shortcut-tag-nudge=Move the selected tags
shortcut-tag-nudge-far=Move the selected tags by 10 cells
shortcut-tag-next=Select the next / previous tag
shortcut-tag-clipboard=Copy, cut and paste tags at the caret
shortcut-tag-duplicate=Duplicate the selected tags one row down
shortcut-tag-delete=Delete the selected tags
shortcut-group-modes=Tool Keys (outside the text tools)
shortcut-mode-keyboard=Keyboard (text cursor)
shortcut-mode-brush=Brush
shortcut-mode-shifter=Shading brush
shortcut-mode-fill=Paint bucket
shortcut-paste-center=Center horizontally
reference-image-image=Image
reference-image-not-found=File not found
reference-image-display=Display
reference-image-mode=Mode
reference-image-mode-stretch=Stretch to Canvas
reference-image-mode-contain=Fit to Canvas
reference-image-mode-fit_width=Fit Width
reference-image-mode-fit_height=Fit Height
reference-image-mode-original=Original Size
reference-image-mode-tile=Tile
reference-image-scale=Scale
reference-image-offset=Offset
outline-style-label=Style { $style }
outline-style-choose=Choose an outline style
outline-style-tooltip=Style { $style } ({ $key })
new-kind-ansi-description=Text mode canvas with layers
new-kind-animation-description=Lua scripted ANSI animation
new-kind-bitfont-description=8 × 16 pixel console font
new-kind-color_font-description=Characters with colors
new-kind-block_font-description=Block characters, one color
new-kind-outline_font-description=Outline placeholders
file-dialog-filter-images=Images
file-dialog-filter-artwork=Supported text artwork
file-dialog-filter-palette=Palette
error-export-overwrites-document=Export must not overwrite the current editable document. Choose a different filename.
tdf-font-number=Font { $index }
tdf-duplicate-font=Duplicate Font
tdf-delete-font=Delete Font
tdf-new-font-name=New Font
tdf-edit-glyph=Edit '{ $ch }'
tdf-create-glyph=Create '{ $ch }'
paste-tool-anchor=Anchor (Enter)
paste-tool-keep=Keep as Layer
paste-tool-cancel=Cancel Paste (Escape)
brush-mode-replace=Replace
brush-mode-char-tooltip=Paint with the selected character and colors
brush-mode-half_block-tooltip=Paint half-block pixels with twice the vertical resolution
brush-mode-shading-tooltip=Shade characters: left click lighter, right click darker
brush-mode-replace-tooltip=Replace existing characters, keep the colors
brush-mode-blink-tooltip=Toggle the blink attribute
brush-mode-colorize-tooltip=Change only the colors of existing characters
brush-char-tooltip=Brush character – click to choose from the character table
brush-size=Brush size
brush-size-tooltip=Brush size in cells (Alt+Plus / Alt+Minus)
brush-apply=Apply
brush-apply-fg-tooltip=Apply the foreground color
brush-apply-bg-tooltip=Apply the background color
brush-exact-tooltip=Only fill cells that match character and colors exactly
fkey-set-of=Set { $set } of { $count }
fkey-set-tooltip=F-key character set (Ctrl+Comma / Ctrl+Period to switch, right click a key to reassign)
select-mode-normal-tooltip=Drag a rectangular selection
select-mode-character-tooltip=Select every cell with the clicked character
select-mode-attribute-tooltip=Select every cell with the clicked colors
select-mode-foreground-tooltip=Select every cell with the clicked foreground color
select-mode-background-tooltip=Select every cell with the clicked background color
select-copy=Copy Selection
select-deselect=Deselect
select-justify=Justify
select-mode-add=Add to selection
select-mode-subtract=Remove from selection
tag-new=New Tag
tag-edit=Edit Tag
tag-delete-selected=Delete Selected Tags
tag-place-hint=Double-click or drag along a row to add a tag · drag a tag to move it · Shift+drag to select
tag-empty=(empty)
tag-info=at { $x }, { $y }  ·  { $length } chars
pipette-modifier-hint=Shift: foreground only  ·  Ctrl/right click: background only
palette-title=Palette
tag-new-menu=New Tag…
tag-properties-menu=Properties…
tag-duplicate=Duplicate
error-export-font=Failed to export font: { $error }
font-preview-text-hint=Preview text
font-favorites=Favorites
font-filter-min=min
font-filter-max=max
font-filter-any=0 = any
monitor-save-defaults=Save Defaults
script-group=Lua Script
script-hint=-- Runs against the current document, e.g.
    -- buf:set_char(0, 0, "A")
script-output=Output
script-run=Run
tag-list-empty-hint=Add one here or place it with the Tag tool.
tag-double-click-edit=Double click to edit
tag-disabled=disabled
tag-edit-menu=Edit Tag…
tag-add-menu=Add Tag…
tag-enabled=Enabled
tag-preview-hint=Shown in the editor
tag-replacement-hint=Inserted by the BBS
tag-replacement-browse=Choose from a replacement list
tag-replacements-title=Replacements
tag-replacements-none=No matching replacements
tag-replacements-example=Example: { $example }
tag-replacements-notes=Notes
tag-replacements-back=Back
tag-replacements-import=Import…
tag-replacements-new=New List
tag-replacements-new-tooltip=Creates a list from a template and opens it in your editor
tag-replacements-open-folder=Open Folder
tag-replacements-custom-hint=Your own lists are TOML files in this folder; they show up the next time the list opens
tag-replacements-filter=Replacement lists
tag-replacements-no-folder=There is no folder for replacement lists.
tag-replacements-import-failed=Could not import { $file }: { $error }
tag-role=Role
tag-layout=Layout
tag-column=Column
tag-row=Row
tag-length=Length
tag-alignment=Alignment
unit-characters=characters
unit-lines=lines
button-apply=Apply
replace-export-file=Replace Export File?
replace-font-file=Replace Font File?
replace-file=Replace File?
sauce-information=SAUCE Information
sauce-limit-hint=Up to { $limit } characters
sauce-comments=Comments
export-file-format=File format
export-ansi-options=ANSI Options
export-compatibility=Compatibility
export-rgb-colors=24-bit RGB colors
export-output=Output
export-screen-preparation=Screen preparation
characters-title=Characters
characters-assign-fkey=Assign F{ $key }
new-file-type=Type
new-file-custom-size=Custom
new-file-preset=Preset
canvas-current-size=Current size
size-preset-standard=Standard
size-preset-vga50=VGA 50 lines
size-preset-wide=Wide
size-preset-wide50=Wide, 50 lines
size-preset-40columns=40 columns
start-ansi-subtitle=80 × 25 canvas
start-custom=Custom…
start-custom-subtitle=Choose type and size
start-tdf-subtitle=TheDraw font
start-tagline=Create ANSI art, TheDraw fonts, bitmap fonts and animations.
start-new=New
start-drop-hint=or drop a file anywhere in this window
start-recent=Recent
color-switcher-tooltip=Foreground / Background – click to pick a palette color
palette-click-hint=Left click: foreground · Right click: background
layer-hide=Hide Layer
layer-show=Show Layer
layer-lock=Lock Layer
layer-unlock=Unlock Layer
layer-properties-menu=Layer Properties…
layer-mode-normal=Normal
layer-mode-chars=Characters
layer-mode-attributes=Attributes
status-unknown-font=Unknown
status-canvas-size-tooltip=Canvas size in characters
status-selection=Selection { $width } × { $height }
status-selection-tooltip=Selection: { $left }, { $top } to { $right }, { $bottom }
status-caret-tooltip=Caret position (column, row)
status-font-tooltip=Font: { $font }
    Click to choose a different font
status-dos-aspect=DOS Aspect
status-square-pixels=Square Pixels
status-dos-aspect-tooltip=Pixels are stretched like on a 4:3 DOS monitor.
    Click to use square pixels.
status-square-pixels-tooltip=Pixels are square.
    Click to stretch them like on a 4:3 DOS monitor.
status-9px-font=9 px Font
status-8px-font=8 px Font
status-9px-font-tooltip=Characters are 9 pixels wide (VGA letter spacing).
    Click to use 8 pixel wide characters.
status-8px-font-tooltip=Characters are 8 pixels wide.
    Click to add the 9th pixel column of VGA text mode.
status-ice-colors=iCE Colors
status-blinking=Blinking
status-ice-colors-tooltip=The blink bit selects 8 additional bright background colors (iCE colors).
    Click to make it blink instead.
status-blinking-tooltip=The blink bit makes characters blink.
    Click to use it for 8 bright background colors (iCE colors).
collab-someone=Someone
collab-connect-title=Connect to Server
collab-connect-subtitle=Join a Moebius-compatible collaboration session.
collab-server=Server
collab-recent-servers=Recent servers
collab-hide-password=Hide password
collab-show-password=Show password
collab-identity=Identity
collab-wrong-password=Wrong password
collab-refused=The collaboration server refused the connection.
collab-changed-sauce={ $nick } changed the SAUCE record
collab-changed-size={ $nick } changed the canvas size to { $columns } × { $rows }
collab-ice-on={ $nick } turned iCE colors on
collab-ice-off={ $nick } turned iCE colors off
collab-spacing-on={ $nick } turned letter spacing on
collab-spacing-off={ $nick } turned letter spacing off
collab-changed-font={ $nick } changed the font to { $font }
collab-changed-background={ $nick } changed the background
collab-jump-to-user=Click to jump to their cursor
collab-user-count={ $count ->
    [one] { $count } user
   *[other] { $count } users
}
collab-status-tooltip=Collaboration session — click to show or hide the chat
collab-connecting=Connecting to { $server }…
error-font-psf-extension=Save bitmap fonts with a .psf extension.
font-editor-pixels=Pixels
font-editor-select-pixels=Select Pixels
font-editor-copy-glyph=Copy Glyph
font-editor-discard-question=Discard unsaved font changes?
font-editor-keep-editing=Keep Editing
font-editor-save-font=Save Font…
font-editor-apply=Apply to Document
animation-format-gif=GIF Animation
animation-format-cast=Asciicast v2
animation-export-extension=Choose a different filename with a .{ $extension } extension.
animation-compile=Compile
animation-previous-frame=Previous Frame
animation-next-frame=Next Frame
animation-play-pause=Play / Pause
animation-loop=Loop
animation-loop-tooltip=Restart at the first frame after the last one
animation-speed=Playback speed
animation-frame=Frame
animation-format-av1=AV1 Video (IVF)
animation-first-frame=First Frame
animation-last-frame=Last Frame
animation-restart=Restart
animation-cursor=Ln { $line }, Col { $column }
animation-log=Log
animation-toggle-log=Show or hide the log
animation-modified=Modified
animation-export-browse=Browse…
animation-export-summary={ $frames } frames · { $width }×{ $height } characters · { $duration }
ice-mode-unlimited=Unlimited

# Bitmap font editor
font-editor-size = Size
font-editor-preview-tooltip = Show a sample text in the edited font
font-editor-apply-tooltip = Use the edited font in the drawing
font-editor-close-tooltip = Return to the drawing without applying the font
font-editor-status-char = Char 0x{ $code } { $char }
font-editor-status-size = Glyph size in pixels
font-editor-hints = Ctrl+Arrows slide  ·  Alt+Arrows insert/delete line or column  ·  +/- next/previous char  ·  Tab switches panel
font-editor-character-set = Character Set
outline-code-fill=Fill marker (@)
outline-code-end=End marker (&)
outline-code-hole=Outline hole (space)
outline-code-placeholder=Outline placeholder { $code }
outline-styles-title=Outline Style
status-font-slot-tooltip=Font slot { $slot }: { $font }
font-slots-title=Document Fonts
font-slots-predefined=Predefined fonts
font-slots-custom=Document fonts
font-slots-active=Active slot: { $slot }
font-slots-add=Add font…
font-slots-replace=Replace active slot…
font-slots-replace-tip=Changes the font in existing artwork that uses this slot
font-slots-full=All 256 font slots are occupied.
    Click to draw with this font, double click to choose a different font
sauce-subtitle=Metadata stored at the end of the file and shown by viewers and BBS software.
sauce-record=Record
sauce-comment-too-long=Line { $line } is longer than { $limit } characters and will be shortened.
sauce-display=Display
shortcuts-dialog-subtitle=Quick reference for Icy Draw
recovery-title=Recover unsaved work
recovery-description=Icy Draw was closed unexpectedly while these documents had unsaved changes.
recovery-restore=Restore
recovery-restore-all=Restore All
recovery-discard=Discard…
recovery-discard-confirm=Discard Permanently
recovery-keep=Keep
recovery-later=Decide Later
recovery-saved-at=Autosaved { $time }
recovery-kind-ansi=Drawing
recovery-kind-charfont=TheDraw font
recovery-kind-bitfont=Bitmap font
recovery-kind-animation=Animation
recovery-disk-changed=The file was changed since. Saving asks before replacing it.
recovery-disk-missing=The file no longer exists.
recovery-damaged=This recovery file cannot be read: { $error }
recovery-new-window=Restores in a new window, since this window has unsaved changes.
recovery-unavailable=Autosave is unavailable, so unsaved changes cannot be recovered after a crash: { $error }
recovery-no-directory=no data directory was found
recovery-failed=Autosave failed. Unsaved changes cannot be recovered after a crash until this is resolved: { $error }
recovery-restore-failed=The document could not be restored. The recovery file was kept: { $error }
shade-characters-tooltip=Characters the shading brush steps through, from light to dark. Right-click lightens.
shade-keep-characters=Keep characters
shade-colors-tooltip=Foreground colors the shading brush steps through, one step per stroke. Only applied while Foreground is on.
shade-brush-color=Brush color
shade-edit-ramps=Edit Ramps…
shade-ramps-title=Shade Ramps
shade-ramps-subtitle=Each stroke of the shading brush moves a cell one step along the character ramp and one step along the color ramp.
shade-character-ramps=Character ramps
shade-character-ramp-hint=Characters from light to dark, e.g. ░▒▓█
shade-color-ramps=Color ramps
shade-add-character-ramp=Add Character Ramp
shade-add-color-ramp=Add Color Ramp
shade-remove-ramp=Remove
shade-add-color=Add a palette color
shade-remove-color=Color { $color } – click to remove
shade-ramp-empty=Empty ramp
shade-invalid-character="{ $character }" is not a usable CP437 character
rip-editor-title = RIP drawing
rip-editor-description = Edit RIPscrip vector graphics
igs-editor-title = IGS drawing
igs-editor-description = Edit Atari ST Instant Graphics and Sound drawings
igs-new-resolution = Resolution
igs-editor-commands = Commands
igs-editor-delete = Delete
igs-editor-up = Move up
igs-editor-down = Move down
igs-editor-preview-through = Preview through the selected command
igs-editor-vertex = Vertex { $index }
igs-editor-vertices = { $count } vertices
igs-editor-add-vertex = Add vertex
igs-editor-remove-vertex = Remove vertex
igs-editor-source = IGS source
igs-editor-apply-source = Apply
igs-editor-revert-source = Revert
igs-editor-source-only = This command is edited as IGS source.
igs-editor-loop-source = Loops are edited as IGS source.
igs-editor-invalid-escape = The source contains an invalid escape; use \xNN, \r, \n, \e or \\.
igs-editor-variable-parameter = A random (r, R) or loop (x, y) value, changed in the IGS source
igs-tool-select = Select
igs-tool-marker = Marker
igs-tool-line = Line
igs-tool-polyline = Polyline
igs-tool-rectangle = Box
igs-tool-rounded-rectangle = Rounded box
igs-tool-filled-rectangle = Filled rectangle
igs-tool-circle = Circle
igs-tool-ellipse = Ellipse
igs-tool-arc = Arc
igs-tool-elliptical-arc = Elliptical arc
igs-tool-pie-slice = Pie slice
igs-tool-elliptical-pie-slice = Elliptical pie slice
igs-tool-polygon = Polygon
igs-tool-flood-fill = Flood fill
igs-tool-text = Text
igs-command-color = Color
igs-command-fill = Fill style
igs-command-line-style = Line style
igs-command-marker-style = Marker style
igs-command-pen-color = Pen color
igs-command-drawing-mode = Drawing mode
igs-command-hollow = Hollow
igs-command-text-effects = Text style
igs-command-resolution = Resolution
igs-command-clear = Clear screen
igs-command-loop = Loop
igs-command-pause = Pause
igs-command-draw-to = Draw to
igs-command-text = VT52 text
igs-command-invalid = Unparsed
igs-pen = Pen { $pen }
igs-pen-line = Line
igs-pen-fill = Fill
igs-pen-text = Text
igs-pen-marker = Marker
igs-pen-kind = Pen
igs-pen-number = Pen
igs-color = Color
igs-fill = Fill
igs-fill-pattern-label = Fill pattern
igs-fill-border = Border
igs-fill-hollow = Hollow
igs-fill-solid = Solid
igs-fill-pattern = Pattern { $index }
igs-fill-hatch = Hatch { $index }
igs-fill-user = User pattern { $index }
igs-fill-random = Random
igs-fill-kind-pattern = Pattern
igs-fill-kind-hatch = Hatch
igs-fill-kind-user = User pattern
igs-filled = Filled
igs-rounded = Rounded corners
igs-line = Line
igs-line-solid = Solid
igs-line-long-dash = Long dash
igs-line-dotted = Dotted
igs-line-dash-dot = Dash dot
igs-line-dashed = Dashed
igs-line-dash-dot-dot = Dash dot dot
igs-line-user = User defined
igs-thickness = Thickness
igs-end-start = Start
igs-end-end = End
igs-end-square = Square
igs-end-arrow = Arrow
igs-end-rounded = Rounded
igs-marker = Marker
igs-marker-point = Point
igs-marker-plus = Plus
igs-marker-star = Star
igs-marker-square = Square
igs-marker-cross = Cross
igs-marker-diamond = Diamond
igs-size = Size
igs-size-prefix = Size{" "}
igs-drawing-mode = Drawing mode
igs-mode-replace = Replace
igs-mode-transparent = Transparent
igs-mode-xor = XOR
igs-mode-reverse-transparent = Reverse transparent
igs-text-bold = Bold
igs-text-light = Light
igs-text-italic = Italic
igs-text-underlined = Underlined
igs-text-outlined = Outlined
igs-text-rotation = Rotation
igs-text-size-summary = size { $size }
igs-text-hint = Click the drawing to type, or click text to edit it
igs-text-typing-hint = Type, then press Enter or click elsewhere
igs-poly-hint = Click each vertex; right-click or Enter finishes
igs-select-hint = Click a shape to select it
igs-start-angle = Start
igs-end-angle = End
igs-on = on
igs-off = off
igs-resolution = Resolution
igs-resolution-low = Low · 320 × 200 · 16 colors
igs-resolution-medium = Medium · 640 × 200 · 4 colors
igs-resolution-high = High · 640 × 400 · 2 colors
igs-palette-mode = Palette
igs-clear-mode = Mode
igs-pause-seconds = Seconds
igs-pause-vsyncs = Vertical syncs
igs-palette-edit = Edit IGS palette…
igs-palette-edit-tooltip = Set the colors of the pens with S commands
igs-palette-title = IGS palette
igs-palette-subtitle = Colors of the pens, in the eight Atari ST levels per channel
igs-palette-pens = Pens
igs-palette-red = Red
igs-palette-green = Green
igs-palette-blue = Blue
igs-palette-revert = Revert
igs-tool-copy-area = Copy area
igs-tool-zone = Mouse zone
igs-blit-and = And
igs-blit-invert = Invert
igs-blit-mode = Copy mode
igs-copy-hint = Drag out the area to copy
igs-copy-place-hint = Click where the copy goes; right-click picks another area
igs-zone-id = Zone
igs-zone-host = Host string
igs-zone-host-tooltip = Sent to the host when the zone is clicked
igs-zone-hint = Drag on free space to add a zone; drag a zone to move it
igs-zone-host-required = Enter the host string of the zone first
igs-command-fill-pattern = Fill pattern
igs-command-sound = Sound
igs-command-chip-music = Chip music
igs-command-stop-sound = Stop sound
igs-command-clear-zones = Clear zones
igs-pattern-slot-summary = slot { $slot }
igs-pattern-edit = Fill patterns…
igs-pattern-draw-user = Draw user pattern…
igs-pattern-edit-tooltip = Draw one of the eight user fill patterns (X 7)
igs-pattern-title = Fill pattern
igs-pattern-subtitle = A 16 × 16 pattern for fills with a user pattern
igs-pattern-slot = Slot
igs-pattern-preview = Tiled
igs-pattern-shift = Move
igs-pattern-shift-left = Move left
igs-pattern-shift-right = Move right
igs-pattern-shift-up = Move up
igs-pattern-shift-down = Move down
igs-pattern-clear = Clear
igs-pattern-invert = Invert
igs-sound-play = Play sound
igs-sound-stop = Stop sound
playback-animation = Animation
playback-play-pause = Play or pause the animation
playback-previous = Previous command
playback-next = Next command
playback-stop = Stop
playback-position = { $current } / { $total }
playback-seek = Drag to jump to a command
playback-speed = Speed
playback-speed-max = Max
igs-group-attributes = Drawing attributes
igs-group-drawing = Drawing
igs-group-screen = Screen
igs-group-colors = Color registers
igs-group-flow = Loops and pauses
igs-group-text = VT52 text and cursor
igs-group-sound = Sound
igs-group-interaction = Input and mouse zones
igs-template-pen-color = Set pen color
igs-template-pen-palette = Set pen palette color
igs-template-drawing-mode = Set drawing mode
igs-template-line-style = Set line type
igs-template-marker-style = Set marker type
igs-template-fill = Set fill pattern
igs-template-hollow = Set hollow
igs-template-text-effects = Set text effects
igs-template-loop = Loop
igs-template-pause-seconds = Pause (seconds)
igs-template-pause-vsync = Pause (vertical syncs)
igs-template-sound = Sound effect
igs-template-chip-music = Chip music
igs-template-effect-loops = Sound effect repeats
igs-template-stop-sound = Stop all sound
igs-template-clear = Clear screen
igs-template-initialize = Initialize
igs-template-resolution = Set resolution
igs-template-random-range = Random range
igs-template-draw-to-start = Draw-to start point
igs-template-draw-to = Draw to
igs-template-clear-zones = Clear mouse zones
igs-template-cursor-off = Cursor off
igs-template-position-cursor = Position cursor
igs-template-text-color = VT52 text color
igs-template-text = VT52 text
igs-tool-spray = Spray paint
igs-spray-density = Markers sprayed into the area
igs-spray-density-prefix = Density{" "}
igs-spray-rotation = Cycle sprayed marker colors
igs-spray-rotation-pen = From pen
igs-spray-rotation-summary = from pen { $pen }
igs-rotate-start = First register
igs-rotate-end = Last register
igs-rotate-count = Shifts
igs-rotate-delay = Delay (1/200 s)
igs-rotate-hint = Registers shift right when the first is lower than the last, otherwise left. 0 shifts restore the colors from before rotating.
igs-rotate-reset = restore
igs-color-register = Register
igs-chip-voice = Voice
igs-chip-volume = Volume
igs-chip-pitch = Pitch
igs-chip-timing = Duration (1/200 s)
igs-chip-stop = Afterwards
igs-chip-stop-none = Keep playing
igs-chip-stop-release = Release the voice
igs-chip-stop-voice = Stop the voice
igs-chip-stop-release-all = Release all voices
igs-chip-stop-all = Stop all voices
igs-effect-loops = Repeats of effects 0–4
igs-cursor = Cursor
igs-cursor-off = Hidden
igs-cursor-on = Visible
igs-cursor-destructive = Destructive backspace
igs-cursor-non-destructive = Non-destructive backspace
igs-inverse-video = VT52 inverse text
igs-text-layer = Layer
igs-text-foreground = Foreground
igs-text-background = Background
igs-input-return = Send a carriage return after the input
igs-input-kind = Waits for
igs-input-output = Typing
igs-input-show = Show and send
igs-input-hide = Hide and send
igs-input-show-discard = Show, don't send
igs-input-hide-discard = Hide, don't send
igs-input-key = One key
igs-input-line = A line of text
igs-input-zones-marker = Mouse zone click, marker pointer
igs-input-zones = Mouse zone click, { $pointer }
igs-pointer-arrow = arrow
igs-pointer-hourglass = hourglass
igs-pointer-bee = busy bee
igs-pointer-finger = pointing finger
igs-pointer-hand = flat hand
igs-pointer-thin-cross = thin cross hair
igs-pointer-thick-cross = thick cross hair
igs-pointer-outlined-cross = outlined cross hair
igs-command-spray-rotation = Spray color rotation
igs-command-color-rotation = Color rotation
igs-command-color-register = Color register
igs-command-color-registers = Color registers
igs-command-input = User input
igs-command-blit-memory = BitBlit memory
igs-template-inverse-text = VT52 inverse text
igs-template-input = Input from user
igs-template-color-register = Set color register
igs-template-color-rotation = Rotate color registers
igs-template-color-rotation-reset = Restore rotated colors
igs-template-spray-rotation = Spray paint color rotation
igs-template-wipe-blit = Wipe BitBlit memory
igs-editor-add = Add command
igs-editor-add-hint = Inserted after the selection while previewing through it, otherwise at the end
igs-editor-duplicate = Duplicate (Ctrl+D)
igs-resolution-tooltip = The resolution the drawing starts in, set by its first R command
igs-resolution-mid-warning = This changes the resolution after drawing commands; later coordinates use the new canvas.
rip-editor-commands = Commands
rip-editor-preview-through = Preview through selected command
rip-editor-properties = Command parameters
rip-editor-unsupported-properties = Parameters for this command are not editable yet.
rip-command-color = Color
rip-command-line-style = Line style
rip-command-fill-style = Fill style
rip-command-font-style = Font
rip-command-palette = Palette
rip-command-palette-slot = Palette color
rip-editor-pixel = Pixel
rip-editor-line = Line
rip-editor-rectangle = Rectangle
rip-editor-bar = Filled rectangle
rip-editor-circle = Circle
rip-editor-oval = Filled ellipse
rip-editor-outline-oval = Ellipse outline
rip-editor-text = Text
rip-editor-bezier = Bézier curve
rip-editor-polygon = Polygon
rip-editor-filled-polygon = Filled polygon
rip-editor-polyline = Polyline
rip-editor-arc = Arc
rip-editor-oval-arc = Oval arc
rip-editor-pie-slice = Pie slice
rip-editor-oval-pie-slice = Oval pie slice
rip-editor-vertex = Vertex
rip-editor-vertices = { $count } vertices
rip-editor-add-vertex = Add vertex
rip-editor-remove-vertex = Remove last vertex
rip-editor-start-angle = Start
rip-editor-end-angle = End
rip-poly-hint = Click vertices; right-click or Enter finishes, Esc cancels.
rip-palette-title = RIP palette
rip-palette-subtitle = Assign one of the 64 EGA colors to each of the 16 drawing color slots.
rip-palette-slots = Color slots
rip-palette-slot-tooltip = Slot { $slot } · EGA { $color }
rip-palette-ega = EGA color for slot { $slot }
rip-palette-default = Restore defaults
rip-palette-edit = Edit RIP palette…
rip-palette-edit-tooltip = Assign EGA colors to the 16 RIP color slots.
rip-editor-button = Button
rip-editor-mouse = Mouse region
rip-command-mouse-fields = Clear mouse regions
rip-mouse-invert = Invert when clicked
rip-mouse-clear = Clear text window
rip-mouse-host-tooltip = Sent to the host when the region is clicked, e.g. M^m for the key M and Enter
rip-mouse-hint = Drag to add a region; click one to select, move or resize it.
rip-mouse-no-command = (no host command)
rip-editor-label = Label
rip-editor-label-required = Enter a label before placing text or a button.
rip-editor-button-size-required = Drag to give the button a size.
rip-editor-button-label-delimiter = Button labels cannot contain <>.
rip-editor-color = Draw
rip-editor-border-color = Border
rip-editor-fill-color = Fill
rip-editor-delete = Delete command
rip-editor-up = Move command up
rip-editor-down = Move command down
rip-editor-preserved = Original mixed RIP/ANSI commands are preserved; add new commands below.
start-connect-tooltip=Join a Moebius-compatible collaboration server and draw together
rip-bezier-segments = Segments
rip-bezier-adjust-hint = Drag the handles; right-click or Enter finishes, Esc cancels.
rip-line-solid = Solid
rip-line-dotted = Dotted
rip-line-center = Center
rip-line-dashed = Dashed
rip-line-user = Custom
rip-line-thin = 1 px
rip-line-thin-tooltip = Thin lines
rip-line-thick = 3 px
rip-line-thick-tooltip = Thick lines
rip-fill-pattern = Fill
rip-fill-empty = Empty
rip-fill-solid = Solid
rip-fill-line = Lines
rip-fill-light-slash = Light slashes
rip-fill-slash = Slashes
rip-fill-backslash = Backslashes
rip-fill-light-backslash = Light backslashes
rip-fill-hatch = Hatch
rip-fill-cross-hatch = Cross hatch
rip-fill-interleave = Interleave
rip-fill-wide-dots = Wide dots
rip-fill-close-dots = Close dots
rip-fill-user = Custom
rip-font = Font
rip-font-default = Default (8 × 8)
rip-font-size = Font size
rip-font-size-prefix = Size{" "}
rip-text-horizontal = Horizontal
rip-text-direction = Direction
rip-text-hint = Click the drawing to type text, or click existing text to change it.
rip-text-typing-hint = Type; Enter or a click elsewhere finishes, Esc cancels.
rip-text-vertical = Vertical
rip-button-plain = Plain
rip-button-plain-tooltip = A beveled button with a text label
rip-button-icon = Icon
rip-button-icon-tooltip = A button showing an icon file (.ICN) from the host
rip-button-clipboard = Clipboard
rip-button-clipboard-tooltip = A button showing the image on the RIP clipboard
rip-button-icon-required = Enter the icon file of the icon button.
rip-button-host-command = Host command
rip-button-host-command-hint = sent when clicked, e.g. ^mMAIN^m
rip-button-icon-file = Icon file
rip-button-hotkey = Hot key
rip-button-group-number = Group
rip-button-style-open = Button Style…
rip-button-style-tooltip = All options of new buttons: colors, font, effects and behavior
rip-button-style-title = Button Style
rip-button-style-entry = Button style
rip-button-edit = Edit Button…
rip-button-edit-hint = The button and its style are edited together.
rip-button-sample = Sample
rip-button-type = Type
rip-button-group-button = Button
rip-button-group-colors = Colors
rip-button-group-layout = Label and size
rip-button-group-effects = Effects
rip-button-group-behavior = Behavior
rip-button-label-color = Label
rip-button-shadow-color = Shadow
rip-button-bright-color = Bright
rip-button-dark-color = Dark
rip-button-surface-color = Surface
rip-button-underline-color = Underline
rip-button-corner-color = Corner
rip-button-orientation = Label position
rip-button-above = Above
rip-button-left = Left
rip-button-center = Center
rip-button-right = Right
rip-button-below = Below
rip-button-justify = Justify
rip-button-bevel-size = Bevel size
rip-button-size = Size for clicks
rip-button-size-tooltip = Used when the button is placed with a click instead of dragged; 0 uses the icon or clipboard size.
rip-button-bevel = Bevel
rip-button-chisel = Chisel
rip-button-sunken = Sunken
rip-button-recessed = Recessed
rip-button-shadow = Shadowed label
rip-button-underline-hotkey = Underline hot key
rip-button-highlight-hotkey = Highlight hot key
rip-button-center-vertically = Center label vertically
rip-button-hot-icons = Hot icons
rip-button-explode = Explode when clicked
rip-button-mouse = Mouse button
rip-button-invert = Invert when clicked
rip-button-reset = Reset screen after click
rip-button-radio = Radio group
rip-button-checkbox = Check box
rip-button-stamp = Stamp image on clipboard
rip-button-selected = Selected
rip-button-group-appearance = Appearance
rip-editor-select = Select
rip-select-hint = Click a shape to select it, drag it to move it and its handles to resize it. Arrow keys nudge, Delete removes.
rip-button-create = Button…
rip-button-create-tooltip = Choose the type, label and style of a new button, then place it in the drawing
rip-button-create-title = Create Button
rip-button-place = Place
rip-button-place-hint = Click to place the button at { $width } × { $height } or drag its size; Esc cancels.

# Attribute picker (Escape without a selection)
attribute-picker-pair = Foreground { $foreground } · background { $background }
attribute-picker-foreground = Foreground { $color }
attribute-picker-background = Background { $color }
attribute-picker-keys = ↑↓ foreground · ←→ background · Enter or Esc closes

# Line tool: outline mode (box-drawing lines)
line-style-outline = Outline
line-style-outline-tooltip = Draw box-drawing lines that join the lines they meet
line-style-single-tooltip = Single box-drawing lines, joined with the lines they meet
line-style-double-tooltip = Double box-drawing lines, joined with the lines they meet
line-style-double-horizontal-tooltip = Double across, single down, joined with the lines they meet
line-style-double-vertical-tooltip = Single across, double down, joined with the lines they meet
