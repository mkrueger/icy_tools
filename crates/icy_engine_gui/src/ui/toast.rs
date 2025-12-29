//! Toast notification system for displaying temporary messages.
//!
//! Toasts are small, non-intrusive notifications that appear briefly
//! and automatically disappear after a timeout.

use std::fmt;

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::time::{Duration, Instant};
use iced::widget::{button, container, row, text};
use iced::window;
use iced::{Alignment, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use super::{TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL};

/// Default timeout for toasts in seconds
pub const DEFAULT_TIMEOUT: u64 = 3;

/// Toast status/type for styling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastStatus {
    /// Informational toast (default)
    #[default]
    Info,
    /// Success toast
    Success,
    /// Warning toast
    Warning,
    /// Error toast
    Error,
}

impl ToastStatus {
    pub const ALL: &'static [Self] = &[Self::Info, Self::Success, Self::Warning, Self::Error];
}

impl fmt::Display for ToastStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToastStatus::Info => write!(f, "Info"),
            ToastStatus::Success => write!(f, "Success"),
            ToastStatus::Warning => write!(f, "Warning"),
            ToastStatus::Error => write!(f, "Error"),
        }
    }
}

/// A toast notification
#[derive(Debug, Clone, Default)]
pub struct Toast {
    /// The message to display
    pub message: String,
    /// The status/type of the toast
    pub status: ToastStatus,
}

impl Toast {
    /// Create a new info toast
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: ToastStatus::Info,
        }
    }

    /// Create a new success toast
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: ToastStatus::Success,
        }
    }

    /// Create a new warning toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: ToastStatus::Warning,
        }
    }

    /// Create a new error toast
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: ToastStatus::Error,
        }
    }
}

/// Toast manager widget that wraps content and displays toasts as an overlay
pub struct ToastManager<'a, Message> {
    content: Element<'a, Message>,
    toasts: Vec<Element<'a, Message>>,
    timeout_secs: u64,
    on_close: Box<dyn Fn(usize) -> Message + 'a>,
}

