// Self-reference for proc macros to work within this crate
extern crate self as icy_engine_gui;

pub mod double_click;
pub use double_click::*;

pub mod focus;
pub use focus::{
    default_style, focus, list_focus_style, no_border_style, Catalog as FocusCatalog, Focus, OnEvent, Style as FocusStyle, StyleFn as FocusStyleFn,
};

pub mod terminal;
use icy_engine::Color;
pub use terminal::*;

pub mod commands;
pub use commands::{
    cmd, create_common_commands, load_commands_from_file, load_commands_from_str, CommandDef, CommandLoadError, CommandSet, Hotkey, KeyCode, Modifiers,
};

// Re-export proc macros
pub use icy_engine_gui_macros::dialog_wrapper;

pub mod scrollbar;
pub use scrollbar::*;

pub mod clipboard;
pub use clipboard::*;

pub mod viewport;
pub use viewport::*;

// Re-export mouse event types from icy_engine
pub use icy_engine::{KeyModifiers, MouseButton, MouseEvent, MouseEventType};

pub mod key_map;
pub mod settings;

pub mod blink;
pub use blink::*;

pub mod theme;
pub use theme::*;

pub mod render_unicode;
pub use render_unicode::*;

pub mod unicode_glyph_cache;
pub use unicode_glyph_cache::*;

pub mod calculations;
pub use calculations::*;

pub mod ui;
pub use ui::*;

pub mod util;

pub mod music;

//pub mod terminal_shader_widget;

use serde::{Deserialize, Serialize};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MonitorType {
    Color = 0,
    Grayscale = 1,
    Amber = 2,
    Green = 3,
    Apple2 = 4,
    Futuristic = 5,
    CustomMonochrome = 6,
}

impl MonitorType {
    pub fn color(&self) -> Color {
        match self {
            MonitorType::Color => Color::new(0, 0, 0),
            MonitorType::Grayscale => Color::new(0xFF, 0xFF, 0xFF),
            MonitorType::Amber => Color::new(0xFF, 0x81, 0x00),
            MonitorType::Green => Color::new(0x0C, 0xCC, 0x68),
            MonitorType::Apple2 => Color::new(0x00, 0xD5, 0x6D),
            MonitorType::Futuristic => Color::new(0x72, 0x9F, 0xCF),
            MonitorType::CustomMonochrome => Color::new(0, 0, 0),
        }
    }

    fn _is_monochrome(&self) -> bool {
        *self != MonitorType::Color
    }
}

impl Into<i32> for MonitorType {
    fn into(self) -> i32 {
        match self {
            MonitorType::Color => 0,
            MonitorType::Grayscale => 1,
            MonitorType::Amber => 2,
            MonitorType::Green => 3,
            MonitorType::Apple2 => 4,
            MonitorType::Futuristic => 5,
            MonitorType::CustomMonochrome => 6,
        }
    }
}

impl From<i32> for MonitorType {
    fn from(value: i32) -> Self {
        match value {
            0 => MonitorType::Color,
            1 => MonitorType::Grayscale,
            2 => MonitorType::Amber,
            3 => MonitorType::Green,
            4 => MonitorType::Apple2,
            5 => MonitorType::Futuristic,
            _ => MonitorType::CustomMonochrome,
        }
    }
}

/// Scaling mode for terminal/viewer content
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ScalingMode {
    /// Automatically scale to fit the available space
    /// With integer_scaling: uses largest integer factor that fits
    /// Without integer_scaling: uses exact fit factor
    #[default]
    Auto,

    /// Manual zoom level (1.0 = 100%, 2.0 = 200%, etc.)
    /// With integer_scaling: rounds to nearest integer
    Manual(f32),
}

impl ScalingMode {
    /// Minimum zoom level (50%)
    pub const MIN_ZOOM: f32 = 0.5;
    /// Maximum zoom level (400%)
    pub const MAX_ZOOM: f32 = 4.0;
    /// Zoom step for each zoom in/out action (25%)
    pub const ZOOM_STEP: f32 = 0.25;
    /// Zoom step for integer scaling
    pub const ZOOM_STEP_INT: f32 = 1.0;

