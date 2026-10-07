//! The editor-specific draft the assistant's tools work on. Every editor contributes its own
//! tools; the Copilot session registers all of them and calls reach only the open editor's draft.

use serde_json::Value;

use super::{
    animation_tools::AnimationDraft, canvas::Draft, font_tools::FontDraft, igs_tools::IgsDraft, rip_tools::RipDraft, skypix_tools::SkypixDraft,
    tdf_tools::TdfDraft,
};

#[derive(Clone)]
pub enum Workspace {
    Canvas(Box<Draft>),
    Animation(AnimationDraft),
    Font(FontDraft),
    Tdf(TdfDraft),
    Rip(RipDraft),
    Igs(IgsDraft),
    Skypix(SkypixDraft),
}

impl Workspace {
    pub fn editor(&self) -> &str {
        match self {
            Self::Canvas(draft) => &draft.editor,
            Self::Animation(_) => "Lua animation",
            Self::Font(_) => "bitmap font",
            Self::Tdf(_) => "TheDraw text-art font (TDF)",
            Self::Rip(_) => "RIP",
            Self::Igs(_) => "IGS",
            Self::Skypix(_) => "SkyPix",
        }
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        let specs = match self {
            Self::Canvas(_) => super::canvas::tool_specs(),
            Self::Animation(_) => super::animation_tools::tool_specs(),
            Self::Font(_) => super::font_tools::tool_specs(),
            Self::Tdf(_) => super::tdf_tools::tool_specs(),
            Self::Rip(_) => super::rip_tools::tool_specs(),
            Self::Igs(_) => super::igs_tools::tool_specs(),
            Self::Skypix(_) => super::skypix_tools::tool_specs(),
        };
        specs
            .into_iter()
            .map(|(name, _, _)| name)
            .filter(|name| {
                !matches!(self, Self::Canvas(draft) if draft.image_authoring) || !matches!(*name, "icy_convert_reference_image" | "icy_refine_reference_image")
            })
            .collect()
    }

    pub fn changed(&self) -> bool {
        match self {
            Self::Canvas(draft) => !draft.changes().is_empty(),
            Self::Animation(draft) => draft.changed(),
            Self::Font(draft) => !draft.changed_codes().is_empty(),
            Self::Tdf(draft) => !draft.changed_codes().is_empty(),
            Self::Rip(draft) => draft.changed(),
            Self::Igs(draft) => draft.changed(),
            Self::Skypix(draft) => draft.changed(),
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
            Self::Tdf(draft) => draft.call(tool, arguments),
            Self::Rip(draft) => draft.call(tool, arguments),
            Self::Igs(draft) => draft.call(tool, arguments),
            Self::Skypix(draft) => draft.call(tool, arguments),
        }
    }
}

/// Every editor's tools, registered once per Copilot session.
pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let mut specs = super::canvas::tool_specs();
    specs.extend(super::animation_tools::tool_specs());
    specs.extend(super::font_tools::tool_specs());
    specs.extend(super::tdf_tools::tool_specs());
    specs.extend(super::rip_tools::tool_specs());
    specs.extend(super::igs_tools::tool_specs());
    specs.extend(super::skypix_tools::tool_specs());
    specs
}