impl<'a, Message> ToastManager<'a, Message>
where
    Message: 'a + Clone,
{
    /// Create a new toast manager
    pub fn new(content: impl Into<Element<'a, Message>>, toasts: &'a [Toast], on_close: impl Fn(usize) -> Message + 'a) -> Self {
        let toasts = toasts
            .iter()
            .enumerate()
            .map(|(index, toast)| {
                let (icon, icon_color) = match toast.status {
                    ToastStatus::Info => ("ℹ", iced::Color::from_rgb(0.4, 0.6, 0.9)),
                    ToastStatus::Success => ("✓", iced::Color::from_rgb(0.3, 0.7, 0.3)),
                    ToastStatus::Warning => ("⚠", iced::Color::from_rgb(0.9, 0.7, 0.2)),
                    ToastStatus::Error => ("✕", iced::Color::from_rgb(0.9, 0.3, 0.3)),
                };

                container(
                    row![
                        text(icon).size(TEXT_SIZE_NORMAL).color(icon_color),
                        text(&toast.message).size(TEXT_SIZE_SMALL),
                        button(text("✕").size(TEXT_SIZE_SMALL))
                            .on_press((on_close)(index))
                            .padding([2, 6])
                            .style(close_button_style),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([8, 12])
                .style(toast_container_style)
                .into()
            })
            .collect();

        Self {
            content: content.into(),
            toasts,
            timeout_secs: DEFAULT_TIMEOUT,
            on_close: Box::new(on_close),
        }
    }

    /// Set the timeout in seconds
    pub fn timeout(self, seconds: u64) -> Self {
        Self { timeout_secs: seconds, ..self }
    }
}

fn toast_container_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(iced::Color {
            a: 0.95,
            ..theme.background.base
        })),
        border: iced::Border {
            color: theme.background.on.scale_alpha(0.2),
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: iced::Shadow {
            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

fn close_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let base_alpha = match status {
        button::Status::Hovered => 0.8,
        button::Status::Pressed => 1.0,
        _ => 0.5,
    };
    button::Style {
        background: None,
        text_color: theme.background.on.scale_alpha(base_alpha),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

impl<Message> Widget<Message, Theme, Renderer> for ToastManager<'_, Message> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn tag(&self) -> widget::tree::Tag {
        struct Marker;
        widget::tree::Tag::of::<Marker>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Vec::<Option<Instant>>::new())
    }

    fn children(&self) -> Vec<Tree> {
        std::iter::once(Tree::new(&self.content)).chain(self.toasts.iter().map(Tree::new)).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let instants = tree.state.downcast_mut::<Vec<Option<Instant>>>();

        // Invalidating removed instants to None allows us to remove
        // them here so that diffing for removed / new toast instants
        // is accurate
        instants.retain(Option::is_some);

        match (instants.len(), self.toasts.len()) {
            (old, new) if old > new => {
                instants.truncate(new);
            }
            (old, new) if old < new => {
                instants.extend(std::iter::repeat_n(Some(Instant::now()), new - old));
            }
            _ => {}
        }

        tree.diff_children(&std::iter::once(&self.content).chain(self.toasts.iter()).collect::<Vec<_>>());
    }

    fn operate(&mut self, state: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        operation.container(None, layout.bounds());
        self.content.as_widget_mut().operate(&mut state.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget_mut()
            .update(&mut state.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(&state.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(&self, state: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle, renderer: &Renderer) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(&state.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let instants = state.state.downcast_mut::<Vec<Option<Instant>>>();

        let (content_state, toasts_state) = state.children.split_at_mut(1);

        let content = self
            .content
            .as_widget_mut()
            .overlay(&mut content_state[0], layout, renderer, viewport, translation);

        let toasts = (!self.toasts.is_empty()).then(|| {
            overlay::Element::new(Box::new(ToastOverlay {
                position: layout.bounds().position() + translation,
                viewport: *viewport,
                toasts: &mut self.toasts,
                state: toasts_state,
                instants,
                on_close: &self.on_close,
                timeout_secs: self.timeout_secs,
            }))
        });

        let overlays = content.into_iter().chain(toasts).collect::<Vec<_>>();

        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

struct ToastOverlay<'a, 'b, Message> {
    position: Point,
    viewport: Rectangle,
    toasts: &'b mut [Element<'a, Message>],
    state: &'b mut [Tree],
    instants: &'b mut [Option<Instant>],
    on_close: &'b dyn Fn(usize) -> Message,
    timeout_secs: u64,
}

impl<Message> overlay::Overlay<Message, Theme, Renderer> for ToastOverlay<'_, '_, Message> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds);

        // Layout toasts in a vertical column on the right side
        layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            Length::Fill,
            Length::Fill,
            10.into(),
            10.0,
            Alignment::End,
            self.toasts,
            self.state,
        )
        .translate(Vector::new(self.position.x, self.position.y))
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = &event {
            self.instants.iter_mut().enumerate().for_each(|(index, maybe_instant)| {
                if let Some(instant) = maybe_instant.as_mut() {
                    let timeout = Duration::from_secs(self.timeout_secs);
                    let remaining = timeout.saturating_sub(instant.elapsed());

                    if remaining == Duration::ZERO {
                        maybe_instant.take();
                        shell.publish((self.on_close)(index));
                    } else {
                        shell.request_redraw_at(*now + remaining);
                    }
                }
            });
        }

        let viewport = layout.bounds();

        for (((child, state), layout), instant) in self
            .toasts
            .iter_mut()
            .zip(self.state.iter_mut())
            .zip(layout.children())
            .zip(self.instants.iter_mut())
        {
            let mut local_messages = vec![];
            let mut local_shell = Shell::new(&mut local_messages);

            child
                .as_widget_mut()
                .update(state, event, layout, cursor, renderer, clipboard, &mut local_shell, &viewport);

            if !local_shell.is_empty() {
                instant.take();
            }

            shell.merge(local_shell, std::convert::identity);
        }
    }

    fn draw(&self, renderer: &mut Renderer, theme: &Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor) {
        let viewport = layout.bounds();

        for ((child, state), layout) in self.toasts.iter().zip(self.state.iter()).zip(layout.children()) {
            child.as_widget().draw(state, renderer, theme, style, layout, cursor, &viewport);
        }
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        operation.container(None, layout.bounds());

        self.toasts
            .iter_mut()
            .zip(self.state.iter_mut())
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                child.as_widget_mut().operate(state, layout, renderer, operation);
            });
    }

    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.toasts
            .iter()
            .zip(self.state.iter())
            .zip(layout.children())
            .map(|((child, state), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(state, layout, cursor, &self.viewport, renderer)
                    .max(if cursor.is_over(layout.bounds()) {
                        mouse::Interaction::Pointer
                    } else {
                        mouse::Interaction::default()
                    })
            })
            .max()
            .unwrap_or_default()
    }
}

impl<'a, Message> From<ToastManager<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(manager: ToastManager<'a, Message>) -> Self {
        Element::new(manager)
    }
}