    /// Clamp a zoom value to valid range
    pub fn clamp_zoom(zoom: f32) -> f32 {
        zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM)
    }

    /// Calculate the next zoom level when zooming in
    pub fn zoom_in(current: f32, use_integer: bool) -> f32 {
        let step = if use_integer { Self::ZOOM_STEP_INT } else { Self::ZOOM_STEP };
        let new_zoom = if use_integer { (current + step).floor() } else { current + step };
        Self::clamp_zoom(new_zoom)
    }

    /// Calculate the next zoom level when zooming out
    pub fn zoom_out(current: f32, use_integer: bool) -> f32 {
        let step = if use_integer { Self::ZOOM_STEP_INT } else { Self::ZOOM_STEP };
        let new_zoom = if use_integer { (current - step).ceil().max(1.0) } else { current - step };
        Self::clamp_zoom(new_zoom)
    }

    /// Get the effective zoom factor for given content and viewport sizes
    /// Returns the zoom factor to use for rendering
    ///
    /// IMPORTANT: Auto mode always uses the **smaller** of scale_x and scale_y
    /// (uniform scaling) so the content is never stretched/distorted.
    /// If the content is smaller than the viewport after scaling, it will be centered.
    pub fn compute_zoom(&self, content_width: f32, content_height: f32, viewport_width: f32, viewport_height: f32, use_integer_scaling: bool) -> f32 {
        match self {
            ScalingMode::Auto => {
                // Calculate uniform scale that fits content in viewport without distortion
                let scale_x = viewport_width / content_width.max(1.0);
                let scale_y = viewport_height / content_height.max(1.0);
                // Always use the smaller scale to prevent any axis from being stretched
                let fit_scale = scale_x.min(scale_y).max(0.1);

                if use_integer_scaling {
                    // Use largest integer that still fits
                    fit_scale.floor().max(1.0)
                } else {
                    fit_scale
                }
            }

            ScalingMode::Manual(zoom) => {
                if use_integer_scaling {
                    zoom.round().max(1.0)
                } else {
                    *zoom
                }
            }
        }
    }

    /// Check if in auto mode
    pub fn is_auto(&self) -> bool {
        matches!(self, ScalingMode::Auto)
    }

    /// Get manual zoom value, or 1.0 if auto
    pub fn get_manual_zoom(&self) -> f32 {
        match self {
            ScalingMode::Auto => 1.0,
            ScalingMode::Manual(z) => *z,
        }
    }

    /// Format zoom info for display in window title
    /// Returns "[AUTO]" for auto mode or "[N%]" for manual mode with clamped value
    pub fn format_zoom_string(&self) -> String {
        match self {
            ScalingMode::Auto => "[AUTO]".to_string(),
            ScalingMode::Manual(zoom) => {
                let clamped = Self::clamp_zoom(*zoom);
                format!("[{:.0}%]", clamped * 100.0)
            }
        }
    }

    /// Apply a zoom message and return the new scaling mode
    /// This is the central zoom handling logic for all applications
    pub fn apply_zoom(&self, msg: ZoomMessage, current_zoom: f32, use_integer_scaling: bool) -> ScalingMode {
        match msg {
            ZoomMessage::In => {
                let new_zoom = Self::zoom_in(current_zoom, use_integer_scaling);
                ScalingMode::Manual(new_zoom)
            }
            ZoomMessage::Out => {
                let new_zoom = Self::zoom_out(current_zoom, use_integer_scaling);
                ScalingMode::Manual(new_zoom)
            }
            ZoomMessage::Reset => ScalingMode::Manual(1.0),
            ZoomMessage::AutoFit => ScalingMode::Auto,
            ZoomMessage::Set(zoom) => ScalingMode::Manual(Self::clamp_zoom(zoom)),
            ZoomMessage::Wheel(delta) => {
                // Extract y-axis delta and determine zoom behavior
                let (y_delta, is_smooth) = match delta {
                    WheelDelta::Lines { y, .. } => {
                        // Discrete scroll wheel - use sign for step-based zoom
                        let sign = if y > 0.0 {
                            1.0
                        } else if y < 0.0 {
                            -1.0
                        } else {
                            0.0
                        };
                        (sign, false)
                    }
                    WheelDelta::Pixels { y, .. } => {
                        // Pixel-based scroll (macOS trackpad) - smooth zooming
                        (y / 200.0, true)
                    }
                };

                if y_delta == 0.0 {
                    return *self; // No change
                }

                let new_zoom = if is_smooth {
                    // Smooth scroll - apply delta directly
                    Self::clamp_zoom(current_zoom + y_delta)
                } else {
                    // Discrete scroll wheel - use step-based zoom
                    if y_delta > 0.0 {
                        Self::zoom_in(current_zoom, use_integer_scaling)
                    } else {
                        Self::zoom_out(current_zoom, use_integer_scaling)
                    }
                };
                ScalingMode::Manual(new_zoom)
            }
        }
    }
}

