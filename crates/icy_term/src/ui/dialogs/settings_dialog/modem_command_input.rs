use iced::{
    Alignment, Element, Length,
    widget::{Space, row, text_input},
};
use icy_engine_gui::ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, error_tooltip, left_label_small};
use icy_net::modem::ModemCommand;

/// Creates a row with a label, text input for ModemCommand editing, and error indicator.
///
/// The input displays the ModemCommand as a string and validates input in real-time.
/// If the input is invalid, an error icon with tooltip is shown.
///
/// # Arguments
/// * `label` - The label text for the input field
/// * `placeholder` - Placeholder text shown when empty (e.g., "ATZ^M")
/// * `current_value` - The current ModemCommand value (used for display)
/// * `on_change` - Callback function that receives the new string value
///
/// # Note
/// This input only updates the stored value when the input is a valid ModemCommand.
/// The error indicator shows parsing errors for the currently displayed value.
///
/// # Returns
/// An Element containing the complete row with label, input, and optional error indicator
pub fn modem_command_input<'a, Message: Clone + 'static>(
    label: String,
    placeholder: &'a str,
    current_value: &ModemCommand,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let display_value = current_value.to_string();

    // Validate the displayed value - should always be valid for stored ModemCommands
    let validation_result = ModemCommand::validate(&display_value);
    let is_valid = validation_result.is_valid();

    let input = text_input(placeholder, &display_value)
        .on_input(on_change)
        .width(Length::Fill)
        .size(TEXT_SIZE_NORMAL);

    let error_element: Element<'a, Message> = if !is_valid {
        let error_text = format_validation_error(&validation_result);
        row![Space::new().width(4), error_tooltip(error_text),].into()
    } else {
        Space::new().width(0).into()
    };

    row![left_label_small(label), input, error_element,]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
}

/// Creates a row with a label, text input for ModemCommand editing with an explicit display string.
///
/// This version allows showing a different value than what's stored, useful for:
/// - Showing user input that hasn't been validated yet
/// - Displaying editing state while typing
///
/// # Arguments
/// * `label` - The label text for the input field
/// * `placeholder` - Placeholder text shown when empty
/// * `display_value` - The string value to display in the input (may be invalid)
/// * `on_change` - Callback function that receives the new string value
///
/// # Returns
/// An Element containing the complete row with label, input, and optional error indicator
pub fn modem_command_input_with_string<'a, Message: Clone + 'static>(
    label: String,
    placeholder: &'a str,
    display_value: &str,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    // Validate the displayed string value
    let validation_result = ModemCommand::validate(display_value);
    let is_valid = validation_result.is_valid();

    let input = text_input(placeholder, display_value)
        .on_input(on_change)
        .width(Length::Fill)
        .size(TEXT_SIZE_NORMAL);

    let error_element: Element<'a, Message> = if !is_valid {
        let error_text = format_validation_error(&validation_result);
        row![Space::new().width(4), error_tooltip(error_text),].into()
    } else {
        Space::new().width(0).into()
    };

    row![left_label_small(label), input, error_element,]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
}

/// Creates a simplified modem command input without label (for inline use)
pub fn modem_command_input_inline<'a, Message: Clone + 'static>(
    placeholder: &'a str,
    current_value: &ModemCommand,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let display_value = current_value.to_string();
    let validation_result = ModemCommand::validate(&display_value);
    let is_valid = validation_result.is_valid();

    let input = text_input(placeholder, &display_value)
        .on_input(on_change)
        .width(Length::Fill)
        .size(TEXT_SIZE_NORMAL);

    if !is_valid {
        let error_text = format_validation_error(&validation_result);
        row![input, Space::new().width(4), error_tooltip(error_text),]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center)
            .into()
    } else {
        input.into()
    }
}

/// Formats the validation error as a human-readable string
fn format_validation_error(result: &icy_net::modem::ModemCommandValidationResult) -> String {
    use icy_net::modem::ModemCommandValidationResult;

    match result {
        ModemCommandValidationResult::Valid => String::new(),
        ModemCommandValidationResult::InvalidControlSequence { char, position } => {
            format!("Invalid control sequence '^{}' at position {}", char, position)
        }
        ModemCommandValidationResult::IncompleteControlSequence { position } => {
            format!("Incomplete control sequence at position {}", position)
        }
        ModemCommandValidationResult::InvalidCharacter { char, position } => {
            format!("Invalid character '{}' at position {}", char, position)
        }
        ModemCommandValidationResult::InvalidHexSequence { position } => {
            format!("Invalid hex sequence at position {}", position)
        }
    }
}

/// Validates a modem command string and returns an error message if invalid.
/// Returns `None` if the string is valid, `Some(error_message)` otherwise.
pub fn validate_modem_command(input: &str) -> Option<String> {
    let result = ModemCommand::validate(input);
    if result.is_valid() { None } else { Some(format_validation_error(&result)) }
}

/// Updates a ModemCommand from a string value, returning the updated command.
/// If parsing fails, returns the previous value unchanged.
pub fn update_modem_command(current: &ModemCommand, new_value: &str) -> ModemCommand {
    new_value.parse().unwrap_or_else(|_| current.clone())
}
