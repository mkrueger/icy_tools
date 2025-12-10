//! Test file demonstrating the dialog_wrapper macro usage

// This file shows how the dialog_wrapper macro can be used to reduce boilerplate.
// It's not meant to be compiled as part of the crate, but serves as documentation.

/*
Example of what the macro generates from:

```rust
use icy_engine_gui::dialog_wrapper;
use std::sync::Arc;
use parking_lot::Mutex;
use std::path::PathBuf;

#[dialog_wrapper(
    state = ExportDialogState,
    internal_message = ExportDialogMessage,
    cancel_message = Cancel,
    confirm_message = Export,
)]
pub struct ExportDialogWrapper {
    screen: Arc<Mutex<Box<dyn Screen>>>,
    #[callback(PathBuf)]
    on_exported: _,
    #[callback]
    on_cancelled: _,
}
```

The macro generates approximately this code:

```rust
pub struct ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + 'static,
{
    state: ExportDialogState,
    on_message: F,
    extract_message: E,
    screen: Arc<Mutex<Box<dyn Screen>>>,
    on_exported: Option<Box<dyn Fn(PathBuf) -> M + Send>>,
    on_cancelled: Option<Box<dyn Fn() -> M + Send>>,
}

impl<M, F, E> ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + 'static,
{
    pub fn new(
        state: ExportDialogState,
        screen: Arc<Mutex<Box<dyn Screen>>>,
        on_message: F,
        extract_message: E,
    ) -> Self {
        Self {
            state,
            on_message,
            extract_message,
            screen,
            on_exported: None,
            on_cancelled: None,
        }
    }

    pub fn on_exported<G>(mut self, callback: G) -> Self
    where
        G: Fn(PathBuf) -> M + Send + 'static,
    {
        self.on_exported = Some(Box::new(callback));
        self
    }

    pub fn on_cancelled<G>(mut self, callback: G) -> Self
    where
        G: Fn() -> M + Send + 'static,
    {
        self.on_cancelled = Some(Box::new(callback));
        self
    }

    pub fn state(&self) -> &ExportDialogState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut ExportDialogState {
        &mut self.state
    }
}

impl<M, F, E> Dialog<M> for ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + Send + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + Send + 'static,
{
    fn view(&self) -> iced::Element<'_, M> {
        self.state.view(self.on_message.clone())
    }

    fn update(&mut self, message: &M) -> Option<DialogAction<M>> {
        let dialog_msg = (self.extract_message)(message)?;
        Some(self.handle_message(dialog_msg.clone()))
    }

    fn request_cancel(&mut self) -> DialogAction<M> {
        if let Some(ref callback) = self.on_cancelled {
            DialogAction::CloseWith(callback())
        } else {
            DialogAction::Close
        }
    }

    fn request_confirm(&mut self) -> DialogAction<M> {
        self.handle_message(ExportDialogMessage::Export)
    }

    fn handle_event(&mut self, _event: &iced::Event) -> Option<DialogAction<M>> {
        None
    }

    fn close_on_blur(&self) -> bool {
        false
    }
}
```

Note: The macro does NOT generate:
- The `handle_message()` method - this contains dialog-specific logic
- Builder functions like `export_dialog()` - these have custom signatures

The user still needs to implement `handle_message()` manually, which is the
dialog-specific business logic.
*/
