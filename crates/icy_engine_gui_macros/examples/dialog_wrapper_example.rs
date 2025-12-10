//! Example file demonstrating the dialog_wrapper macro usage
//!
//! This file shows how the dialog_wrapper macro reduces boilerplate for dialogs.

/*
=============================================================================
BASIC USAGE - Minimal form (everything derived from naming conventions)
=============================================================================

```rust
use icy_engine_gui::dialog_wrapper;

#[derive(Debug, Clone)]
pub enum SimpleDialogMessage {
    DoSomething,
    Cancel,
}

#[dialog_wrapper]
pub struct SimpleDialogState {
    pub some_field: String,
}

impl SimpleDialogState {
    pub fn new() -> Self {
        Self { some_field: String::new() }
    }

    pub fn handle_message(&mut self, message: SimpleDialogMessage) -> StateResult<()> {
        match message {
            SimpleDialogMessage::DoSomething => StateResult::Success(()),
            SimpleDialogMessage::Cancel => StateResult::Close,
        }
    }

    pub fn view<M: Clone + 'static>(&self, on_message: impl Fn(SimpleDialogMessage) -> M + Clone + 'static) -> Element<'static, M> {
        // Build your UI here...
        todo!()
    }
}
```

The macro derives:
- SimpleDialogState → SimpleDialogWrapper (wrapper struct)
- SimpleDialogState → SimpleDialogMessage (expected message enum name)
- result_type defaults to ()
- close_on_blur defaults to false

=============================================================================
WITH RESULT TYPE - For dialogs that return data on success
=============================================================================

```rust
use icy_engine_gui::dialog_wrapper;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ExportDialogMessage {
    UpdatePath(String),
    Export,
    Cancel,
}

#[dialog_wrapper(result_type = PathBuf)]
pub struct ExportDialogState {
    pub path: String,
}

impl ExportDialogState {
    pub fn handle_message(&mut self, message: ExportDialogMessage) -> StateResult<PathBuf> {
        match message {
            ExportDialogMessage::UpdatePath(p) => {
                self.path = p;
                StateResult::None
            }
            ExportDialogMessage::Export => {
                StateResult::Success(PathBuf::from(&self.path))
            }
            ExportDialogMessage::Cancel => StateResult::Close,
        }
    }
}
```

=============================================================================
WITH CLOSE ON BLUR - For lightweight dialogs that dismiss on click-away
=============================================================================

```rust
#[dialog_wrapper(close_on_blur = true)]
pub struct SauceDialogState { ... }
```

=============================================================================
WHAT THE MACRO GENERATES
=============================================================================

From `#[dialog_wrapper]` on `FooDialogState`, the macro generates:

```rust
// The state struct is kept as-is
pub struct FooDialogState { ... }

// Generated wrapper struct
pub struct FooDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(FooDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&FooDialogMessage> + Clone + 'static,
{
    pub state: FooDialogState,
    pub on_message: F,
    pub extract_message: E,
    on_confirm: Option<Box<dyn Fn() -> M + Send>>,  // or Fn(T) -> M if result_type specified
    on_cancel: Option<Box<dyn Fn() -> M + Send>>,
}

impl<M, F, E> FooDialogWrapper<M, F, E> { ... } {
    pub fn new(state: FooDialogState, on_message: F, extract_message: E) -> Self;
    pub fn on_confirm<G>(self, callback: G) -> Self;  // builder method
    pub fn on_cancel<G>(self, callback: G) -> Self;   // builder method
}

impl<M, F, E> Dialog<M> for FooDialogWrapper<M, F, E> {
    fn view(&self) -> Element<'_, M>;
    fn update(&mut self, message: &M) -> Option<DialogAction<M>>;
    fn request_cancel(&mut self) -> DialogAction<M>;
    fn request_confirm(&mut self) -> DialogAction<M>;
    fn handle_event(&mut self, event: &Event) -> Option<DialogAction<M>>;
    fn close_on_blur(&self) -> bool;
}
```

=============================================================================
USAGE WITH DialogStack
=============================================================================

```rust
use icy_engine_gui::dialog_msg;

// In your main application:
pub enum Message {
    ExportDialog(ExportDialogMessage),
    // ...
}

// Opening a dialog:
self.dialog_stack.push(
    ExportDialogWrapper::new(
        ExportDialogState::new(),
        Message::ExportDialog,
        |msg| match msg { Message::ExportDialog(m) => Some(m), _ => None },
    )
    .on_confirm(|path| Message::ExportComplete(path))
    .on_cancel(|| Message::DialogClosed)
);

// Or with the dialog_msg! macro:
self.dialog_stack.push(
    ExportDialogWrapper::new(
        ExportDialogState::new(),
        dialog_msg!(Message::ExportDialog),
    )
    .on_confirm(|path| Message::ExportComplete(path))
);
```

=============================================================================
WHAT YOU STILL NEED TO IMPLEMENT MANUALLY
=============================================================================

The macro does NOT generate:
1. `handle_message()` - Contains your dialog-specific business logic
2. `view()` - Contains your dialog UI
3. Builder functions like `export_dialog()` - Optional convenience wrappers

The macro handles all the boring Dialog trait boilerplate so you can focus
on the actual dialog logic.

=============================================================================
SPECIAL CASES - Custom theme() override
=============================================================================

If your dialog needs to override `theme()` (e.g., for live theme preview in
a settings dialog), you need to wrap the generated wrapper:

```rust
pub struct SettingsDialogWrapperWithTheme<M, F, E> {
    inner: SettingsDialogWrapper<M, F, E>,
}

impl<M, F, E> Dialog<M> for SettingsDialogWrapperWithTheme<M, F, E> {
    // Delegate all methods to inner...

    fn theme(&self) -> Option<iced::Theme> {
        Some(self.inner.state.get_preview_theme())
    }
}
```

This is rare - most dialogs don't need it.
*/
