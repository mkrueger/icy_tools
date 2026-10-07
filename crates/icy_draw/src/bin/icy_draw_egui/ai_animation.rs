//! Tools for the Lua animation editor. They edit a draft of the script; the user reviews the
//! diff and applies it. Scripts are only syntax-checked here, never run.

use serde_json::{json, Value};

const MAX_SOURCE_BYTES: usize = 512 * 1024;
const MAX_READ_LINES: usize = 1500;
const API_DOC: &str = include_str!("../../../doc/ANIMATION.md");

#[derive(Clone)]
pub struct AnimationDraft {
    /// The script when drawing started; the diff and stale checks compare against it.
    pub original: String,
    pub source: String,
    pub file_name: Option<String>,
    pub frame_count: usize,
    /// The editor's last run error, from before the draft was edited.
    pub error: Option<String>,
}

/// The changed part of a script: lines between the common start and end, as kept (' '),
/// removed ('-') or added ('+').
pub struct Diff {
    /// 1-based line in the original where the change starts.
    pub line: usize,
    pub lines: Vec<(char, String)>,
    pub removed: usize,
    pub added: usize,
}

/// Line-level diff via longest common subsequence; very large changes fall back to remove-then-add.
fn diff_lines(before: &[&str], after: &[&str]) -> Vec<(char, String)> {
    const MAX_CELLS: usize = 1_000_000;
    let (n, m) = (before.len(), after.len());
    if n.saturating_mul(m) > MAX_CELLS {
        let removed = before.iter().map(|line| ('-', (*line).to_owned()));
        return removed.chain(after.iter().map(|line| ('+', (*line).to_owned()))).collect();
    }
    // common[i][j]: LCS length of before[i..] and after[j..].
    let mut common = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i * (m + 1) + j] = if before[i] == after[j] {
                common[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                common[(i + 1) * (m + 1) + j].max(common[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j, mut lines) = (0, 0, Vec::new());
    while i < n || j < m {
        if i < n && j < m && before[i] == after[j] {
            lines.push((' ', before[i].to_owned()));
            i += 1;
            j += 1;
        } else if j < m && (i == n || common[i * (m + 1) + j + 1] >= common[(i + 1) * (m + 1) + j]) {
            lines.push(('+', after[j].to_owned()));
            j += 1;
        } else {
            lines.push(('-', before[i].to_owned()));
            i += 1;
        }
    }
    lines
}

impl AnimationDraft {
    pub fn new(source: &str, file_name: Option<String>, frame_count: usize, error: Option<String>) -> Self {
        Self {
            original: source.to_owned(),
            source: source.to_owned(),
            file_name,
            frame_count,
            error,
        }
    }

    pub fn changed(&self) -> bool {
        self.source != self.original
    }

    pub fn diff(&self) -> Diff {
        let before: Vec<_> = self.original.split('\n').collect();
        let after: Vec<_> = self.source.split('\n').collect();
        let prefix = before.iter().zip(&after).take_while(|(a, b)| a == b).count();
        let suffix = before[prefix..]
            .iter()
            .rev()
            .zip(after[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let lines = diff_lines(&before[prefix..before.len() - suffix], &after[prefix..after.len() - suffix]);
        Diff {
            line: prefix + 1,
            removed: lines.iter().filter(|(kind, _)| *kind == '-').count(),
            added: lines.iter().filter(|(kind, _)| *kind == '+').count(),
            lines,
        }
    }

    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        match tool {
            "icy_animation_info" => Ok(self.info().to_string()),
            "icy_animation_api" => Ok(API_DOC.to_owned()),
            "icy_read_source" => self.read(arguments),
            "icy_replace_lines" => self.replace_lines(arguments),
            "icy_write_source" => {
                let text = arguments.get("text").and_then(Value::as_str).ok_or("text is required")?;
                self.set_source(text.to_owned())
            }
            "icy_check_lua" => Ok(match check_syntax(&self.source) {
                Ok(()) => "The script compiles. It was not run; the user sees the result after applying.".into(),
                Err(error) => format!("Syntax error: {error}"),
            }),
            _ => Err(format!("Unknown tool {tool}")),
        }
    }

    fn lines(&self) -> Vec<&str> {
        self.source.split('\n').collect()
    }

    fn info(&self) -> Value {
        json!({
            "editor": "Lua animation",
            "file": self.file_name,
            "lines": self.lines().len(),
            "bytes": self.source.len(),
            "frames_from_last_run": self.frame_count,
            "error_from_last_run": self.error,
            "notes": "The script builds ANSI frames with the Lua API from icy_animation_api. \
                      Lines are 1-based. Your edits change a draft that is not run; use icy_check_lua to check syntax.",
        })
    }

    fn read(&self, arguments: &Value) -> Result<String, String> {
        let lines = self.lines();
        let start = arguments.get("start_line").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
        let end = arguments
            .get("end_line")
            .and_then(Value::as_u64)
            .map_or(lines.len(), |end| end as usize)
            .min(lines.len());
        if start > end {
            return Err(format!("No lines in {start}..{end}; the script has {} lines", lines.len()));
        }
        if end - start + 1 > MAX_READ_LINES {
            return Err(format!("Read at most {MAX_READ_LINES} lines at a time"));
        }
        let width = end.to_string().len();
        let mut text = format!("Lines {start}-{end} of {}:\n", lines.len());
        for (number, line) in lines[start - 1..end].iter().enumerate() {
            text.push_str(&format!("{:>width$}| {line}\n", start + number));
        }
        Ok(text)
    }

    fn replace_lines(&mut self, arguments: &Value) -> Result<String, String> {
        let start = arguments.get("start_line").and_then(Value::as_u64).ok_or("start_line is required")? as usize;
        let end = arguments.get("end_line").and_then(Value::as_u64).ok_or("end_line is required")? as usize;
        let text = arguments.get("text").and_then(Value::as_str).ok_or("text is required")?;
        let mut lines: Vec<String> = self.lines().into_iter().map(str::to_owned).collect();
        // end_line = start_line - 1 inserts before start_line; start_line = lines + 1 appends.
        if start == 0 || start > lines.len() + 1 || end + 1 < start || end > lines.len() {
            return Err(format!(
                "Invalid range {start}..{end} for a script with {} lines. Use end_line = start_line - 1 to insert.",
                lines.len()
            ));
        }
        let replacement: Vec<String> = if text.is_empty() {
            Vec::new()
        } else {
            text.strip_suffix('\n').unwrap_or(text).split('\n').map(str::to_owned).collect()
        };
        let (removed, added) = (end + 1 - start, replacement.len());
        lines.splice(start - 1..end, replacement);
        self.set_source(lines.join("\n"))?;
        Ok(format!(
            "Replaced {removed} lines with {added} lines in the draft. The user reviews the change before applying it."
        ))
    }

    fn set_source(&mut self, source: String) -> Result<String, String> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err("The script would exceed 512 KiB".into());
        }
        self.source = source;
        Ok(match check_syntax(&self.source) {
            Ok(()) => "Updated the draft; it compiles. The user reviews the change before applying it.".into(),
            Err(error) => format!("Updated the draft, but it has a syntax error: {error}"),
        })
    }
}

/// Compiles the script without running it.
fn check_syntax(source: &str) -> Result<(), String> {
    let lua = mlua::Lua::new();
    lua.load(source)
        .set_name("animation")
        .into_function()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let line = |description: &str| json!({ "type": "integer", "minimum": 1, "description": description });
    vec![
        (
            "icy_animation_info",
            "[Animation editor] Describe the open Lua animation script: file, line count, frames and error of the last run.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_animation_api",
            "[Animation editor] Return the Lua scripting API documentation for animations.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_read_source",
            "[Animation editor] Read lines of the script with line numbers (at most 1500 lines).",
            json!({
                "type": "object",
                "properties": {
                    "start_line": line("First line, default 1"),
                    "end_line": line("Last line, default the end"),
                },
            }),
        ),
        (
            "icy_replace_lines",
            "[Animation editor] Replace lines start_line..end_line (inclusive) with text. \
             Use end_line = start_line - 1 to insert before start_line and empty text to delete.",
            json!({
                "type": "object",
                "properties": {
                    "start_line": line("First replaced line"),
                    "end_line": { "type": "integer", "minimum": 0, "description": "Last replaced line" },
                    "text": { "type": "string", "description": "New lines, separated by \\n" },
                },
                "required": ["start_line", "end_line", "text"],
            }),
        ),
        (
            "icy_write_source",
            "[Animation editor] Replace the whole script.",
            json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        ),
        (
            "icy_check_lua",
            "[Animation editor] Check the draft script for Lua syntax errors without running it.",
            json!({ "type": "object", "properties": {} }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> AnimationDraft {
        AnimationDraft::new("local buf = new_buffer(80, 25)\nnext_frame(buf)\n", Some("demo.icyanim".into()), 1, None)
    }

    #[test]
    fn reads_numbered_lines_and_describes_the_script() {
        let mut draft = draft();
        let text = draft.call("icy_read_source", &json!({ "start_line": 2 })).unwrap();
        assert!(text.contains("2| next_frame(buf)"), "{text}");
        let info: Value = serde_json::from_str(&draft.call("icy_animation_info", &json!({})).unwrap()).unwrap();
        assert_eq!(info["lines"], 3);
        assert_eq!(info["file"], "demo.icyanim");
        assert!(draft.call("icy_animation_api", &json!({})).unwrap().contains("next_frame"));
        assert!(draft.call("icy_read_source", &json!({ "start_line": 9 })).is_err());
    }

    #[test]
    fn replaces_inserts_and_deletes_lines() {
        let mut draft = draft();
        draft
            .call("icy_replace_lines", &json!({ "start_line": 2, "end_line": 1, "text": "buf.fg = 14\n" }))
            .unwrap();
        assert_eq!(draft.source, "local buf = new_buffer(80, 25)\nbuf.fg = 14\nnext_frame(buf)\n");
        draft.call("icy_replace_lines", &json!({ "start_line": 2, "end_line": 2, "text": "" })).unwrap();
        assert_eq!(draft.source, draft.original);
        assert!(!draft.changed());
        for range in [json!([0, 0]), json!([3, 1]), json!([5, 5]), json!([2, 9])] {
            let arguments = json!({ "start_line": range[0], "end_line": range[1], "text": "x" });
            assert!(draft.call("icy_replace_lines", &arguments).is_err(), "{range}");
        }
    }

    #[test]
    fn syntax_is_checked_without_running_the_script() {
        let mut draft = draft();
        let result = draft.call("icy_write_source", &json!({ "text": "error('must not run')" })).unwrap();
        assert!(result.contains("it compiles"), "{result}");
        let result = draft.call("icy_write_source", &json!({ "text": "for i = 1 do" })).unwrap();
        assert!(result.contains("syntax error"), "{result}");
        assert!(draft.call("icy_check_lua", &json!({})).unwrap().starts_with("Syntax error"));
        assert!(draft.call("icy_write_source", &json!({ "text": "x".repeat(MAX_SOURCE_BYTES + 1) })).is_err());
    }

    #[test]
    fn diff_shows_only_the_changed_lines() {
        let mut draft = AnimationDraft::new("a\nb\nc\nd", None, 0, None);
        draft
            .call("icy_replace_lines", &json!({ "start_line": 2, "end_line": 3, "text": "B\nC\nC2" }))
            .unwrap();
        let diff = draft.diff();
        assert_eq!(diff.line, 2);
        assert_eq!((diff.removed, diff.added), (2, 3));

        let mut draft = AnimationDraft::new("head\nfor i = 1, 8 do\n  clear()\n  next_frame()\nend", None, 0, None);
        let edited = "head\nset_delay(50)\nfor i = 1, 8 do\n  clear()\n  draw(i)\n  next_frame()\nend";
        draft.call("icy_write_source", &json!({ "text": edited })).unwrap();
        let diff = draft.diff();
        let kinds: String = diff.lines.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(kinds, "+  +", "unchanged lines stay as context: {:?}", diff.lines);
        assert_eq!((diff.line, diff.removed, diff.added), (2, 0, 2));
    }
}