/// Unified zoom message for all icy_tools applications
/// Used by ScalingMode::apply_zoom() for consistent zoom handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMessage {
    /// Zoom in by one step (respects integer scaling if enabled)
    In,
    /// Zoom out by one step (respects integer scaling if enabled)
    Out,
    /// Reset zoom to 100% (1:1 pixel mapping)
    Reset,
    /// Auto-fit content to viewport
    AutoFit,
    /// Set specific zoom level (1.0 = 100%)
    Set(f32),
    /// Mouse wheel zoom (raw delta from Cmd/Ctrl+scroll)
    /// Positive delta = zoom in, negative = zoom out
    /// |delta| >= 1.0: discrete scroll wheel (use step-based zoom)
    /// |delta| < 1.0: smooth trackpad (apply delta directly)
    Wheel(WheelDelta),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorSettings {
    pub theme: String,

    pub monitor_type: MonitorType,
    pub custom_monitor_color: Color,

    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub saturation: f32,

    /// Colors for the transparency checkerboard pattern
    #[serde(default)]
    pub checkerboard_colors: CheckerboardColors,

    /// Use integer scaling (1x, 2x, 3x) for sharp bitmap fonts
    #[serde(alias = "use_pixel_perfect_scaling")]
    pub use_integer_scaling: bool,

    pub use_bilinear_filtering: bool,

    /// Scaling mode: Auto (fit-to-window) or Manual (user-defined zoom)
    #[serde(default)]
    pub scaling_mode: ScalingMode,

    pub use_bloom: bool,
    pub bloom_threshold: f32,
    pub bloom_radius: f32,
    pub glow_strength: f32,
    pub phosphor_persistence: f32, // decay speed (higher = longer afterglow)

    pub use_scanlines: bool,
    pub scanline_thickness: f32, // 0..1 relative thickness
    pub scanline_sharpness: f32, // exponent/style
    pub scanline_phase: f32,     // offset for anim/flicker

    pub use_curvature: bool,
    pub curvature_x: f32,
    pub curvature_y: f32,

    pub use_noise: bool,
    pub noise_level: f32,
    pub sync_wobble: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerSettings {
    pub reference_image_alpha: f32,

    pub raster_alpha: f32,
    pub raster_color: Color,

    pub guide_alpha: f32,
    pub guide_color: Color,
}

impl Default for MarkerSettings {
    fn default() -> Self {
        Self {
            reference_image_alpha: 0.2,
            raster_alpha: 0.2,
            raster_color: Color::new(0xBB, 0xBB, 0xBB),
            guide_alpha: 0.2,
            guide_color: Color::new(0xAB, 0xAB, 0xAB),
        }
    }
}

// ============================================================================
// Reference Image Settings - serialized per document in .icy files
// ============================================================================

/// Display mode for reference images
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ReferenceImageMode {
    /// Original size, positioned at offset
    #[default]
    Original,
    /// Stretch to fill canvas (ignores aspect ratio unless locked)
    Stretch,
    /// Fit to canvas width, maintain aspect ratio
    FitWidth,
    /// Fit to canvas height, maintain aspect ratio
    FitHeight,
    /// Fit to canvas (contain), maintain aspect ratio
    Contain,
    /// Tile the image
    Tile,
}

/// Reference image settings - serialized per document in .icy files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceImageSettings {
    /// Path to the reference image file (relative to document or absolute)
    pub path: std::path::PathBuf,

    /// Alpha/transparency (0.0 = invisible, 1.0 = opaque)
    pub alpha: f32,

    /// Position offset in characters (can be negative)
    pub offset: (f32, f32),

    /// Scale factor (1.0 = original size, 2.0 = double)
    pub scale: f32,

    /// Lock aspect ratio when scaling
    pub lock_aspect_ratio: bool,

    /// Display mode
    pub mode: ReferenceImageMode,

    /// Is the reference image currently visible
    pub visible: bool,

    /// Cached RGBA image data (not serialized)
    #[serde(skip)]
    pub cached_data: Option<(Vec<u8>, u32, u32)>,

    /// Path hash when cached_data was loaded (to detect changes)
    #[serde(skip)]
    pub cached_path_hash: u64,
}