/// Prepended to the prompt so the model knows which tools apply.
pub fn editor_hint(workspace: Option<&Workspace>) -> String {
    match workspace {
        Some(Workspace::Tdf(_)) => format!(
            "[icy_draw: the open editor is TheDraw text-art font (TDF). Read icy_tdf_info and \
             icy_read_tdf_glyphs, then use icy_write_tdf_glyphs to generate a full font in batches. \
             NOT bitmap pixel tools or ANSI canvas tools. Use icy_preview_tdf to inspect pages. \
             Check missing_codes before claiming all 94 glyphs are complete. User Apply is required.]\n{}",
            super::tdf_tools::GUIDANCE
        ),
        Some(workspace @ Workspace::Canvas(draft)) => {
            if draft.image_authoring {
                return format!(
                    "[icy_draw: AI image authoring on a blank native {} canvas. Constraints: {}. \
                     Author the picture as recognizable character art using native glyphs, large coherent \
                     shapes and deliberate facial features. Local pixel conversion tools are unavailable. \
                     Read icy_canvas_info and icy_read_canvas_glyphs, draw with cell batches and rectangles, \
                     then inspect icy_preview_canvas and correct the composition. The user sees this draft \
                     in the import dialog before accepting it; do not claim it has already been applied.]",
                    workspace.editor(),
                    draft.info()
                );
            }
            let info = draft.info();
            let target = serde_json::json!({
                "width": info["width"],
                "height": info["height"],
                "fonts": info["fonts"],
                "font_cell_size": info["font_cell_size"],
                "display_cell_size": info["display_cell_size"],
                "letter_spacing": info["letter_spacing"],
                "aspect_ratio_correction": info["aspect_ratio_correction"],
                "character_profile": info["character_profile"],
                "tool_capabilities": info["tool_capabilities"],
                "encoding": info["encoding"],
                "ice_colors": info["ice_colors"],
                "reference_image": info["reference_image"],
            });
            let guidance = if super::canvas::is_retro(&draft.buffer) {
                "Use native glyph_codes/font_pages, not CP437 assumptions. char_code writes a screen glyph, \
                 not a terminal control. Preserve font pages and shared colors; PETSCII text follows the \
                 current bank, ATASCII inverse uses high-bit glyphs, VT52 uses Atari ST characters. \
                 For local picture conversion use preset='faithful', mode='full'."
            } else {
                super::canvas::ANSI_GUIDANCE
            };
            format!(
                "[icy_draw: the open editor is \"{}\"; its tools are {}. Target constraints: {target}. \
                 These constraints apply even when optional knowledge is disabled. Start with icy_canvas_info \
                 and read the target region, checking the current layer is visible and unlocked. \
                 For image conversion, read icy_read_canvas_glyphs for the active font pages and prefer \
                 icy_convert_reference_image for a local first pass using the attached picture. \
                 For CP437 styled conversions, inspect the PNG and metrics, then use icy_refine_reference_image \
                 with a specific visual critique and tuning changes; it retains only measured improvements. \
                 Finish parameter trials before any icy_set_cells touch-ups. \
                 A textual description alone does not create art. \
                 {guidance} \
                 Use icy_preview_canvas to inspect the actual rendered draft and correct defects; \
                 no more than three previews and three conversion/refinement trials per turn. Check cell data with icy_read_region; \
                 only claim a draft was created if successful write calls actually changed it. \
                 If tools reject the edits or the draft remains unchanged, explain that explicitly.]",
                workspace.editor(),
                workspace.tool_names().join(", ")
            )
        }
        Some(workspace @ Workspace::Skypix(_)) => format!(
            "[icy_draw: the open editor is SkyPix; tools are {}. Read icy_skypix_info, icy_skypix_api and item \
             pages before editing ordered pixel graphics/state and mixed CP437/ANSI text. Use \
             icy_replace_skypix_items with actual ESC byte source or source_hex, NOT ANSI cell tools, \
             RIP or IGS syntax. Preview with icy_preview_skypix. Preserve read-only runtime/external items; \
             static previews omit audio/delays/transfers/controller/gadgets and unsupported fragments. \
             Explain omissions, never claim playback equivalence. User Apply/Discard is required.]",
            workspace.tool_names().join(", ")
        ),
        Some(workspace @ Workspace::Igs(_)) => format!(
            "[icy_draw: the open editor is IGS; its tools are {}. Read icy_igs_info, icy_igs_api and item pages \
             before editing ordered graphics/state and mixed VT52 text. Use icy_replace_igs_items in coherent \
             batches and icy_preview_igs to inspect static PNG feedback. NOT ANSI cells or RIP base-36. \
             Preserve read-only runtime/unsafe items. Previews omit loops, timing/audio, input and unsafe \
             fragments; explain omissions, do not claim playback equivalence. Apply/Discard is required.]",
            workspace.tool_names().join(", ")
        ),
        Some(workspace @ Workspace::Rip(_)) => format!(
            "[icy_draw: the open editor is \"RIP\"; its tools are {}. Read icy_rip_info and icy_rip_api, \
             then existing command pages. Edit ordered pixel-graphics commands in coherent batches, \
             not ANSI cells. Preserve read-only prefixes; wire fields are fixed-width base-36.]",
            workspace.tool_names().join(", ")
        ),
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
        let mut rip = Workspace::Rip(RipDraft::new(&icy_draw::rip_document::RipDocument::new()));
        assert_eq!(rip.editor(), "RIP");
        assert!(editor_hint(Some(&rip)).contains("base-36"));
        assert!(names.contains(&"icy_replace_rip_commands"));
        assert!(rip.call("icy_rip_info", &json!({})).is_ok());
        assert!(rip.call("icy_set_cells", &json!({})).unwrap_err().contains("RIP"));
        assert!(workspace.call("icy_replace_rip_commands", &json!({})).unwrap_err().contains("Lua animation"));
        let retro = Workspace::Canvas(Box::new(Draft::new(
            "ATASCII",
            icy_draw::screen_profile::atascii_buffer(icy_draw::screen_profile::AtasciiMode::Antic),
            0,
            None,
        )));
        assert!(editor_hint(Some(&retro)).contains("char_code"));
        assert!(editor_hint(Some(&retro)).contains("not CP437"));
        let hint = editor_hint(Some(&retro));
        assert!(hint.contains("icy_read_canvas_glyphs"));
        assert!(hint.contains("\"width\":40") && hint.contains("\"height\":24"));
        assert!(hint.contains("\"per_character_colors\":false"));
        assert!(hint.contains("glyph_width") && hint.contains("display_cell_size"));
        assert!(hint.contains("omit fg/bg") && hint.contains("\"change_resolution\":false"));
        assert!(hint.contains("optional knowledge is disabled"));
        assert!(hint.contains("A textual description alone does not create art"));
        let ansi = Workspace::Canvas(Box::new(Draft::new("ANSI/ASCII", icy_engine::TextBuffer::new((80, 25)), 0, None)));
        let hint = editor_hint(Some(&ansi));
        assert!(hint.contains("Classic DOS ANSI has 16 foreground"));
        assert!(hint.contains("icy_preview_canvas") && hint.contains("icy_convert_reference_image"));
        assert!(hint.contains("\"encoding\":\"CP437\""));
        let mut igs = Workspace::Igs(IgsDraft::new(&icy_draw::igs_document::IgsDocument::default()));
        assert!(editor_hint(Some(&igs)).contains("icy_replace_igs_items"));
        assert!(igs.call("icy_igs_info", &json!({})).is_ok());
        assert!(igs.call("icy_replace_rip_commands", &json!({})).unwrap_err().contains("IGS"));
        assert!(names.contains(&"icy_preview_igs"));
    }
}
