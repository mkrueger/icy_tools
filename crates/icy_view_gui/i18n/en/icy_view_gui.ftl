heading-title=Title
heading-author=Author
heading-group=Group
heading-screen-mode=Flags

menu-item-discuss=Discuss
menu-item-report-bug=Report a bug
menu-item-check-releases=Latest release
menu-item-auto-scroll=Auto scroll
menu-item-scroll-speed-slow=Slow speed
menu-item-scroll-speed-medium=Medium speed
menu-item-scroll-speed-fast=Fast speed
menu-item-set_terminal_width=Set terminal width
menu-item-settings=Settings

menu-upgrade_version=Upgrade to { $version }

tooltip-refresh=Refresh
tooltip-reset-filter-button=Reset filter
tooltip-back=Go back
tooltip-forward=Go forward
tooltip-up=Go to parent directory
tooltip-filter=Filter (Ctrl+F)
tooltip-view-mode-list=Switch to list view
tooltip-view-mode-tiles=Switch to tiles view
tooltip-browse-16colors=Browse 16colors.rs
tooltip-settings=Settings
tooltip-sort-name-asc=Sort by name (A-Z)
tooltip-sort-name-desc=Sort by name (Z-A)
tooltip-sort-size-asc=Sort by size (smallest first)
tooltip-sort-size-desc=Sort by size (largest first)
tooltip-sort-date-asc=Sort by date (oldest first)
tooltip-sort-date-desc=Sort by date (newest first)
tooltip-sauce-mode-on=Show SAUCE information
tooltip-sauce-mode-off=Hide SAUCE information
tooltip-shuffle-mode=Shuffle mode (slideshow)

header-name=Name
header-title=Title
header-author=Author
header-group=Group

statusbar-items={ $count } items
statusbar-ready=Ready

filter-entries-hint-text=Filter entries
label-terminal_width=Terminal width:

message-loading-image=Loading image…
message-file-not-supported=File { $name } may not be supported.
button-load-anyways=Load anyways
button-ok=OK
button-cancel=Cancel
button-open=Open
message-empty=Here you see nothing until you select a supported file.

error-invalid-path=Invalid path
error-never-happens=Should never happen :) - open a bug report!

sauce-dialog-title=Sauce info
sauce-dialog-title-label=Title:
sauce-dialog-author-label=Author:
sauce-dialog-group-label=Group:
sauce-dialog-comments-label=Comments:
sauce-dialog-date-label=Date:
sauce-dialog-flags-label=Flags:
sauce-dialog-font-name=Font:

sauce-unknown=Unknown
sauce-btn-formatted=Formatted
sauce-btn-raw=Raw

sauce-section-info=SAUCE Information
sauce-section-capabilities=Capabilities
sauce-section-comments=Comments
sauce-section-raw-header=Raw SAUCE Header
sauce-section-technical=Technical Info
sauce-section-comment-lines=Comment Lines

sauce-field-title=Title
sauce-field-author=Author
sauce-field-group=Group
sauce-field-date=Date
sauce-field-type=Type
sauce-field-file-size=File Size
sauce-field-format=Format
sauce-field-columns=Columns
sauce-field-lines=Lines
sauce-field-ice-colors=iCE Colors
sauce-field-letter-spacing=Letter Spacing
sauce-field-aspect-ratio=Aspect Ratio
sauce-field-font=Font
sauce-field-width=Width
sauce-field-height=Height
sauce-field-pixel-depth=Pixel Depth
sauce-field-sample-rate=Sample Rate
sauce-field-data-type=DataType
sauce-field-file-type=FileType
sauce-field-tinfo1=TInfo1
sauce-field-tinfo2=TInfo2
sauce-field-tinfo3=TInfo3
sauce-field-tinfo4=TInfo4
sauce-field-tflags=TFlags
sauce-field-tinfos=TInfoS

sauce-value-yes=Yes
sauce-value-9px=9px
sauce-value-legacy=Legacy
sauce-value-none=None
sauce-value-executable=Executable
sauce-value-bytes={ $count } bytes
sauce-value-lines={ $count } lines
sauce-value-pixels={ $count }px
sauce-value-bpp={ $count }bpp
sauce-value-hz={ $count } Hz

# Help dialog
help-title = Keyboard Shortcuts
help-subtitle = Quick reference for iCY VIEW

toast-auto-scroll-on=Auto scroll on
toast-auto-scroll-off=Auto scroll off
toast-scroll-slow=Scroll speed: slow
toast-scroll-medium=Scroll speed: medium
toast-scroll-fast=Scroll speed: fast
toast-baud-rate-off=Baud emulation: off
toast-baud-rate=Baud rate: { $rate }
toast-command-not-configured={ $key } command not configured

button-play_music=Play Music
button-stop_music=Stop Music

label-music_pause=Pause { $duration }ms
label-music_note=Play { $note }({ $octave }) for { $duration }ms

label-sixteencolors_year= { $year } ({ $packs } packs)

settings-heading=Settings
settings-reset_button=Reset
settings-monitor-category=Monitor
settings-paths-category=Paths