impl PartialEq for ReferenceImageSettings {
    fn eq(&self, other: &Self) -> bool {
        // Compare only serialized fields, ignore cache
        self.path == other.path
            && self.alpha == other.alpha
            && self.offset == other.offset
            && self.scale == other.scale
            && self.lock_aspect_ratio == other.lock_aspect_ratio
            && self.mode == other.mode
            && self.visible == other.visible
    }
}

impl Default for ReferenceImageSettings {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::new(),
            alpha: 0.2,
            offset: (0.0, 0.0),
            scale: 1.0,
            lock_aspect_ratio: true,
            mode: ReferenceImageMode::Original,
            visible: true,
            cached_data: None,
            cached_path_hash: 0,
        }
    }
}

impl ReferenceImageSettings {
    /// Compute a hash of the path for cache invalidation
    fn compute_path_hash(path: &std::path::Path) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        hasher.finish()
    }

    /// Load and cache the reference image data
    /// Returns the cached RGBA data (bytes, width, height) or None if loading fails
    pub fn load_and_cache(&mut self) -> Option<&(Vec<u8>, u32, u32)> {
        let path_hash = Self::compute_path_hash(&self.path);

        // Return cached data if path hasn't changed
        if self.cached_path_hash == path_hash && self.cached_data.is_some() {
            return self.cached_data.as_ref();
        }

        // Clear old cache
        self.cached_data = None;
        self.cached_path_hash = 0;

        // Check if path exists
        if self.path.as_os_str().is_empty() || !self.path.exists() {
            return None;
        }

        // Try to load the image
        match image::open(&self.path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let data = rgba.into_raw();
                self.cached_data = Some((data, width, height));
                self.cached_path_hash = path_hash;
                self.cached_data.as_ref()
            }
            Err(e) => {
                log::warn!("Failed to load reference image {:?}: {}", self.path, e);
                None
            }
        }
    }

    /// Get cached data without loading (returns None if not cached)
    pub fn get_cached(&self) -> Option<&(Vec<u8>, u32, u32)> {
        self.cached_data.as_ref()
    }

    /// Clear the cached image data
    pub fn clear_cache(&mut self) {
        self.cached_data = None;
        self.cached_path_hash = 0;
    }
} // ============================================================================
  // Editor Markers - active marker state for current editor session
  // ============================================================================

/// Active marker state for current editor session
/// This is NOT serialized in app settings - raster/guide are per-session,
/// reference_image is serialized per-document in .icy files
#[derive(Debug, Clone)]
pub struct EditorMarkers {
    /// Raster grid spacing in characters (None = disabled)
    /// Example: Some((8.0, 8.0)) for an 8x8 character grid
    pub raster: Option<(f32, f32)>,

    /// Guide crosshair position in characters (None = disabled)
    /// Example: Some((40.0, 12.0)) for a guide at column 40, row 12
    pub guide: Option<(f32, f32)>,

    /// Reference image settings (serialized per-document in .icy files)
    pub reference_image: Option<ReferenceImageSettings>,

    /// Marker visual settings (colors, alphas) from app settings
    pub marker_settings: Option<MarkerSettings>,

    /// Current layer bounds rectangle in pixels (x, y, width, height)
    /// None = layer bounds not shown
    pub layer_bounds: Option<(f32, f32, f32, f32)>,

    /// Whether to show layer bounds at all
    pub show_layer_bounds: bool,

    /// Whether paste mode is active (floating layer) - uses animated cyan border
    pub paste_mode: bool,

    /// Whether the layer border should be animated (marching ants)
    /// Set to true when paste mode is active or layer needs animated border
    pub layer_border_animated: bool,

    /// Selection rectangle in pixels (x, y, width, height)
    /// None = no selection
    pub selection_rect: Option<(f32, f32, f32, f32)>,

    /// Selection border color (RGBA) for marching ants
    /// Use selection_colors::DEFAULT, selection_colors::ADD, or selection_colors::SUBTRACT
    pub selection_color: [f32; 4],

    /// Selection mask texture data (RGBA, width in cells, height in cells)
    /// Each pixel represents one character cell: white (255,255,255,255) = selected, black (0,0,0,255) = not selected
    /// None = no selection mask (use selection_rect only)
    pub selection_mask_data: Option<(Vec<u8>, u32, u32)>,

    /// Tool overlay texture data (RGBA, width in pixels, height in pixels)
    /// Used for Moebius-style translucent tool previews (line/rect/ellipse) drawn as alpha overlays.
    /// None = no tool overlay preview.
    pub tool_overlay_mask_data: Option<(Vec<u8>, u32, u32)>,

