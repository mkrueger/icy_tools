# Icy Mail user interface (English, the fallback language)

dialog-close_button = Close

# Toolbar
toolbar-open = Open
toolbar-open-tooltip = Open a mail packet (Ctrl+O)
toolbar-new = New
toolbar-new-tooltip = Write a new message (Ctrl+N)
toolbar-reply = Reply
toolbar-reply-tooltip = Reply to the message (Ctrl+R)
toolbar-forward = Forward
toolbar-forward-tooltip = Forward the message (Ctrl+L)
toolbar-export = Export Replies
toolbar-export-count = Export Replies ({ $count })
toolbar-export-tooltip = Save the reply packet to upload to the BBS (Ctrl+Shift+E)
toolbar-menu = Menu
toolbar-threads-tooltip = Group messages into threads (Ctrl+T)
toolbar-list-tooltip = Show messages as a list (Ctrl+T)
toolbar-search-hint = Search messages
toolbar-search-clear = Clear Search (Esc)

# Main menu
menu-open-packet = Open Packet…
menu-open-recent = Open Recent
menu-reload-packet = Reload Packet
menu-packet-information = Packet Information
menu-new-message = New Message
menu-reply = Reply
menu-forward = Forward
menu-export-replies = Export Replies…
menu-next-unread = Next Unread
menu-mark-read = Mark as Read
menu-mark-unread = Mark as Unread
menu-mark-folder-read = Mark Folder as Read
menu-address-book = Address Book…
menu-taglines = Taglines…
menu-add-author = Add Author to Address Book
menu-save-tagline = Save Message Tagline
menu-view = View
menu-view-list = List
menu-view-threads = Threads
menu-unread-only = Unread Messages Only
menu-message-zoom = Message Zoom
menu-appearance = Appearance
menu-settings = Settings…
menu-keyboard-shortcuts = Keyboard Shortcuts
menu-new-window = New Window
menu-close-window = Close Window

# Status bar
status-drafts = { $count ->
    [one] 1 draft
   *[other] { $count } drafts
}
status-messages = { $count } messages · { $unread } unread
status-no-packet = No packet open
status-message-zoom = Message zoom
status-replies-to-send = { $count ->
    [one] 1 reply to send
   *[other] { $count } replies to send
}
status-show-outbox = Show the outbox

# Sidebar
sidebar-offline-mail = Offline mail
sidebar-packet-information = Packet information
sidebar-mailboxes = Mailboxes
sidebar-conferences = Conferences
sidebar-sort-conferences = Sort conferences
sidebar-sort-number = By Number
sidebar-sort-name = By Name
sidebar-sort-count = By Message Count
sidebar-conference-tooltip = Conference { $number }
    { $count } messages, { $unread } unread
folder-all = All Messages
folder-all-tooltip = { $count } messages, { $unread } unread
folder-personal = Personal
folder-personal-tooltip = Messages addressed to { $user }
folder-outbox = Outbox
folder-outbox-tooltip = Replies and new messages waiting to be exported

# File dialogs
loading-open-title = Open Mail Package
loading-filter-packages = Mail Packages
loading-filter-all = All Files
loading-export-title = Export Reply Packet
loading-filter-reply = QWK Reply Packet

# Start page
welcome-opening = Opening { $name }…
welcome-tagline = Read and answer your BBS mail offline.
welcome-open-packet = Open Packet…
welcome-open-packet-tooltip = Open a QWK packet (Ctrl+O)
welcome-drop-hint = or drop a .QWK packet onto this window
welcome-recent-packets = Recent packets
welcome-forget-recent = Remove from the list

# Command line
cli-about = An offline QWK mail reader and reply-packet composer
cli-debug-help = Enable debug logging, including raw ANSI QWK headers
cli-file-help = Mail package to open (QWK in any common archive format)

# Application: notices, errors and generated text
folder-conference = Conference { $number }
app-tab-folders = Folders
app-tab-messages = Messages
app-tab-message = Message
app-no-subject = (no subject)
app-export-failed = Unable to export { $path }:
    { $error }
app-drafts-failed = Unable to load the drafts for { $path }:
    { $error }
