//! The editor-specific draft the assistant's tools work on. Every editor contributes its own
//! tools; the Copilot session registers all of them and calls reach only the open editor's draft.

use serde_json::Value;

use super::{animation_tools::AnimationDraft, canvas::Draft, font_tools::FontDraft};

#[derive(Clone)]
pub enum Workspace {
    Canvas(Box<Draft>),
    Animation(AnimationDraft),
    Font(FontDraft),
}

impl Workspace {
    pub fn editor(&self) -> &str {
        match self {
            Self::Canvas(draft) => &draft.editor,
            Self::Animation(_) => "Lua animation",
            Self::Font(_) => "bitmap font",
        }
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        let specs = match self {
            Self::Canvas(_) => super::canvas::tool_specs(),
            Self::Animation(_) => super::animation_tools::tool_specs(),
            Self::Font(_) => super::font_tools::tool_specs(),
        };
        specs.into_iter().map(|(name, _, _)| name).collect()
    }

    pub fn changed(&self) -> bool {
        match self {
            Self::Canvas(draft) => !draft.changes().is_empty(),
            Self::Animation(draft) => draft.changed(),
            Self::Font(draft) => !draft.changed_codes().is_empty(),
        }
    }

    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        if !self.tool_names().contains(&tool) {
            return Err(format!(
                "{tool} does not work in the open {} editor. Its tools are: {}",
                self.editor(),
                self.tool_names().join(", ")
            ));
        }
        match self {
            Self::Canvas(draft) => draft.call(tool, arguments),
            Self::Animation(draft) => draft.call(tool, arguments),
            Self::Font(draft) => draft.call(tool, arguments),
        }
    }
}

/// Every editor's tools, registered once per Copilot session.
pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let mut specs = super::canvas::tool_specs();
    specs.extend(super::animation_tools::tool_specs());
    specs.extend(super::font_tools::tool_specs());
    specs
}

/// Prepended to the prompt so the model knows which tools apply.
pub fn editor_hint(workspace: Option<&Workspace>) -> String {
    match workspace {
        Some(workspace @ Workspace::Font(_)) => format!(
            "[icy_draw: the open editor is \"bitmap font\"; its tools are {}. \
             Use icy_transform_glyphs for mechanical font-wide changes. Otherwise prefer \
             format='hex' and icy_write_glyphs batches, not one write per glyph.]",
            workspace.tool_names().join(", ")
        ),
        Some(workspace) => format!(
            "[icy_draw: the open editor is \"{}\"; its tools are {}.]",
            workspace.editor(),
            workspace.tool_names().join(", ")
        ),
        None => "[icy_draw: the open editor has no assistant tools yet; answer with advice in text.]".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tools_of_other_editors_name_the_open_editor() {
        let mut workspace = Workspace::Animation(AnimationDraft::new("next_frame(new_buffer(1, 1))", None, 0, None));
        let error = workspace.call("icy_draw_text", &json!({ "x": 0, "y": 0, "text": "x" })).unwrap_err();
        assert!(error.contains("Lua animation") && error.contains("icy_read_source"), "{error}");
        assert!(workspace.call("icy_read_source", &json!({})).is_ok());
        for tool in ["icy_write_glyphs", "icy_transform_glyphs"] {
            assert!(workspace.call(tool, &json!({})).unwrap_err().contains("Lua animation"));
        }
        assert!(editor_hint(Some(&workspace)).contains("icy_replace_lines"));
        let names: Vec<_> = tool_specs().into_iter().map(|(name, _, _)| name).collect();
        let mut unique = names.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "tool names must be unique");
        assert!(names.contains(&"icy_write_glyphs") && names.contains(&"icy_transform_glyphs"));
        let font = Workspace::Font(FontDraft::new("Test", 8, 16, 0, vec![vec![vec![false; 8]; 16]]));
        let hint = editor_hint(Some(&font));
        assert!(hint.contains("icy_transform_glyphs") && hint.contains("icy_write_glyphs") && hint.contains("format='hex'"));
    }
}