    /// Tool overlay rectangle in document pixel space (x, y, width, height).
    /// Must match the dimensions of `tool_overlay_mask_data`.
    pub tool_overlay_rect: Option<(f32, f32, f32, f32)>,

    /// (Deprecated) Cell height scale for legacy cell-mask tool overlay sampling.
    /// Kept for compatibility; no longer used by the shader.
    pub tool_overlay_cell_height_scale: f32,

    /// Brush/Pencil preview rectangle in pixels (x, y, width, height) in document space
    /// None = no brush preview
    pub brush_preview_rect: Option<(f32, f32, f32, f32)>,

    /// Caret origin offset in document pixels (x, y).
    ///
    /// Used by editor UIs to render the caret relative to the *current layer* instead
    /// of absolute document coordinates.
    pub caret_origin_px: (f32, f32),
}

impl Default for EditorMarkers {
    fn default() -> Self {
        Self {
            raster: None,
            guide: None,
            reference_image: None,
            marker_settings: None,
            layer_bounds: None,
            show_layer_bounds: false,
            paste_mode: false,
            layer_border_animated: false,
            selection_rect: None,
            selection_color: selection_colors::DEFAULT,
            selection_mask_data: None,
            tool_overlay_mask_data: None,
            tool_overlay_rect: None,
            tool_overlay_cell_height_scale: 1.0,
            brush_preview_rect: None,

            caret_origin_px: (0.0, 0.0),
        }
    }
}

impl EditorMarkers {
    /// Create new empty markers
    pub fn new() -> Self {
        Self::default()
    }

    /// Set raster grid spacing (in pixels)
    pub fn set_raster(&mut self, width: f32, height: f32) {
        if width > 0.0 && height > 0.0 {
            self.raster = Some((width, height));
        } else {
            self.raster = None;
        }
    }

    /// Clear raster grid
    pub fn clear_raster(&mut self) {
        self.raster = None;
    }

    /// Set guide crosshair position (in pixels)
    pub fn set_guide(&mut self, x: f32, y: f32) {
        if x > 0.0 || y > 0.0 {
            self.guide = Some((x, y));
        } else {
            self.guide = None;
        }
    }

    /// Clear guide crosshair
    pub fn clear_guide(&mut self) {
        self.guide = None;
    }

    /// Check if any markers are active
    pub fn has_active_markers(&self) -> bool {
        self.raster.is_some() || self.guide.is_some() || self.reference_image.as_ref().is_some_and(|r| r.visible)
    }
}

/// Configurable colors for the transparency checkerboard pattern.
/// Used in Terminal, Minimap, and LayerView for rendering transparent areas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckerboardColors {
    /// First color of the checkerboard (typically lighter)
    pub color1: Color,
    /// Second color of the checkerboard (typically darker)
    pub color2: Color,
    /// Size of each checker cell in pixels
    pub cell_size: f32,
}

impl Default for CheckerboardColors {
    fn default() -> Self {
        // Classic Photoshop-style gray checkerboard
        Self {
            color1: Color::new(0x80, 0x80, 0x80), // Light gray
            color2: Color::new(0x60, 0x60, 0x60), // Dark gray
            cell_size: 8.0,
        }
    }
}

impl CheckerboardColors {
    /// Create a new checkerboard with custom colors
    pub fn new(color1: Color, color2: Color, cell_size: f32) -> Self {
        Self { color1, color2, cell_size }
    }

    /// Convert color1 to RGBA float array for shaders
    pub fn color1_rgba(&self) -> [f32; 4] {
        let (r, g, b) = self.color1.rgb();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }

    /// Convert color2 to RGBA float array for shaders
    pub fn color2_rgba(&self) -> [f32; 4] {
        let (r, g, b) = self.color2.rgb();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
    }
}

unsafe impl Send for MonitorSettings {}
unsafe impl Sync for MonitorSettings {}

impl Default for MonitorSettings {
    fn default() -> Self {
        MonitorSettings::neutral()
    }
}

