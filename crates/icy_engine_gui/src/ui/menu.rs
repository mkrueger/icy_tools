//! Menu system for iced applications
//!
//! Provides macros and structures for declaratively defining menus
//! that integrate with the command system.
//!
//! # Example
//!
//! ```ignore
//! use icy_engine_gui::ui::menu::*;
//! use icy_engine_gui::commands::cmd;
//!
//! // Define menu structure
//! let menu_bar = menu_bar![
//!     menu!("file-menu",
//!         item!(cmd::FILE_NEW, Message::NewFile),
//!         item!(cmd::FILE_OPEN, Message::OpenFile),
//!         separator!(),
//!         item!(cmd::FILE_SAVE, Message::SaveFile),
//!     ),
//!     menu!("edit-menu",
//!         item!(cmd::EDIT_UNDO, Message::Undo),
//!         item!(cmd::EDIT_REDO, Message::Redo),
//!     ),
//! ];
//! ```

use std::borrow::Cow;

use crate::commands::CommandDef;

/// A single menu item
#[derive(Clone)]
pub enum MenuItem<M: Clone> {
    /// A command-based menu item with action
    Command {
        /// The command definition for label and hotkey
        command: CommandDef,
        /// The message to send when clicked
        message: M,
        /// Whether the item is enabled
        enabled: bool,
        /// Whether the item is checked (for toggle items)
        checked: Option<bool>,
    },
    /// A visual separator
    Separator,
    /// A submenu
    SubMenu {
        /// Localized label for the submenu
        label: Cow<'static, str>,
        /// Items in the submenu
        items: Vec<MenuItem<M>>,
    },
}

impl<M: Clone> MenuItem<M> {
    /// Create a new command menu item
    pub fn command(command: CommandDef, message: M) -> Self {
        Self::Command {
            command,
            message,
            enabled: true,
            checked: None,
        }
    }

    /// Create a disabled command menu item
    pub fn command_disabled(command: CommandDef, message: M) -> Self {
        Self::Command {
            command,
            message,
            enabled: false,
            checked: None,
        }
    }

    /// Create a checkable command menu item
    pub fn command_checked(command: CommandDef, message: M, checked: bool) -> Self {
        Self::Command {
            command,
            message,
            enabled: true,
            checked: Some(checked),
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Create a submenu
    pub fn submenu(label: impl Into<Cow<'static, str>>, items: Vec<MenuItem<M>>) -> Self {
        Self::SubMenu { label: label.into(), items }
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        if let Self::Command { enabled: ref mut e, .. } = self {
            *e = enabled;
        }
        self
    }

    /// Set checked state
    pub fn checked(mut self, checked: bool) -> Self {
        if let Self::Command { checked: ref mut c, .. } = self {
            *c = Some(checked);
        }
        self
    }
}

/// A top-level menu (dropdown)
#[derive(Clone)]
pub struct Menu<M: Clone> {
    /// Localized label for the menu (e.g., "File", "Edit")
    pub label: Cow<'static, str>,
    /// Translation key for the menu label
    pub label_key: Cow<'static, str>,
    /// Items in this menu
    pub items: Vec<MenuItem<M>>,
}

impl<M: Clone> Menu<M> {
    /// Create a new menu with a label key
    pub fn new(label_key: impl Into<Cow<'static, str>>) -> Self {
        let key = label_key.into();
        Self {
            label: key.clone(), // Will be replaced by translation
            label_key: key,
            items: Vec::new(),
        }
    }

    /// Create with translated label
    pub fn with_label(label: impl Into<Cow<'static, str>>, label_key: impl Into<Cow<'static, str>>) -> Self {
        Self {
            label: label.into(),
            label_key: label_key.into(),
            items: Vec::new(),
        }
    }

    /// Add an item to the menu
    pub fn item(mut self, item: MenuItem<M>) -> Self {
        self.items.push(item);
        self
    }

    /// Add multiple items
    pub fn items(mut self, items: impl IntoIterator<Item = MenuItem<M>>) -> Self {
        self.items.extend(items);
        self
    }

    /// Translate the menu label using the provided translator
    pub fn translate<F>(mut self, translator: F) -> Self
    where
        F: Fn(&str) -> String,
    {
        self.label = Cow::Owned(translator(&self.label_key));
        self
    }
}

/// A complete menu bar
#[derive(Clone)]
pub struct MenuBar<M: Clone> {
    /// The menus in this menu bar
    pub menus: Vec<Menu<M>>,
}

impl<M: Clone> MenuBar<M> {
    /// Create a new empty menu bar
    pub fn new() -> Self {
        Self { menus: Vec::new() }
    }

    /// Add a menu to the bar
    pub fn menu(mut self, menu: Menu<M>) -> Self {
        self.menus.push(menu);
        self
    }

    /// Add multiple menus
    pub fn menus(mut self, menus: impl IntoIterator<Item = Menu<M>>) -> Self {
        self.menus.extend(menus);
        self
    }
}

impl<M: Clone> Default for MenuBar<M> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Macros for declarative menu definition
// =============================================================================

/// Create a menu item from a command
///
/// # Usage
/// ```ignore
/// item!(cmd::FILE_NEW, Message::NewFile)
/// item!(cmd::FILE_NEW, Message::NewFile, enabled: false)
/// item!(cmd::VIEW_FULLSCREEN, Message::ToggleFullscreen, checked: is_fullscreen)
/// ```
#[macro_export]
macro_rules! menu_item {
    // Basic item: command and message
    ($cmd:expr, $msg:expr) => {
        $crate::ui::menu::MenuItem::command($cmd.clone(), $msg)
    };

    // Disabled item
    ($cmd:expr, $msg:expr, enabled: $enabled:expr) => {
        $crate::ui::menu::MenuItem::command($cmd.clone(), $msg).enabled($enabled)
    };

    // Checked item
    ($cmd:expr, $msg:expr, checked: $checked:expr) => {
        $crate::ui::menu::MenuItem::command_checked($cmd.clone(), $msg, $checked)
    };

    // Both enabled and checked
    ($cmd:expr, $msg:expr, enabled: $enabled:expr, checked: $checked:expr) => {
        $crate::ui::menu::MenuItem::command_checked($cmd.clone(), $msg, $checked).enabled($enabled)
    };
}

/// Create a menu separator
#[macro_export]
macro_rules! menu_separator {
    () => {
        $crate::ui::menu::MenuItem::separator()
    };
}

/// Create a submenu
///
/// # Usage
/// ```ignore
/// submenu!("Recent Files",
///     item!(cmd::FILE_OPEN_RECENT_1, Message::OpenRecent(1)),
///     item!(cmd::FILE_OPEN_RECENT_2, Message::OpenRecent(2)),
/// )
/// ```
#[macro_export]
macro_rules! menu_submenu {
    ($label:expr, $($item:expr),* $(,)?) => {
        $crate::ui::menu::MenuItem::submenu($label, vec![$($item),*])
    };
}

/// Create a menu (top-level dropdown)
///
/// # Usage
/// ```ignore
/// menu!("menu-file",
///     item!(cmd::FILE_NEW, Message::NewFile),
///     item!(cmd::FILE_OPEN, Message::OpenFile),
///     separator!(),
///     item!(cmd::FILE_SAVE, Message::SaveFile),
/// )
/// ```
#[macro_export]
macro_rules! menu {
    ($label_key:expr, $($item:expr),* $(,)?) => {
        $crate::ui::menu::Menu::new($label_key).items(vec![$($item),*])
    };
}

/// Create a menu bar with multiple menus
///
/// # Usage
/// ```ignore
/// menu_bar![
///     menu!("menu-file", ...),
///     menu!("menu-edit", ...),
///     menu!("menu-view", ...),
/// ]
/// ```
#[macro_export]
macro_rules! menu_bar {
    ($($menu:expr),* $(,)?) => {
        $crate::ui::menu::MenuBar::new().menus(vec![$($menu),*])
    };
}

// Re-export macros at module level
pub use menu;
pub use menu_bar;
pub use menu_item;
pub use menu_separator;
pub use menu_submenu;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        New,
        Open,
        Save,
        Undo,
        Redo,
    }

