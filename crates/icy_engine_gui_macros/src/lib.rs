//! Procedural macros for icy_engine_gui dialog wrappers.
//!
//! This crate provides the `dialog_wrapper` attribute macro that generates
//! boilerplate code for dialog wrappers that implement the `Dialog` trait.
//!
//! # Example
//!
//! ```ignore
//! use icy_engine_gui_macros::dialog_wrapper;
//!
//! // Minimal form - everything derived from naming conventions
//! #[dialog_wrapper]
//! pub struct SimpleDialogState { ... }
//! // -> SimpleDialogWrapper, SimpleDialogMessage, result_type = ()
//!
//! // With result type
//! #[dialog_wrapper(result_type = PathBuf)]
//! pub struct ExportDialogState { ... }
//! // -> ExportDialogWrapper, ExportDialogMessage, result_type = PathBuf
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemStruct, Token, Type,
};

/// Configuration for the dialog wrapper macro
struct DialogWrapperConfig {
    /// The internal message type (e.g., ExportDialogMessage) - derived if not specified
    internal_message: Option<Type>,
    /// The result type for StateResult::Success(T) (e.g., PathBuf) - defaults to ()
    result_type: Option<Type>,
    /// Whether to close on blur (default: false)
    close_on_blur: bool,
    /// Dialog style (Modal or Fullscreen) - defaults to Modal
    style: Option<Ident>,
}

impl Parse for DialogWrapperConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut internal_message: Option<Type> = None;
        let mut result_type: Option<Type> = None;
        let mut close_on_blur = false;
        let mut style: Option<Ident> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "internal_message" => {
                    internal_message = Some(input.parse()?);
                }
                "result_type" => {
                    result_type = Some(input.parse()?);
                }
                "close_on_blur" => {
                    let lit: syn::LitBool = input.parse()?;
                    close_on_blur = lit.value;
                }
                "style" => {
                    style = Some(input.parse()?);
                }
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown config key: {}", other)));
                }
            }

            // Consume optional trailing comma
            let _ = input.parse::<Token![,]>();
        }

        Ok(DialogWrapperConfig {
            internal_message,
            result_type,
            close_on_blur,
            style,
        })
    }
}