impl MonitorSettings {
    pub fn neutral() -> Self {
        Self {
            theme: "Dark".to_string(),

            // Display settings
            monitor_type: MonitorType::Color,
            custom_monitor_color: Color::new(0xFF, 0xFF, 0xFF),

            // Color adjustments - neutral values
            brightness: 100.0, // 100% = 1.0 multiplier (neutral)
            contrast: 100.0,   // 100% = 1.0 multiplier (neutral)
            gamma: 1.0,
            saturation: 100.0, // 100% = 1.0 multiplier (full saturation)

            // Effects
            checkerboard_colors: CheckerboardColors::default(),

            // Scaling - auto-fit with integer scaling for sharp fonts
            use_integer_scaling: true,
            use_bilinear_filtering: false,
            scaling_mode: ScalingMode::Auto,

            // CRT effects - all disabled for neutral
            use_bloom: false,
            bloom_threshold: 25.0,
            bloom_radius: 3.0,
            glow_strength: 15.0,
            phosphor_persistence: 10.0,

            use_scanlines: false,
            scanline_thickness: 0.5,
            scanline_sharpness: 0.5,
            scanline_phase: 0.0,

            use_curvature: false,
            curvature_x: 60.0,
            curvature_y: 60.0,

            use_noise: false,
            noise_level: 20.0,
            sync_wobble: 20.0,
        }
    }

    fn _get_monochrome_color(&self) -> Color {
        match self.monitor_type {
            MonitorType::CustomMonochrome => self.custom_monitor_color.clone(),
            _ => self.monitor_type.color(),
        }
    }

    pub fn get_theme(&self) -> iced::Theme {
        match self.theme.as_str() {
            "Light" => iced::Theme::Light,
            "Dark" => iced::Theme::Dark,
            "Dracula" => iced::Theme::Dracula,
            "Nord" => iced::Theme::Nord,
            "SolarizedLight" => iced::Theme::SolarizedLight,
            "SolarizedDark" => iced::Theme::SolarizedDark,
            "GruvboxLight" => iced::Theme::GruvboxLight,
            "GruvboxDark" => iced::Theme::GruvboxDark,
            "Ferra" => iced::Theme::Ferra,
            "CatppuccinLatte" => iced::Theme::CatppuccinLatte,
            "CatppuccinFrappe" => iced::Theme::CatppuccinFrappe,
            "CatppuccinMacchiato" => iced::Theme::CatppuccinMacchiato,
            "CatppuccinMocha" => iced::Theme::CatppuccinMocha,
            "TokyoNight" => iced::Theme::TokyoNight,
            "TokyoNightStorm" => iced::Theme::TokyoNightStorm,
            "TokyoNightLight" => iced::Theme::TokyoNightLight,
            "KanagawaWave" => iced::Theme::KanagawaWave,
            "KanagawaDragon" => iced::Theme::KanagawaDragon,
            "KanagawaLotus" => iced::Theme::KanagawaLotus,
            "Moonfly" => iced::Theme::Moonfly,
            "Nightfly" => iced::Theme::Nightfly,
            "Oxocarbon" => iced::Theme::Oxocarbon,
            // Default to Dark theme if theme string is empty or unrecognized
            _ => iced::Theme::Dark,
        }
    }

    pub fn set_theme(&mut self, theme: iced::Theme) {
        self.theme = match theme {
            iced::Theme::Light => "Light",
            iced::Theme::Dark => "Dark",
            iced::Theme::Dracula => "Dracula",
            iced::Theme::Nord => "Nord",
            iced::Theme::SolarizedLight => "SolarizedLight",
            iced::Theme::SolarizedDark => "SolarizedDark",
            iced::Theme::GruvboxLight => "GruvboxLight",
            iced::Theme::GruvboxDark => "GruvboxDark",
            iced::Theme::CatppuccinLatte => "CatppuccinLatte",
            iced::Theme::CatppuccinFrappe => "CatppuccinFrappe",
            iced::Theme::CatppuccinMacchiato => "CatppuccinMacchiato",
            iced::Theme::CatppuccinMocha => "CatppuccinMocha",
            iced::Theme::TokyoNight => "TokyoNight",
            iced::Theme::TokyoNightStorm => "TokyoNightStorm",
            iced::Theme::TokyoNightLight => "TokyoNightLight",
            iced::Theme::KanagawaWave => "KanagawaWave",
            iced::Theme::KanagawaDragon => "KanagawaDragon",
            iced::Theme::KanagawaLotus => "KanagawaLotus",
            iced::Theme::Moonfly => "Moonfly",
            iced::Theme::Nightfly => "Nightfly",
            iced::Theme::Oxocarbon => "Oxocarbon",
            iced::Theme::Custom(_) => "Dark",
            iced::Theme::Ferra => "Ferra",
        }
        .to_string();
    }
}

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