settings-paths-header=Application Paths
settings-paths-user-header=User Paths
settings-paths-export-path=Export path:
settings-paths-config-dir=Config directory:
settings-paths-config-file=Config file:
settings-paths-log-file=Log file:
settings-paths-open=Open

settings-commands-category = Commands
settings-commands-section = External Programs
settings-commands-placeholder = Command (e.g. icy_draw %F)
settings-commands-description = Use %F for file name

export-no-file-selected = No file selected to export
export-success = Exported to { $path }

preview-no-file-selected = No file selected
preview-loading = Loading...
preview-error = Error: { $message }
preview-error-title = Failed to load file

error-read-file-data = Unable to read file content
error-read-file = { $error }

error-external-command-title = Failed to execute external command
error-external-command-message = Command: { $command }

    Error: { $error }

welcome-select-file = 📂 Select a file to preview
welcome-tip = Tip: Press Ctrl+F to filter, or click 🌐 to browse 16colors.rs

thumbnail-loading = Loading...
thumbnail-no-diz = no file_id.diz
thumbnail-unsupported = Unsupported

filter-no-items-found = No items found.

# ═══════════════════════════════════════════════════════════════════════════════
# Command System (generated from command IDs)
# ═══════════════════════════════════════════════════════════════════════════════

# Categories
cmd-category-file = File
cmd-category-edit = Edit
cmd-category-view = View
cmd-category-navigation = Navigation
cmd-category-window = Window
cmd-category-help = Help
cmd-category-settings = Settings
cmd-category-dialog = Dialogs
cmd-category-playback = Playback
cmd-category-external = External

# File commands
cmd-file-open-action = Open
cmd-file-open-desc = Open a file
cmd-file-export-action = Export
cmd-file-export-desc = Export file to image
cmd-file-close-action = Close
cmd-file-close-desc = Close current file

# Edit commands
cmd-edit-copy-action = Copy
cmd-edit-copy-desc = Copy selection to clipboard
cmd-edit-paste-action = Paste
cmd-edit-paste-desc = Paste from clipboard
cmd-edit-select_all-action = Select All
cmd-edit-select_all-desc = Select all content

# View commands
cmd-view-zoom_in-action = Zoom In
cmd-view-zoom_in-desc = Increase zoom level
cmd-view-zoom_out-action = Zoom Out
cmd-view-zoom_out-desc = Decrease zoom level
cmd-view-zoom_reset-action = Reset Zoom
cmd-view-zoom_reset-desc = Reset to 100% zoom
cmd-view-zoom_fit-action = Fit to Window
cmd-view-zoom_fit-desc = Auto-fit content to window
cmd-view-fullscreen-action = Fullscreen
cmd-view-fullscreen-desc = Toggle fullscreen mode

# Navigation commands
cmd-nav-back-action = Go Back
cmd-nav-back-desc = Navigate to previous location
cmd-nav-forward-action = Go Forward
cmd-nav-forward-desc = Navigate to next location
cmd-nav-up-action = Parent Directory
cmd-nav-up-desc = Go up one directory level

# Window commands
cmd-window-new-action = New Window
cmd-window-new-desc = Open a new window
cmd-window-close-action = Close Window
cmd-window-close-desc = Close current window

# Help commands
cmd-help-show-action = Help
cmd-help-show-desc = Show keyboard shortcuts
cmd-help-about-action = About
cmd-help-about-desc = Show about dialog

# Settings commands
cmd-settings-open-action = Settings
cmd-settings-open-desc = Open settings dialog

# Dialog commands (icy_view specific)
cmd-dialog-sauce-action = SAUCE Info
cmd-dialog-sauce-desc = Show SAUCE metadata dialog
cmd-dialog-export-action = Export
cmd-dialog-export-desc = Export file to image
cmd-dialog-filter-action = Filter
cmd-dialog-filter-desc = Toggle filter input

# Playback commands (icy_view specific)
cmd-playback-toggle_scroll-action = Auto Scroll
cmd-playback-toggle_scroll-desc = Toggle automatic scrolling
cmd-playback-scroll_speed-action = Scroll Speed +
cmd-playback-scroll_speed-desc = Increase scroll speed
cmd-playback-scroll_speed_back-action = Scroll Speed -
cmd-playback-scroll_speed_back-desc = Decrease scroll speed
cmd-playback-baud_rate-action = Baud Rate +
cmd-playback-baud_rate-desc = Increase baud rate
cmd-playback-baud_rate_back-action = Baud Rate -
cmd-playback-baud_rate_back-desc = Decrease baud rate
cmd-playback-baud_rate_off-action = Baud Off
cmd-playback-baud_rate_off-desc = Disable baud emulation

# External commands (icy_view specific)
cmd-external-command_0-action = External 1
cmd-external-command_0-desc = Run external command 1 (F5)
cmd-external-command_1-action = External 2
cmd-external-command_1-desc = Run external command 2 (F6)
cmd-external-command_2-action = External 3
cmd-external-command_2-desc = Run external command 3 (F7)
cmd-external-command_3-action = External 4
cmd-external-command_3-desc = Run external command 4 (F8)