app-no-conference = This packet has no conference to post a message in.
app-new-message-failed = Unable to start a new message: { $error }
app-message-unavailable = The selected message is no longer available.
app-original-failed = Unable to read the original message: { $error }
app-reply-failed = Unable to start the reply: { $error }
app-draft-unavailable = The draft is no longer available.
app-delete-draft-failed = Unable to delete the draft: { $error }
app-taglines-read-failed = Unable to read the taglines:
    { $error }
app-taglines-save-failed = Unable to save the taglines:
    { $error }
# Inserted into the message text; QWK can only transport CP437 characters.
app-quote-attribution = On { $date } { $name } wrote:
app-forward-header = --- Forwarded message ---
    From: { $from }
    To: { $to }
    Date: { $date }
    Subject: { $subject }
notice-finish-writing = Save or discard the message you are writing first
notice-exported = Reply packet saved as { $file } - upload it to the BBS
notice-read-marks-unavailable = Read marks are unavailable: { $error }
notice-read-marks-failed = Unable to save read marks: { $error }
notice-marked-read = { $count ->
    [one] Marked 1 message as read
   *[other] Marked { $count } messages as read
}
notice-continuing-in = Continuing in { $name }
notice-no-more-unread = No more unread messages
notice-draft-deleted = Draft deleted
notice-nothing-to-export = There are no replies to export
notice-no-tagline = This message has no tagline
notice-tagline-known = This tagline is already in your list
notice-tagline-saved = Tagline saved: ... { $tagline }
notice-message-copied = Message copied to the clipboard

# Settings
settings-title = Settings
settings-page-general = General
settings-page-monitor = Monitor
settings-section-appearance = Appearance
settings-theme-label = Theme
settings-theme-follow-system = Follow System
settings-theme-light = Light
settings-theme-dark = Dark
settings-section-messages = Messages
settings-zoom-label = Zoom
settings-zoom-fit-width = Fit Width
settings-zoom-fit = Fit
settings-section-writing = Writing
settings-add-random-tagline = Add a random tagline to new messages

# Mail dialogs
dialog-mail-app-title = Icy Mail
dialog-mail-delete-draft-title = Delete this draft?
dialog-mail-delete-draft-message = “{ $title }” will be removed from the outbox. This cannot be undone.
dialog-mail-delete-draft-keep = Keep
dialog-mail-delete-draft-delete = Delete
dialog-mail-discard-title = Discard this message?
dialog-mail-discard-quit-message = The message you are writing has not been saved. Close the window anyway?
dialog-mail-discard-message = Your changes to this message will be lost.
dialog-mail-discard-keep-editing = Keep Editing
dialog-mail-discard-discard = Discard
dialog-mail-export-problems-title = Some messages need attention
dialog-mail-export-problems-message =
    Fix these messages in the outbox before exporting the reply packet:

    { $problems }
dialog-mail-show-outbox = Show Outbox
dialog-mail-packet-info-subtitle = Packet information
dialog-mail-packet-board = Board
dialog-mail-packet-location = Location
dialog-mail-packet-phone = Phone
dialog-mail-packet-sysop = Sysop
dialog-mail-packet-bbs-id = BBS ID
dialog-mail-packet-packet = Packet
dialog-mail-packet-user = User
dialog-mail-packet-created = Created
dialog-mail-packet-messages = Messages
dialog-mail-packet-unread = Unread
dialog-mail-packet-conferences = Conferences
dialog-mail-packet-outbox = Outbox
dialog-mail-packet-file = File

