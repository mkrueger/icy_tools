//! Configurable attribution for quoted messages.

use std::fmt;

use i18n_embed_fl::fl;

use crate::LANGUAGE_LOADER;

/// Empty selects the localized built-in attribution at the call site.
pub const DEFAULT_QUOTE_HEADER: &str = "";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuoteHeaderError {
    UnsupportedPlaceholder(String),
    UnclosedPlaceholder,
    UnexpectedClosingBrace,
}

impl fmt::Display for QuoteHeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedPlaceholder(placeholder) => {
                fl!(LANGUAGE_LOADER, "writing-quote-header-unsupported", placeholder = placeholder.as_str())
            }
            Self::UnclosedPlaceholder => fl!(LANGUAGE_LOADER, "writing-quote-header-unclosed"),
            Self::UnexpectedClosingBrace => fl!(LANGUAGE_LOADER, "writing-quote-header-unexpected-close"),
        };
        formatter.write_str(&message)
    }
}

impl std::error::Error for QuoteHeaderError {}

/// Empty attribution is allowed; braces must name one of the supported placeholders.
pub fn validate_quote_header(template: &str) -> Result<(), QuoteHeaderError> {
    let mut remaining = template;
    while let Some(index) = remaining.find(['{', '}']) {
        if remaining.as_bytes()[index] == b'}' {
            return Err(QuoteHeaderError::UnexpectedClosingBrace);
        }
        remaining = &remaining[index + 1..];
        let end = remaining.find('}').ok_or(QuoteHeaderError::UnclosedPlaceholder)?;
        let placeholder = &remaining[..end];
        if !matches!(placeholder, "author" | "subject" | "date") {
            return Err(QuoteHeaderError::UnsupportedPlaceholder(format!("{{{placeholder}}}")));
        }
        remaining = &remaining[end + 1..];
    }
    Ok(())
}

/// Substitute original message metadata literally, without interpreting braces in its values.
pub fn format_quote_attribution(template: &str, author: &str, subject: &str, date: &str) -> Result<String, QuoteHeaderError> {
    validate_quote_header(template)?;
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 1..];
        let end = remaining.find('}').expect("validated placeholder");
        result.push_str(match &remaining[..end] {
            "author" => author,
            "subject" => subject,
            "date" => date,
            _ => unreachable!("validated placeholder"),
        });
        remaining = &remaining[end + 1..];
    }
    result.push_str(remaining);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_formats_supported_placeholders_and_literal_metadata() {
        assert_eq!(
            format_quote_attribution("{author} wrote:", "Alice", "Topic", "2026-10-04").unwrap(),
            "Alice wrote:"
        );
        assert_eq!(format_quote_attribution(DEFAULT_QUOTE_HEADER, "Alice", "Topic", "2026-10-04").unwrap(), "");
        assert_eq!(
            format_quote_attribution("{date}: {author} on {subject} ({author})", "Jörg {subject}", "Résumé", "04.10.2026").unwrap(),
            "04.10.2026: Jörg {subject} on Résumé (Jörg {subject})"
        );
        assert_eq!(format_quote_attribution("", "Alice", "Topic", "date").unwrap(), "");
        assert_eq!(format_quote_attribution("Quoted message:", "", "", "").unwrap(), "Quoted message:");
    }

    #[test]
    fn attribution_rejects_unsupported_and_malformed_placeholders() {
        for template in ["{name}", "{}", "{{author}}", "{Author}"] {
            assert!(matches!(validate_quote_header(template), Err(QuoteHeaderError::UnsupportedPlaceholder(_))));
            assert!(format_quote_attribution(template, "Alice", "", "").is_err());
        }
        for template in ["{author", "{", "{author} {date"] {
            assert_eq!(validate_quote_header(template), Err(QuoteHeaderError::UnclosedPlaceholder));
        }
        for template in ["author}", "{author}}"] {
            assert_eq!(validate_quote_header(template), Err(QuoteHeaderError::UnexpectedClosingBrace));
        }
    }
}
