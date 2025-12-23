//! A focusable widget for Iced.
//!
//! It provides a `focus` function that you can wrap elements in to allow them to
//! be focusable. The focused element will be outlined with a border, which you can
//! style as needed. It also enables you to (relatively easily) support tabbing
//! between focusable elements in your app and handle events when focused without
//! requiring a custom widget.
//!
//! MIT License
//! Copyright (c) 2024 Brady Simon
//! https://github.com/bradysimon/iced_focus

use iced::advanced::renderer::Quad;
use iced::advanced::widget::{operation, tree, Tree};
use iced::advanced::{self, layout, overlay, renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::border::Radius;
use iced::keyboard::key;
use iced::widget::Id;
use iced::{keyboard, mouse as iced_mouse, touch, Border, Color, Element, Event, Font, Length, Padding, Rectangle, Shadow, Size, Vector};

/// A boxed closure that takes an [`Event`] and an [`Id`], and produces an optional message.
/// The event is only sent if the widget is focused, and returning `None` ignores the event.
pub type OnEvent<'a, Message> = Box<dyn Fn(Event, Id) -> Option<Message> + 'a>;

/// Allows a child element to be focusable. This can help you arbitrarily focus
/// elements in your application, and handle events only when focused.
pub struct Focus<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: advanced::Renderer,
    Theme: Catalog,
{
    /// The content that will become focusable.
    content: Element<'a, Message, Theme, Renderer>,
    /// The unique identifier of this widget.
    id: Id,
    /// A closure that potentially produces a message when an event occurs.
    /// The event is only sent if the widget is focused.
    on_event: Option<OnEvent<'a, Message>>,
    /// The message to emit when this widget gains focus.
    on_focus: Option<Message>,
    /// The message to emit when this widget loses focus.
    on_blur: Option<Message>,
    /// The message to emit when the next widget in the app should be focused.
    /// You typically want to call [`iced::widget::operation::focus_next()`] when this happens.
    focus_next: Option<Message>,
    /// The message to emit when the previous widget in the app should be focused.
    /// You typically want to call [`iced::widget::operation::focus_previous()`] when this happens.
    focus_previous: Option<Message>,
    /// The style of the [`Focus`].
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Focus<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: advanced::Renderer,
    Theme: Catalog,
{
    /// Creates a new [`Focus`] wrapping the given content.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            id: Id::unique(),
            on_event: None,
            on_focus: None,
            on_blur: None,
            focus_next: None,
            focus_previous: None,
            class: Theme::default(),
        }
    }

    /// Sets the unique identifier of this widget.
    #[must_use]
    pub fn id(mut self, id: Id) -> Self {
        self.id = id;
        self
    }

    /// Sets the event handler for this widget. Return `None` to ignore the event.
    #[must_use]
    pub fn on_event(mut self, on_event: impl Fn(Event, Id) -> Option<Message> + 'a) -> Self {
        self.on_event = Some(Box::new(on_event));
        self
    }

    /// Optionally sets the event handler for this widget. Return `None` to ignore the event.
    #[must_use]
    pub fn on_event_maybe(mut self, on_event: Option<impl Fn(Event, Id) -> Option<Message> + 'a>) -> Self {
        self.on_event = on_event.map(|f| Box::new(f) as _);
        self
    }

    /// The message to emit when this widget gains focus.
    #[must_use]
    pub fn on_focus(mut self, on_focus: Message) -> Self {
        self.on_focus = Some(on_focus);
        self
    }

    /// The message to emit when this widget loses focus.
    #[must_use]
    pub fn on_blur(mut self, on_blur: Message) -> Self {
        self.on_blur = Some(on_blur);
        self
    }

    /// Emits the `message` when the next widget in the app should be focused,
    /// typically calling `iced::widget::operation::focus_next()`.
    ///
    /// This is necesssary until Iced supports focus navigation natively.
    #[must_use]
    pub fn focus_next(mut self, message: Message) -> Self {
        self.focus_next = Some(message);
        self
    }

    /// Emits the `message` when the previous widget in the app should be focused,
    /// typically calling `iced::widget::operation::focus_previous()`.
    ///
    /// This is necesssary until Iced supports focus navigation natively.
    #[must_use]
    pub fn focus_previous(mut self, message: Message) -> Self {
        self.focus_previous = Some(message);
        self
    }

    /// Sets the style of the [`Focus`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }
}

#[derive(Debug, Default, Clone)]
struct State {
    /// Whether the widget is currently focused.
    is_focused: bool,
    /// Whether the focus state has changed from outside this widget.
    /// Used to trigger on_focus/on_blur messages if a `Task` changes focus.
    changed: bool,
}

impl operation::Focusable for State {
    fn focus(&mut self) {
        self.changed = !self.is_focused;
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.changed = self.is_focused;
        self.is_focused = false;
    }

    fn is_focused(&self) -> bool {
        self.is_focused
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Focus<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: advanced::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> widget::tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: iced_mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn operation::Operation) {
        let state = tree.state.downcast_mut::<State>();

        operation.focusable(Some(&self.id), layout.bounds(), state);
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: iced_mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget_mut()
            .update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);