/// Generate the dialog wrapper implementation
fn generate_wrapper(config: &DialogWrapperConfig, state_struct: &ItemStruct) -> syn::Result<TokenStream2> {
    let state_name = &state_struct.ident;
    let state_vis = &state_struct.vis;
    let close_on_blur = config.close_on_blur;

    // Generate style() implementation
    let style_impl = if let Some(style_ident) = &config.style {
        let style_str = style_ident.to_string();
        match style_str.as_str() {
            "Fullscreen" => quote! {
                fn style(&self) -> icy_engine_gui::ui::DialogStyle {
                    icy_engine_gui::ui::DialogStyle::Fullscreen
                }
            },
            "Modal" | _ => quote! {
                fn style(&self) -> icy_engine_gui::ui::DialogStyle {
                    icy_engine_gui::ui::DialogStyle::Modal
                }
            },
        }
    } else {
        // Default: Modal (inherited from trait default)
        quote! {}
    };

    let state_name_str = state_name.to_string();

    // Generate wrapper name from state name:
    // FooState -> FooWrapper, FooDialogState -> FooDialogWrapper
    let wrapper_name_str = if state_name_str.ends_with("State") {
        format!("{}Wrapper", &state_name_str[..state_name_str.len() - 5])
    } else {
        format!("{}Wrapper", state_name_str)
    };
    let wrapper_name = Ident::new(&wrapper_name_str, state_name.span());

    // Generate message type name from state name if not provided:
    // FooState -> FooMessage, FooDialogState -> FooDialogMessage
    let internal_message: TokenStream2 = if let Some(msg_type) = &config.internal_message {
        quote! { #msg_type }
    } else {
        let message_name_str = if state_name_str.ends_with("State") {
            format!("{}Message", &state_name_str[..state_name_str.len() - 5])
        } else {
            format!("{}Message", state_name_str)
        };
        let message_name = Ident::new(&message_name_str, state_name.span());
        quote! { #message_name }
    };

    // Keep the original state struct
    let state_attrs = &state_struct.attrs;
    let state_generics = &state_struct.generics;
    let state_fields = &state_struct.fields;

    // Generate callback field definitions - always on_confirm(T) and on_cancel()
    let (on_confirm_field, on_confirm_builder, on_confirm_init, success_action) = if let Some(result_type) = &config.result_type {
        (
            quote! { on_confirm: Option<Box<dyn Fn(#result_type) -> M + Send>> },
            quote! {
                /// Set callback for successful confirmation.
                /// Called with the result value when dialog completes successfully.
                pub fn on_confirm<G>(mut self, callback: G) -> Self
                where
                    G: Fn(#result_type) -> M + Send + 'static,
                {
                    self.on_confirm = Some(Box::new(callback));
                    self
                }
            },
            quote! { on_confirm: None },
            quote! {
                if let Some(ref callback) = self.on_confirm {
                    icy_engine_gui::ui::DialogAction::CloseWith(callback(value))
                } else {
                    icy_engine_gui::ui::DialogAction::Close
                }
            },
        )
    } else {
        // No result type - on_confirm takes no argument, value is ()
        (
            quote! { on_confirm: Option<Box<dyn Fn() -> M + Send>> },
            quote! {
                /// Set callback for successful confirmation.
                pub fn on_confirm<G>(mut self, callback: G) -> Self
                where
                    G: Fn() -> M + Send + 'static,
                {
                    self.on_confirm = Some(Box::new(callback));
                    self
                }
            },
            quote! { on_confirm: None },
            quote! {
                if let Some(ref callback) = self.on_confirm {
                    icy_engine_gui::ui::DialogAction::CloseWith(callback())
                } else {
                    icy_engine_gui::ui::DialogAction::Close
                }
            },
        )
    };

    // Generate request_cancel implementation - just calls on_cancel if set
    let request_cancel_impl = quote! {
        fn request_cancel(&mut self) -> icy_engine_gui::ui::DialogAction<M> {
            if let Some(ref callback) = self.on_cancel {
                icy_engine_gui::ui::DialogAction::CloseWith(callback())
            } else {
                icy_engine_gui::ui::DialogAction::Close
            }
        }
    };

    // Generate request_confirm implementation - returns None (dialog handles it via messages)
    let request_confirm_impl = quote! {
        fn request_confirm(&mut self) -> icy_engine_gui::ui::DialogAction<M> {
            icy_engine_gui::ui::DialogAction::None
        }
    };

    Ok(quote! {
        // Original state struct (unchanged)
        #(#state_attrs)*
        #state_vis struct #state_name #state_generics #state_fields

        /// A wrapper that makes the dialog state usable with the Dialog trait.
        /// Generated by `#[dialog_wrapper]` macro.
        #state_vis struct #wrapper_name<M, F, E>
        where
            M: Clone + Send + 'static,
            F: Fn(#internal_message) -> M + Clone + 'static,
            E: Fn(&M) -> Option<&#internal_message> + Clone + 'static,
        {
            /// The dialog state
            pub state: #state_name,
            /// Message wrapper function
            pub on_message: F,
            /// Message extractor function
            pub extract_message: E,
            /// Callback for successful confirmation
            #on_confirm_field,
            /// Callback for cancel/close
            on_cancel: Option<Box<dyn Fn() -> M + Send>>,
        }

        impl<M, F, E> #wrapper_name<M, F, E>
        where
            M: Clone + Send + 'static,
            F: Fn(#internal_message) -> M + Clone + 'static,
            E: Fn(&M) -> Option<&#internal_message> + Clone + 'static,
        {
            /// Create a new dialog wrapper.
            pub fn new(
                state: #state_name,
                on_message: F,
                extract_message: E,
            ) -> Self {
                Self {
                    state,
                    on_message,
                    extract_message,
                    #on_confirm_init,
                    on_cancel: None,
                }
            }

            #on_confirm_builder

            /// Set callback for cancel/close.
            /// Called when dialog is cancelled or closed without confirmation.
            pub fn on_cancel<G>(mut self, callback: G) -> Self
            where
                G: Fn() -> M + Send + 'static,
            {
                self.on_cancel = Some(Box::new(callback));
                self
            }

            /// Handle a dialog message.
            /// Calls `state.handle_message()` and maps the StateResult to DialogAction.
            pub fn handle_message(&mut self, message: #internal_message) -> icy_engine_gui::ui::DialogAction<M> {
                match self.state.handle_message(message) {
                    icy_engine_gui::ui::StateResult::Success(value) => {
                        // Success with value
                        #success_action
                    }
                    icy_engine_gui::ui::StateResult::Close => {
                        // Cancelled
                        if let Some(ref callback) = self.on_cancel {
                            icy_engine_gui::ui::DialogAction::CloseWith(callback())
                        } else {
                            icy_engine_gui::ui::DialogAction::Close
                        }
                    }
                    icy_engine_gui::ui::StateResult::None => {
                        // Keep dialog open
                        icy_engine_gui::ui::DialogAction::None
                    }
                }
            }

            /// Get a reference to the inner state
            pub fn state(&self) -> &#state_name {
                &self.state
            }

            /// Get a mutable reference to the inner state
            pub fn state_mut(&mut self) -> &mut #state_name {
                &mut self.state
            }
        }

        impl<M, F, E> icy_engine_gui::ui::Dialog<M> for #wrapper_name<M, F, E>
        where
            M: Clone + Send + 'static,
            F: Fn(#internal_message) -> M + Clone + Send + 'static,
            E: Fn(&M) -> Option<&#internal_message> + Clone + Send + 'static,
        {
            fn view(&self) -> iced::Element<'_, M> {
                self.state.view(self.on_message.clone())
            }

            fn update(&mut self, message: &M) -> Option<icy_engine_gui::ui::DialogAction<M>> {
                let dialog_msg = (self.extract_message)(message)?;
                Some(self.handle_message(dialog_msg.clone()))
            }

            #request_cancel_impl

            #request_confirm_impl

            fn handle_event(&mut self, _event: &iced::Event) -> Option<icy_engine_gui::ui::DialogAction<M>> {
                None
            }

            fn close_on_blur(&self) -> bool {
                #close_on_blur
            }

            #style_impl
        }
    })
}

/// Attribute macro to generate dialog wrapper boilerplate.
///
/// Apply this to the dialog **State** struct to automatically generate
/// a corresponding **Wrapper** struct with all the boilerplate.
///
/// # Naming Conventions
///
/// Names are derived automatically from the state struct name:
/// - Wrapper: `FooState` → `FooWrapper`, `FooDialogState` → `FooDialogWrapper`
/// - Message: `FooState` → `FooMessage`, `FooDialogState` → `FooDialogMessage`
///
/// # Configuration Options (all optional)
///
/// - `result_type`: Type returned by `StateResult::Success(T)` (default: `()`)
/// - `internal_message`: Override the message enum name (default: derived from state name)
/// - `close_on_blur`: Whether clicking outside closes the dialog (default: `false`)
/// - `style`: Dialog style - `Modal` (default) or `Fullscreen`
///
/// # Generated Callbacks
///
/// - `on_confirm(T)`: Called on success (T is result_type, or no argument if `()`)
/// - `on_cancel()`: Called when cancelled
///
/// # Required State Methods
///
/// - `handle_message(&mut self, msg) -> StateResult<T>`: Handle messages, return result
/// - `view(&self, on_message) -> Element`: Render the dialog UI
///
/// # Examples
///
/// ```ignore
/// // Minimal - all names derived, no result value
/// #[dialog_wrapper]
/// pub struct ConfirmDialogState { ... }
/// // -> ConfirmDialogWrapper, ConfirmDialogMessage, StateResult<()>
///
/// // With result type
/// #[dialog_wrapper(result_type = PathBuf)]
/// pub struct ExportDialogState { ... }
/// // -> ExportDialogWrapper, ExportDialogMessage, StateResult<PathBuf>
///
/// // Fullscreen dialog (no modal overlay)
/// #[dialog_wrapper(style = Fullscreen)]
/// pub struct AboutDialogState { ... }
///
/// // Usage:
/// export_dialog(...)
///     .on_confirm(|path| Message::ExportSuccess(path))
///     .on_cancel(|| Message::DialogClosed)
/// ```
#[proc_macro_attribute]
pub fn dialog_wrapper(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(attr as DialogWrapperConfig);
    let state_struct = parse_macro_input!(item as ItemStruct);

    match generate_wrapper(&config, &state_struct) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
