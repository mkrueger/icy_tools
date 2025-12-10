use iced::{
    Alignment, Border, Color, Element, Event, Length, Padding, Theme,
    widget::{Space, column, container, row, scrollable, text},
};

use super::dialog::{Dialog, DialogAction};
use super::{DIALOG_SPACING, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, primary_button};
use crate::{ButtonType, commands::CommandDef};

/// Category translation function type
pub type CategoryTranslator = Box<dyn Fn(&str) -> String>;

/// Configuration for the help dialog
pub struct HelpDialogConfig {
    /// Title of the help dialog
    pub title: String,
    /// Subtitle/description
    pub subtitle: String,
    /// Icon for the title (emoji)
    pub title_icon: String,
    /// Commands to display
    pub commands: Vec<CommandDef>,
    /// Function to translate category keys to display names
    category_translator: Option<CategoryTranslator>,
}

impl HelpDialogConfig {
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            title_icon: "⌨".to_string(),
            commands: Vec::new(),
            category_translator: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.title_icon = icon.into();
        self
    }

    pub fn with_commands(mut self, commands: Vec<CommandDef>) -> Self {
        self.commands = commands;
        self
    }

    /// Set a translator function for category names
    /// The function receives the category key (e.g., "connection") and should return
    /// the translated name (e.g., "Connection")
    pub fn with_category_translator<F>(mut self, translator: F) -> Self
    where
        F: Fn(&str) -> String + 'static,
    {
        self.category_translator = Some(Box::new(translator));
        self
    }

    /// Translate a category key to its display name
    fn translate_category(&self, key: &str) -> String {
        if let Some(translator) = &self.category_translator {
            translator(key)
        } else {
            // Default: capitalize first letter
            let mut chars = key.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        }
    }
}

/// Create a keyboard shortcut pill
fn pill<Message: 'static>(content: &str) -> Element<'static, Message> {
    container(
        text(content.to_owned())
            .size(TEXT_SIZE_NORMAL)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            })
            .style(|theme: &Theme| text::Style {
                color: Some(theme.palette().text),
                ..Default::default()
            }),
    )
    .padding(Padding::from([5, 12]))
    .style(|theme: &Theme| container::Style {
        background: Some(theme.palette().primary.into()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// Create a key group from a key string like "Ctrl+Shift+N" or "Alt D"
/// Use "++" to represent a literal "+" key (e.g., "Ctrl++" for Ctrl+Plus)
fn key_group<Message: 'static + Clone>(keys: &str) -> Element<'static, Message> {
    // Replace "++" with a placeholder, then split, then restore
    const PLUS_PLACEHOLDER: &str = "\x00PLUS\x00";
    let escaped = keys.replace("++", PLUS_PLACEHOLDER);

    let parts: Vec<String> = escaped
        .split(' ')
        .flat_map(|chunk| {
            if chunk.contains('+') {
                chunk.split('+').map(|s| s.to_string()).collect::<Vec<_>>()
            } else {
                vec![chunk.to_string()]
            }
        })
        .map(|s| s.replace(PLUS_PLACEHOLDER, "+"))
        .filter(|s| !s.is_empty())
        .collect();

    let mut r = row![].spacing(DIALOG_SPACING).align_y(Alignment::Center);
    for (i, p) in parts.iter().enumerate() {
        r = r.push(pill::<Message>(p));
        if i + 1 < parts.len() {
            r = r.push(text("+").size(TEXT_SIZE_NORMAL).style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.base.text),
                ..Default::default()
            }));
        }
    }
    r.into()
}