# Keyboard shortcuts
shortcuts-title = Keyboard Shortcuts
shortcuts-section-reading = Reading
shortcuts-reading-previous-next-entry = Previous or next entry
shortcuts-reading-next-pane = Next pane
shortcuts-reading-open-selected-folder-message = Open the selected folder or message
shortcuts-reading-page-down-next-unread = Page down, then next unread message
shortcuts-reading-next-unread = Next unread message
shortcuts-reading-mark-read-unread = Mark as read or unread
shortcuts-reading-mark-folder-read = Mark the folder as read
shortcuts-reading-search-messages = Search messages
shortcuts-reading-switch-list-threads = Switch between list and threads
shortcuts-reading-save-message-tagline = Save the message's tagline
shortcuts-reading-address-book = Address book
shortcuts-reading-add-author-address-book = Add the author to the address book
shortcuts-section-writing = Writing
shortcuts-writing-new-message = New message
shortcuts-writing-reply = Reply
shortcuts-writing-forward = Forward
shortcuts-writing-save-draft = Save the draft
shortcuts-writing-pick-recipient-address-book = Pick the recipient from the address book
shortcuts-writing-choose-tagline = Choose the tagline while writing
shortcuts-writing-edit-tagline-list = Edit the tagline list
shortcuts-writing-cancel-writing = Cancel writing
shortcuts-writing-delete-selected-draft = Delete the selected draft
shortcuts-writing-export-reply-packet = Export the reply packet
shortcuts-section-packets = Packets
shortcuts-packets-open-packet = Open a packet
shortcuts-packets-reload-packet = Reload the packet
shortcuts-section-windows = Windows
shortcuts-windows-new-window = New window
shortcuts-windows-close-window = Close the window
shortcuts-windows-settings = Settings
shortcuts-windows-keyboard-shortcuts = Keyboard shortcuts
shortcuts-section-message-editor = Message editor
shortcuts-editor-text-color = Text color, recolors a selection
shortcuts-editor-character-table = Character table, pick with the keyboard
shortcuts-editor-quote-panel = Quote panel
shortcuts-editor-find-text = Find text
shortcuts-editor-find-next = Find next
shortcuts-editor-delete-line = Delete the line
shortcuts-editor-undo = Undo
shortcuts-editor-redo = Redo
shortcuts-editor-select-all-quote-rest = Select all, or quote the rest
shortcuts-editor-insert-overwrite = Insert or overwrite
shortcuts-editor-close-panel-selection = Close a panel or the selection

# Composer
composer-title-edit-draft = Edit Draft
composer-title-new-message = New Message
composer-title-reply = Reply
composer-title-forward-message = Forward Message
composer-draft-saved-needs-changes = Draft saved - it needs changes before it can be exported
composer-draft-saved-outbox = { $count ->
    [one] Draft saved - 1 message in the outbox
   *[other] Draft saved - { $count } messages in the outbox
}
composer-save-error = Unable to save the draft: { $error }
composer-forwarding-origin = Forwarding { $origin }
composer-replying-origin = Replying to { $origin }
composer-save-draft = Save Draft
composer-save-draft-tooltip = Keep the message in the outbox (Ctrl+S)
composer-cancel = Cancel
composer-cancel-tooltip = Close without saving (Esc)
composer-delete-draft = Delete Draft
composer-conference = Conference
composer-conference-number = Conference { $number }
composer-private = Private
composer-private-tooltip = Only the recipient and the sysop can read private messages
composer-field-to = To
composer-field-subject = Subject
composer-field-from = From
composer-qwk-header-limit-tooltip = QWK headers hold up to 25 characters
composer-address-book-tooltip = Choose from the address book (Ctrl+B)
composer-tagline = Tagline
composer-no-tagline = No tagline
composer-choose-tagline-tooltip = Choose a tagline (Ctrl+T)
composer-random-tagline = Random tagline