    #[test]
    fn test_menu_item_creation() {
        let cmd = CommandDef::new("file.new");
        let item: MenuItem<TestMessage> = MenuItem::command(cmd.clone(), TestMessage::New);

        match item {
            MenuItem::Command {
                command,
                message,
                enabled,
                checked,
            } => {
                assert_eq!(command.id, "file.new");
                assert_eq!(message, TestMessage::New);
                assert!(enabled);
                assert!(checked.is_none());
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[test]
    fn test_separator() {
        let sep: MenuItem<TestMessage> = MenuItem::separator();
        assert!(matches!(sep, MenuItem::Separator));
    }

    #[test]
    fn test_menu_creation() {
        let menu: Menu<TestMessage> = Menu::new("menu-file")
            .item(MenuItem::command(CommandDef::new("file.new"), TestMessage::New))
            .item(MenuItem::separator())
            .item(MenuItem::command(CommandDef::new("file.open"), TestMessage::Open));

        assert_eq!(menu.label_key, "menu-file");
        assert_eq!(menu.items.len(), 3);
    }

    #[test]
    fn test_menu_bar_creation() {
        let bar: MenuBar<TestMessage> = MenuBar::new()
            .menu(Menu::new("menu-file").item(MenuItem::command(CommandDef::new("file.new"), TestMessage::New)))
            .menu(Menu::new("menu-edit").item(MenuItem::command(CommandDef::new("edit.undo"), TestMessage::Undo)));

        assert_eq!(bar.menus.len(), 2);
    }

    #[test]
    fn test_menu_macros() {
        use crate::commands::cmd;

        let file_menu: Menu<TestMessage> = menu!(
            "menu-file",
            menu_item!(cmd::FILE_NEW, TestMessage::New),
            menu_separator!(),
            menu_item!(cmd::FILE_OPEN, TestMessage::Open),
        );

        assert_eq!(file_menu.items.len(), 3);
    }

    #[test]
    fn test_menu_bar_macro() {
        use crate::commands::cmd;

        let bar: MenuBar<TestMessage> = menu_bar![
            menu!(
                "menu-file",
                menu_item!(cmd::FILE_NEW, TestMessage::New),
                menu_item!(cmd::FILE_OPEN, TestMessage::Open),
            ),
            menu!(
                "menu-edit",
                menu_item!(cmd::EDIT_UNDO, TestMessage::Undo),
                menu_item!(cmd::EDIT_REDO, TestMessage::Redo),
            ),
        ];

        assert_eq!(bar.menus.len(), 2);
        assert_eq!(bar.menus[0].items.len(), 2);
        assert_eq!(bar.menus[1].items.len(), 2);
    }
}