/// Create a category header
fn category_header<Message: 'static>(name: &str) -> container::Container<'static, Message> {
    container(
        row![text(name.to_owned()).size(16).style(|theme: &Theme| text::Style {
            color: Some(theme.palette().text),
            ..Default::default()
        }),]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10, 24]))
    .style(|t: &Theme| container::Style {
        background: Some(
            Color::from_rgba(
                t.extended_palette().background.weak.color.r,
                t.extended_palette().background.weak.color.g,
                t.extended_palette().background.weak.color.b,
                0.3,
            )
            .into(),
        ),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
}

/// Create the help dialog content
///
/// # Arguments
/// * `config` - The help dialog configuration
/// * `close_message` - Message to send when closing the dialog
pub fn help_dialog_content<Message: Clone + 'static>(config: &HelpDialogConfig, close_message: Message) -> Element<'static, Message> {
    // Title block
    let title = container(
        row![
            text(config.title_icon.clone()).size(24),
            Space::new().width(10),
            column![
                text(config.title.clone()).size(22).style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().text),
                    ..Default::default()
                }),
                text(config.subtitle.clone()).size(TEXT_SIZE_SMALL).style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().secondary.base.color),
                    ..Default::default()
                }),
            ]
            .spacing(2)
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: 16.0,
        right: 30.0,
        bottom: 8.0,
        left: 30.0,
    });

    // Group commands by category
    let mut categories: std::collections::BTreeMap<String, Vec<&CommandDef>> = std::collections::BTreeMap::new();
    for cmd in &config.commands {
        let category = cmd.category.clone().unwrap_or_else(|| "general".to_string());
        categories.entry(category).or_default().push(cmd);
    }

    // Build scrollable content
    let mut content = column![].spacing(0);

    for (cat_index, (category_key, commands)) in categories.iter().enumerate() {
        let category_name = config.translate_category(category_key);
        let header = category_header::<Message>(&category_name);
        content = content.push(header.width(Length::Fill));

        for (row_index, cmd) in commands.iter().enumerate() {
            let shaded = (cat_index + row_index) % 2 == 0;
            let keys = cmd.primary_hotkey_display().unwrap_or_default();
            let action = cmd.label_menu.clone();
            let desc = cmd.label_description.clone();

            let shortcut_row = container(
                row![
                    container(key_group::<Message>(&keys)).width(Length::Fixed(200.0)),
                    Space::new().width(16),
                    container(text(action).size(TEXT_SIZE_NORMAL).style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text),
                        ..Default::default()
                    }))
                    .width(Length::Fixed(140.0)),
                    Space::new().width(12),
                    text(desc)
                        .size(TEXT_SIZE_NORMAL)
                        .style(|theme: &Theme| text::Style {
                            color: Some(theme.palette().text),
                            ..Default::default()
                        })
                        .width(Length::Fill),
                ]
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([7, 30]))
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Style {
                background: if shaded { Some(Color::from_rgba(0.0, 0.0, 0.0, 0.015).into()) } else { None },
                ..Default::default()
            });

            content = content.push(shortcut_row);
        }

        content = content.push(container(Space::new().height(4)).width(Length::Fill).style(|_theme: &Theme| container::Style {
            background: None,
            ..Default::default()
        }));
    }

    // Footer
    let footer = container(
        row![
            Space::new().width(Length::Fill),
            primary_button(format!("{}", ButtonType::Close), Some(close_message)).padding(Padding::from([5, 20])),
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([12, 30]))
    .width(Length::Fill);

    // Main dialog container
    let dialog = container(
        column![
            title,
            container(scrollable(container(content).padding(Padding::from([0, 0])).width(Length::Fill)).height(Length::Fill))
                .height(Length::FillPortion(1))
                .padding(Padding::from([0, 0])),
            container(Space::new().height(1)).width(Length::Fill).style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                ..Default::default()
            }),
            footer,
        ]
        .spacing(0),
    )
    .width(Length::Fixed(700.0))
    .height(Length::Fixed(500.0))
    .style(|theme: &Theme| container::Style {
        background: Some(iced::Background::Color(theme.extended_palette().background.base.color)),
        border: Border {
            color: theme.extended_palette().background.strong.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: iced::Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    });

    container(dialog)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Create a full help dialog with modal overlay
///
/// # Arguments
/// * `background` - The background content
/// * `config` - The help dialog configuration  
/// * `close_message` - Message to send when closing the dialog
pub fn help_dialog<'a, Message: Clone + 'static>(
    background: Element<'a, Message>,
    config: &'a HelpDialogConfig,
    close_message: Message,
) -> Element<'a, Message> {
    let content = help_dialog_content(config, close_message.clone());
    super::modal(background, content, close_message)
}

/// Returns the modifier symbol for the current platform
/// Returns "⌘" on macOS, "Ctrl" on other platforms
pub fn platform_mod_symbol() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
}

/// Returns true if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// A wrapper that makes HelpDialogConfig usable with the Dialog trait.
///
/// This wrapper owns the HelpDialogConfig and provides Dialog trait implementation
/// that can be used with DialogStack.
pub struct HelpDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn() -> M + Clone + Send + 'static,
{
    config: HelpDialogConfig,
    on_close: F,
    _phantom: std::marker::PhantomData<M>,
}

impl<M, F> HelpDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn() -> M + Clone + Send + 'static,
{
    /// Create a new help dialog wrapper.
    ///
    /// # Arguments
    /// * `config` - The help dialog configuration
    /// * `on_close` - Callback to generate the close message
    pub fn new(config: HelpDialogConfig, on_close: F) -> Self {
        Self {
            config,
            on_close,
            _phantom: std::marker::PhantomData,
        }
    }
}

// ============================================================================
// Builder function for help dialog
// ============================================================================

/// Create a help dialog showing keyboard shortcuts.
///
/// # Example
/// ```ignore
/// use crate::commands::COMMAND_DEFINITIONS;
///
/// dialog_stack.push(help_dialog_for(
///     "Keyboard Shortcuts",
///     "Available keyboard shortcuts",
///     COMMAND_DEFINITIONS.to_vec(),
///     || Message::CloseHelp,
/// ));
/// ```
pub fn help_dialog_for<M, F>(title: impl Into<String>, subtitle: impl Into<String>, commands: Vec<CommandDef>, on_close: F) -> HelpDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn() -> M + Clone + Send + 'static,
{
    HelpDialogWrapper::new(HelpDialogConfig::new(title, subtitle).with_commands(commands), on_close)
}

/// Create a help dialog with a custom category translator.
///
/// # Example
/// ```ignore
/// dialog_stack.push(help_dialog_with_translator(
///     "Keyboard Shortcuts",
///     "Available keyboard shortcuts",
///     commands,
///     |category| translate_category(category),
///     || Message::CloseHelp,
/// ));
/// ```
pub fn help_dialog_with_translator<M, F, T>(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    commands: Vec<CommandDef>,
    category_translator: T,
    on_close: F,
) -> HelpDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn() -> M + Clone + Send + 'static,
    T: Fn(&str) -> String + 'static,
{
    HelpDialogWrapper::new(
        HelpDialogConfig::new(title, subtitle)
            .with_commands(commands)
            .with_category_translator(category_translator),
        on_close,
    )
}

impl<M, F> Dialog<M> for HelpDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn() -> M + Clone + Send + 'static,
{
    fn view(&self) -> Element<'_, M> {
        let close_msg = (self.on_close)();
        help_dialog_content(&self.config, close_msg)
    }

    fn request_cancel(&mut self) -> DialogAction<M> {
        DialogAction::CloseWith((self.on_close)())
    }

    fn request_confirm(&mut self) -> DialogAction<M> {
        // Help dialog has no confirm action, only close
        DialogAction::None
    }

    fn handle_event(&mut self, _event: &Event) -> Option<DialogAction<M>> {
        None
    }

    fn close_on_blur(&self) -> bool {
        true
    }
}