# Terminal editor
editor-paste-replaced = { $count } pasted characters are not in CP437 and became “?”
editor-character-not-cp437 = “{ $character }” is not a CP437 character - Ctrl+G shows the character table
editor-no-original-to-quote = There is no original message to quote
editor-find-not-found = “{ $query }” was not found
editor-qwk-separator-unusable = Character 227 (E3h) is the QWK line separator and cannot be used
editor-control-code-unusable = Character { $code } ({ $hex }h) is a terminal control code and cannot be used
editor-text-color-tooltip = Text color (Ctrl+K)
editor-characters = Characters
editor-characters-tooltip = CP437 character table (Ctrl+G)
editor-quote = Quote
editor-quote-tooltip = Quote the original message (Ctrl+Q)
editor-quote-disabled-tooltip = Only replies have a message to quote
editor-find = Find
editor-find-tooltip = Find text (Ctrl+F, F3 next)
editor-keys-tooltip = Editor keys (F1)
editor-find-in-message = Find in message
editor-find-next-tooltip = Find next (F3)
editor-close-tooltip = Close (Esc)
editor-quote-from-original = Quote from the original
editor-quote-rest = Quote Rest
editor-quote-rest-tooltip = Quote the selected line and all after it (Ctrl+A)
editor-quote-line = Quote Line
editor-quote-line-tooltip = Quote the selected line (Enter)
editor-quote-help = ↑↓ select · Enter quote line · Ctrl+A quote rest · Esc close
editor-character-details = Character { $code } · { $hex }h
editor-character-details-reserved = Character { $code } · { $hex }h · reserved
editor-character-picking-hint = Arrows choose · Enter insert · Esc back to the text
editor-character-click-hint = Click to insert · Ctrl+G picks with the keyboard
editor-selection-color = Color of the selection
editor-text-color = Text color
editor-foreground = Foreground
editor-background = Background
editor-blink = Blink
editor-color-preview-text = The quick brown fox
editor-color-help = ←→ ↑↓ 0-F choose · Space blink
editor-default = Default
editor-default-color-tooltip = Light gray on black (Del)
editor-apply = Apply
editor-cancel = Cancel
editor-status-line = Ln
editor-status-column = Col
editor-status-sample = Aa
editor-status-insert = INS
editor-status-overwrite = OVR
editor-status-color = Color
editor-status-chars = Chars
editor-status-quote = Quote
editor-status-find = Find
editor-status-help = Help
editor-color-black = Black
editor-color-blue = Blue
editor-color-green = Green
editor-color-cyan = Cyan
editor-color-red = Red
editor-color-magenta = Magenta
editor-color-brown = Brown
editor-color-light-gray = Light gray
editor-color-dark-gray = Dark gray
editor-color-light-blue = Light blue
editor-color-light-green = Light green
editor-color-light-cyan = Light cyan
editor-color-light-red = Light red
editor-color-light-magenta = Light magenta
editor-color-yellow = Yellow
editor-color-white = White

# Reader view
reader-no-message-selected = No message selected
reader-pick-message-detail = Pick a message from the list to read it here.
reader-next-message = Next message
reader-previous-message = Previous message
reader-copy-message-text = Copy message text
reader-no-tagline = This message has no tagline
reader-save-tagline = Save tagline “{ $tagline }” (T)
reader-add-author-address-book = Add the author to the address book (Shift+A)
reader-mark-as-read-short = Mark as read (M)
reader-mark-as-unread-short = Mark as unread (M)
reader-forward-short = Forward (Ctrl+L)
reader-reply-short = Reply (Ctrl+R)
reader-to = to
reader-private = Private
reader-reply-to = reply to #{ $number }
reader-show-original-message = Show the original message
reader-copy = Copy
reader-copy-message = Copy Message
reader-reply = Reply
reader-forward = Forward

# Draft preview
reader-no-draft-selected = No draft selected
reader-no-draft-detail = Write a reply or a new message to fill the outbox.
reader-conference-number = Conference { $number }
reader-kind-new-message = New message
reader-kind-reply = Reply
reader-kind-forward = Forward
reader-delete-draft-short = Delete draft (Del)
reader-edit = Edit
reader-edit-draft-short = Edit the draft (Enter)
reader-to-capital = To
reader-no-recipient = (no recipient)
reader-from = from
reader-delete = Delete
reader-fix-before-export = Fix before exporting

# Message list
list-draft-count = { $count ->
    [one] 1 message
   *[other] { $count } messages
}
list-message-count-unread = { $count } · { $unread } unread
list-export-replies = Export Replies…
list-unread = Unread
list-show-only-unread-messages = Show only unread messages
list-column-from = From
list-column-subject-threads = Subject (threads)
list-column-subject = Subject
list-column-date = Date
list-column-lines = Lines
list-reply = Reply
list-forward = Forward
list-mark-as-read = Mark as Read
list-mark-as-unread = Mark as Unread
list-nothing-matches = Nothing matches “{ $query }”
list-all-read = Everything in this folder has been read
list-no-personal = No messages are addressed to { $name }
list-folder-empty = This folder is empty
list-no-messages = No messages

# Draft list
list-column-to = To
list-column-conference = Conference
list-column-written = Written
list-no-recipient = (no recipient)
list-needs-attention = Needs attention before export: { $message }
list-edit = Edit
list-delete-ellipsis = Delete…
list-outbox-empty-title = The outbox is empty
list-outbox-empty-detail = Replies and new messages stay here until you export them as a reply packet.