        // Update focus state based on mouse and touch events
        let state = tree.state.downcast_mut::<State>();
        if matches!(
            event,
            Event::Mouse(iced_mouse::Event::ButtonPressed(iced_mouse::Button::Left)) | Event::Touch(touch::Event::FingerPressed { .. })
        ) {
            if !state.is_focused && cursor.is_over(layout.bounds()) {
                state.is_focused = true;
                shell.request_redraw();
                if let Some(on_focus) = &self.on_focus {
                    shell.publish(on_focus.clone());
                }
                shell.capture_event();
            }

            if state.is_focused && !cursor.is_over(layout.bounds()) {
                state.is_focused = false;
                shell.request_redraw();
                if let Some(on_blur) = &self.on_blur {
                    shell.publish(on_blur.clone());
                }
            }
        }

        // Call on_focus/on_blur if focus state changed from outside
        if state.changed {
            state.changed = false;
            if state.is_focused {
                if let Some(on_focus) = &self.on_focus {
                    shell.publish(on_focus.clone());
                }
            } else if let Some(on_blur) = &self.on_blur {
                shell.publish(on_blur.clone());
            }
        }

        // Handle Tab key for focus navigation
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
            if key.as_ref() == keyboard::Key::Named(key::Named::Tab) && state.is_focused {
                if modifiers.shift() {
                    if let Some(focus_previous) = &self.focus_previous {
                        shell.publish(focus_previous.clone());
                    }
                } else if let Some(focus_next) = &self.focus_next {
                    shell.publish(focus_next.clone());
                }
                shell.request_redraw();
                shell.capture_event();
            }
        }

        // Call the user-defined event handler if focused
        if let Some(on_event) = &self.on_event {
            if state.is_focused {
                if let Some(message) = on_event(event.clone(), self.id.clone()) {
                    shell.publish(message);
                    shell.capture_event();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: iced_mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> iced_mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let child_overlay = self
            .content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation);

        let state = tree.state.downcast_ref::<State>();
        let overlay = state.is_focused.then(|| {
            overlay::Element::new(Box::new(Overlay {
                bounds: layout.bounds(),
                class: &self.class,
            }))
        });

        let children: Vec<_> = child_overlay.into_iter().chain(overlay).collect();
        if children.is_empty() {
            None
        } else {
            Some(overlay::Group::with_children(children).overlay())
        }
    }
}

/// An overlay that draws a focus indicator around the focused widget.
struct Overlay<'a, 'b, Theme>
where
    Theme: Catalog,
{
    bounds: Rectangle,
    class: &'a <Theme as Catalog>::Class<'b>,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer> for Overlay<'_, '_, Theme>
where
    Theme: Catalog,
    Renderer: advanced::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> layout::Node {
        layout::Node::new(bounds)
    }

    fn draw(&self, renderer: &mut Renderer, theme: &Theme, _style: &renderer::Style, _layout: Layout<'_>, _cursor: iced_mouse::Cursor) {
        let style = theme.style(self.class);
        renderer.fill_quad(
            Quad {
                border: style.border,
                bounds: self.bounds.expand(Padding::new(style.offset + style.border.width)),
                shadow: Shadow::default(),
                snap: style.snap,
            },
            Color::TRANSPARENT,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Focus<'a, Message, Theme, Renderer>> for iced::Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: advanced::text::Renderer<Font = Font> + 'a,
    Theme: Catalog + 'a,
{
    fn from(focus: Focus<'a, Message, Theme, Renderer>) -> iced::Element<'a, Message, Theme, Renderer> {
        Self::new(focus)
    }
}

/// Allows a child element to be focusable. This can help you arbitrarily focus
/// elements in your application, and handle events only when focused.
pub fn focus<'a, Message, Theme, Renderer>(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Focus<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: advanced::Renderer + 'a,
    Theme: Catalog + 'a,
{
    Focus::new(content)
}

/// The style of a [`Focus`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The border to draw around the focused widget.
    pub border: Border,
    /// The distance between the border and the focused widget.
    pub offset: f32,
    /// Whether the widget should be snapped to the pixel grid.
    pub snap: bool,
}

impl Style {
    /// Rounds the corners of the border.
    pub fn rounded(mut self, radius: impl Into<Radius>) -> Self {
        self.border.radius = radius.into();
        self
    }

    /// Sets the color of the border.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.border.color = color.into();
        self
    }

    /// Sets the width of the border.
    pub fn width(mut self, width: f32) -> Self {
        self.border.width = width;
        self
    }

    /// Sets the offset between the border and the focused widget.
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
        self
    }
}

/// The theme catalog of a [`Focus`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default_style)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// A styling function for a [`Focus`].
///
/// This is just a boxed closure: `Fn(&Theme, Status) -> Style`.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl<Theme> From<Style> for StyleFn<'_, Theme> {
    fn from(style: Style) -> Self {
        Box::new(move |_theme| style)
    }
}

/// The default style for a focused widget.
pub fn default_style(theme: &iced::Theme) -> Style {
    let palette = theme.extended_palette();
    Style {
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.primary.strong.color,
        },
        offset: 0.0,
        snap: true,
    }
}

/// A style that shows no border (invisible focus indicator).
pub fn no_border_style(_theme: &iced::Theme) -> Style {
    Style {
        border: Border::default().width(0.0),
        offset: 0.0,
        snap: true,
    }
}

/// A focus style matching iced's text_input widget - integrates visually.
pub fn list_focus_style(theme: &iced::Theme) -> Style {
    let palette = theme.extended_palette();
    Style {
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.primary.strong.color,
        },
        offset: 0.0,
        snap: true,
    }
}