# Taglines

taglines-read-error =
    Unable to read the taglines:
    { $error }
taglines-save-error = Unable to save the taglines: { $error }
taglines-count-none = No taglines yet
taglines-count-one = 1 tagline
taglines-count-many = { $count } taglines
taglines-title-choose = Choose a Tagline
taglines-title-manage = Taglines
taglines-edit-heading = Edit tagline
taglines-new-heading = New tagline
taglines-edit-hint = A witty one-liner
taglines-save = Save
taglines-cancel = Cancel
taglines-shown-as = Shown below the message as “... text” · { $count }/{ $limit }
taglines-filter-hint = Filter taglines
taglines-new-button = New Tagline
taglines-new-tooltip = Add a tagline to the list
taglines-empty-title = No taglines yet
taglines-empty-help = Add your favourite sayings with New Tagline, or press T while reading to keep the tagline of a message.
taglines-no-filter-match = No tagline contains “{ $filter }”
taglines-delete-tooltip = Delete (Del)
taglines-edit-tooltip = Edit
taglines-random = Random
taglines-random-tooltip = Pick any tagline from the list
taglines-no-tagline = No Tagline
taglines-use = Use Tagline
taglines-close = Close

# Address book

address-read-error =
    Unable to read the address book:
    { $error }
address-save-error-multiline =
    Unable to save the address book:
    { $error }
address-save-error = Unable to save the address book: { $error }
address-sender-already-present = { $name } is already in the address book
address-sender-added = { $name } added to the address book
address-count-none = No contacts yet
address-count-one = 1 contact
address-count-many = { $count } contacts
address-title-choose = Choose a Recipient
address-title-manage = Address Book
address-edit-heading = Edit contact
address-new-heading = New contact
address-name-label = Name
address-name-hint = As the BBS knows them
address-address-label = Address
address-address-hint = Optional netmail or Internet address
address-save = Save
address-cancel = Cancel
address-filter-hint = Filter by name or address
address-new-button = New Contact
address-new-tooltip = Add someone to the address book
address-empty-title = The address book is empty
address-empty-help = Add people with New Contact, or press Shift+A while reading to add the author of a message.
address-no-filter-match = Nobody matches “{ $filter }”
address-delete-tooltip = Delete (Del)
address-edit-tooltip = Edit
address-use-recipient = Use as Recipient
address-write-message = Write Message
address-write-message-tooltip = Start a new message to this contact
address-close = Close
address-duplicate-name = Somebody with this name is already in the address book

# Draft validation

draft-field-conference = Conference
draft-field-from = From
draft-field-to = To
draft-field-subject = Subject
draft-field-body = Message text
draft-field-tagline = Tagline
draft-issue-conference-not-in-packet = Conference { $conference } is not part of this packet
draft-issue-field-required = { $field } is required
draft-issue-field-too-long = { $field } has { $length } characters; QWK allows { $limit }
draft-issue-message-text-required = Message text is required
draft-issue-control-character = { $field } contains a control character (U+{ $code })
draft-issue-no-cp437-equivalent = { $field } contains “{ $character }”, which has no CP437 equivalent
draft-issue-qwk-line-break = { $field } contains “{ $character }”, which QWK reserves as its line break

# Packet and reader

packet-error-unknown-archive-format = not a mail packet: unknown archive format
packet-error-control-dat-not-found = CONTROL.DAT not found in archive
packet-error-control-dat-parse-failed = Failed to parse CONTROL.DAT: { $error }
packet-error-messages-dat-not-found = MESSAGES.DAT not found in archive
packet-unreadable-message-subject = <unreadable message #{ $number }>
packet-error-message-index-out-of-range = Message index out of range
packet-conference-fallback-name = Conference { $number }
packet-all-conferences = All Conferences
packet-user-data-directory-unavailable = no user data directory available

# Text and settings

text-configuration-directory-unavailable = no configuration directory available

# Window titles and startup
window-title = Icy Mail { $version }
window-packet-title = { $name } - Icy Mail
app-error-wgpu-unavailable = wgpu renderer unavailable
