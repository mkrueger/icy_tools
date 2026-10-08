//! Local image import into ANSI, RIP, IGS, PETSCII and VT52 documents. The dialog and chat
//! use the same bounded character converter.

use std::{
    io::Cursor,
    path::Path,
    sync::{mpsc, Arc},
};

use eframe::egui;
use icy_draw::{fl, igs_document::IgsDocument, rip_document::RipDocument};
use icy_engine::{IceMode, PetsciiCase, PetsciiMachine, Rectangle, TextBuffer, TextPane, EGA_PALETTE};
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use icy_parser_core::{IgsCommand, IgsItem, IgsParameter, PaletteMode, PenType, RipCommand, ScreenClearMode, TerminalResolution};
use image::{imageops, ImageFormat};
use parking_lot::Mutex;
use serde_json::json;

use super::ai_chat::{
    canvas::Draft,
    image_attachment::{self, ReferenceImage},
};

pub const PETSCII_MACHINE: PetsciiMachine = PetsciiMachine::C64;
pub const PETSCII_CASE: PetsciiCase = PetsciiCase::Upper;
/// Low resolution offers 16 colors, which suits images better than the 4 of medium resolution.
pub const VT52_RESOLUTION: TerminalResolution = TerminalResolution::Low;
const RIP_SIZE: (u32, u32) = (640, 350);
const IGS_SIZE: (u32, u32) = (320, 200);

pub enum Action {
    Browse,
    Accept(Box<Accepted>),
    Generate(Box<AiRequest>),
    Cancel,
}

pub struct Accepted {
    pub imported: Imported,
}

pub struct AiRequest {
    pub image: ReferenceImage,
    pub format: &'static str,
    pub buffer: TextBuffer,
}

#[derive(Clone)]
pub enum Imported {
    Ansi(TextBuffer),
    Petscii(TextBuffer),
    Vt52(TextBuffer),
    Rip(Vec<RipCommand>),
    Igs(Vec<IgsCommand>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Target {
    #[default]
    Ansi,
    Rip,
    Igs,
    Petscii,
    Vt52,
}

impl Target {
    const ALL: [Self; 5] = [Self::Ansi, Self::Rip, Self::Igs, Self::Petscii, Self::Vt52];

    fn label(self) -> &'static str {
        match self {
            Self::Ansi => "ANSI",
            Self::Rip => "RIP",
            Self::Igs => "IGS",
            Self::Petscii => "PETSCII",
            Self::Vt52 => "VT52",
        }
    }

    /// The native screen of the character targets other than ANSI.
    fn retro_buffer(self) -> Option<TextBuffer> {
        match self {
            Self::Petscii => Some(icy_draw::screen_profile::petscii_buffer(PETSCII_MACHINE, PETSCII_CASE)),
            Self::Vt52 => Some(icy_draw::screen_profile::atari_st_buffer(VT52_RESOLUTION)),
            Self::Ansi | Self::Rip | Self::Igs => None,
        }
    }

    /// Character canvases fit the assistant's draft limits; imported RIP and IGS line runs do not.
    fn refinable(self) -> bool {
        matches!(self, Self::Ansi | Self::Petscii | Self::Vt52)
    }

    fn description(self) -> String {
        match self {
            Self::Ansi => fl!("ai-import-ansi-size"),
            Self::Rip => fl!("ai-import-rip-size"),
            Self::Igs => fl!("ai-import-igs-size"),
            Self::Petscii => fl!("ai-import-petscii-size"),
            Self::Vt52 => fl!("ai-import-vt52-size"),
        }
    }

    /// Columns and rows plus pixel size of the native screen of PETSCII and VT52. Cached, since
    /// building the screen loads its font and the dialog asks every frame.
    fn retro_geometry(self) -> Option<((i32, i32), egui::Vec2)> {
        static GEOMETRY: std::sync::OnceLock<[((i32, i32), egui::Vec2); 2]> = std::sync::OnceLock::new();
        let geometry = GEOMETRY.get_or_init(|| {
            [Self::Petscii, Self::Vt52].map(|target| {
                let buffer = target.retro_buffer().unwrap();
                let font = buffer.font_dimensions();
                (
                    (buffer.width(), buffer.height()),
                    egui::vec2((buffer.width() * font.width) as f32, (buffer.height() * font.height) as f32),
                )
            })
        });
        match self {
            Self::Petscii => Some(geometry[0]),
            Self::Vt52 => Some(geometry[1]),
            Self::Ansi | Self::Rip | Self::Igs => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Preset {
    Scene,
    Shaded,
}

impl Preset {
    fn label(self) -> String {
        match self {
            Self::Scene => fl!("ai-import-scene"),
            Self::Shaded => fl!("ai-import-shaded"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    Crop,
    Contain,
    Stretch,
}

impl Fit {
    fn label(self) -> String {
        match self {
            Self::Crop => fl!("ai-import-crop"),
            Self::Contain => fl!("ai-import-contain"),
            Self::Stretch => fl!("ai-import-stretch"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glyphs {
    HalfBlocks,
    Blocks,
    Full,
    Ascii,
}

impl Glyphs {
    fn mode(self) -> &'static str {
        match self {
            Self::HalfBlocks => "half_pixels",
            Self::Blocks => "blocks",
            Self::Full => "full",
            Self::Ascii => "ascii",
        }
    }

    fn label(self) -> String {
        match self {
            Self::HalfBlocks => fl!("ai-import-half-blocks"),
            Self::Blocks => fl!("ai-import-blocks"),
            Self::Full => fl!("ai-import-full-glyphs"),
            Self::Ascii => fl!("ai-import-ascii"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Options {
    target: Target,
    columns: i32,
    rows: i32,
    preset: Preset,
    fit: Fit,
    focus: egui::Rect,
    ice: bool,
    spacing: bool,
    aspect: bool,
    glyphs: Glyphs,
    dither: bool,
    brightness: f64,
    contrast: f64,
    saturation: f64,
    shade_penalty: f64,
    coherence: f64,
    lightness_levels: u8,
    hue_families: bool,
    local_contrast: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            target: Target::Ansi,
            columns: 80,
            rows: 50,
            preset: Preset::Scene,
            fit: Fit::Crop,
            focus: egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            ice: false,
            spacing: false,
            aspect: false,
            glyphs: Glyphs::Blocks,
            dither: false,
            brightness: 0.03,
            contrast: 1.2,
            saturation: 2.0,
            shade_penalty: 0.10,
            coherence: 0.0015,
            lightness_levels: 0,
            hue_families: true,
            local_contrast: 0.75,
        }
    }
}

impl Options {
    fn select_preset(&mut self, preset: Preset) {
        self.preset = preset;
        self.brightness = if preset == Preset::Scene { 0.03 } else { 0.0 };
        self.contrast = if preset == Preset::Scene { 1.2 } else { 1.0 };
        self.lightness_levels = if preset == Preset::Shaded && self.glyphs != Glyphs::HalfBlocks {
            8
        } else {
            0
        };
        self.saturation = 2.0;
        self.shade_penalty = if preset == Preset::Scene { 0.10 } else { 1.0 };
        self.coherence = 0.0015;
        self.hue_families = true;
        self.local_contrast = if preset == Preset::Scene { 0.75 } else { 0.5 };
    }

    fn validate(&self) -> Result<(), String> {
        if self.target == Target::Ansi && (!(1..=160).contains(&self.columns) || !(1..=200).contains(&self.rows) || self.columns * self.rows > 8000) {
            return Err(fl!("ai-import-invalid-size"));
        }
        if !self.focus.is_finite()
            || self.focus.width() <= 0.0
            || self.focus.height() <= 0.0
            || !egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)).contains_rect(self.focus)
        {
            return Err(fl!("ai-import-invalid-crop"));
        }
        Ok(())
    }

    fn target_aspect(&self) -> f64 {
        match self.target {
            Target::Ansi => {
                let buffer = self.buffer();
                let aspect = if self.aspect { buffer.get_aspect_ratio_stretch_factor() } else { 1.0 };
                self.columns as f64 * if self.spacing { 9.0 } else { 8.0 } / (self.rows as f64 * 16.0 * f64::from(aspect))
            }
            Target::Rip => f64::from(RIP_SIZE.0) / f64::from(RIP_SIZE.1),
            Target::Igs => f64::from(IGS_SIZE.0) / f64::from(IGS_SIZE.1),
            Target::Petscii | Target::Vt52 => {
                let (_, pixels) = self.target.retro_geometry().unwrap();
                f64::from(pixels.x) / f64::from(pixels.y)
            }
        }
    }

    fn buffer(&self) -> TextBuffer {
        let mut buffer = TextBuffer::new((self.columns, self.rows));
        buffer.terminal_state.is_terminal_buffer = false;
        buffer.ice_mode = if self.ice { IceMode::Ice } else { IceMode::Blink };
        buffer.set_use_letter_spacing(self.spacing);
        buffer.set_use_aspect_ratio(self.aspect);
        buffer
    }

    fn crop(&self, source: &ReferenceImage) -> (u32, u32, u32, u32) {
        let left = (self.focus.left() * source.width as f32).floor() as u32;
        let top = (self.focus.top() * source.height as f32).floor() as u32;
        let mut width = ((self.focus.right() * source.width as f32).ceil() as u32).min(source.width) - left;
        let mut height = ((self.focus.bottom() * source.height as f32).ceil() as u32).min(source.height) - top;
        let mut x = left;
        let mut y = top;
        if self.fit == Fit::Crop {
            let target = self.target_aspect();
            if f64::from(width) / f64::from(height) > target {
                let cropped = (f64::from(height) * target).round().clamp(1.0, f64::from(width)) as u32;
                x += (width - cropped) / 2;
                width = cropped;
            } else {
                let cropped = (f64::from(width) / target).round().clamp(1.0, f64::from(height)) as u32;
                y += (height - cropped) / 2;
                height = cropped;
            }
        }
        (x, y, width, height)
    }
}

struct Converted {
    imported: Imported,
    preview: egui::ColorImage,
}

#[cfg(test)]
impl Converted {
    fn ansi_buffer(&self) -> &TextBuffer {
        let Imported::Ansi(buffer) = &self.imported else {
            panic!("expected ANSI import")
        };
        buffer
    }
}

fn convert(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    options.validate()?;
    match options.target {
        Target::Ansi => convert_ansi(source, options),
        Target::Rip => convert_rip(source, options),
        Target::Igs => convert_igs(source, options),
        Target::Petscii | Target::Vt52 => convert_retro(source, options),
    }
}

fn cropped_reference(source: &ReferenceImage, options: &Options) -> Result<ReferenceImage, String> {
    let (x, y, width, height) = options.crop(source);
    let cropped = imageops::crop_imm(&source.pixels()?, x, y, width, height).to_image();
    let mut png = Cursor::new(Vec::new());
    cropped.write_to(&mut png, ImageFormat::Png).map_err(|error| error.to_string())?;
    image_attachment::prepare(&source.name, &png.into_inner())
}

/// PETSCII and VT52 screens convert literally with the native glyphs and colors of the machine.
fn convert_retro(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    let buffer = options.target.retro_buffer().ok_or("Not a character target")?;
    let reference = cropped_reference(source, options)?;
    let mut draft = Draft::new(options.target.label(), buffer, 0, None);
    draft.begin_turn(Some(reference));
    draft.convert_image(&json!({
        "preset": "faithful", "mode": "full", "dither": options.dither,
        "fit": if options.fit == Fit::Contain { "contain" } else { "stretch" },
    }))?;
    let buffer = draft.buffer;
    let (size, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, buffer.width(), buffer.height()).into(), false);
    let preview = egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &pixels);
    let imported = if options.target == Target::Petscii {
        Imported::Petscii(buffer)
    } else {
        Imported::Vt52(buffer)
    };
    Ok(Converted { imported, preview })
}

/// The cropped source scaled to a fixed pixel canvas, centered on black for "contain".
fn fit_pixels(source: &ReferenceImage, options: &Options, (canvas_width, canvas_height): (u32, u32)) -> Result<image::RgbaImage, String> {
    let (x, y, width, height) = options.crop(source);
    let source = imageops::crop_imm(&source.pixels()?, x, y, width, height).to_image();
    Ok(match options.fit {
        Fit::Crop | Fit::Stretch => imageops::resize(&source, canvas_width, canvas_height, imageops::FilterType::Triangle),
        Fit::Contain => {
            let scale = (f64::from(canvas_width) / f64::from(width)).min(f64::from(canvas_height) / f64::from(height));
            let scaled_width = (f64::from(width) * scale).round().clamp(1.0, f64::from(canvas_width)) as u32;
            let scaled_height = (f64::from(height) * scale).round().clamp(1.0, f64::from(canvas_height)) as u32;
            let scaled = imageops::resize(&source, scaled_width, scaled_height, imageops::FilterType::Triangle);
            let mut contained = image::RgbaImage::from_pixel(canvas_width, canvas_height, image::Rgba([0, 0, 0, 255]));
            imageops::overlay(
                &mut contained,
                &scaled,
                i64::from((canvas_width - scaled_width) / 2),
                i64::from((canvas_height - scaled_height) / 2),
            );
            contained
        }
    })
}

/// Horizontal runs `(x0, x1, y)` of each palette index, except `background`. Runs never
/// overlap, so drawing them grouped by color needs only one color change per color.
fn runs_by_color(indices: &[u8], width: usize, background: u8) -> Vec<Vec<(u16, u16, u16)>> {
    let mut runs = vec![Vec::new(); 16];
    for (y, row) in indices.chunks_exact(width).enumerate() {
        let mut x = 0;
        while x < row.len() {
            let color = row[x];
            let mut end = x + 1;
            while end < row.len() && row[end] == color {
                end += 1;
            }
            if color != background {
                runs[color as usize].push((x as u16, (end - 1) as u16, y as u16));
            }
            x = end;
        }
    }
    runs
}

fn convert_igs(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    let image = fit_pixels(source, options, IGS_SIZE)?;
    let mut levels = igs_palette(&image)?;
    let rgb: Vec<[f32; 3]> = levels.iter().map(|level| level.map(|value| f32::from(value * 34))).collect();
    let mut indices = quantize(&image, &rgb, options.dither);

    // Pen 0 is the color the screen is cleared to, so the most common color needs no lines.
    let mut counts = [0usize; 16];
    for index in &indices {
        counts[*index as usize] += 1;
    }
    let dominant = (0..16).max_by_key(|index| counts[*index]).unwrap_or(0) as u8;
    levels.swap(0, dominant as usize);
    for index in &mut indices {
        if *index == dominant {
            *index = 0;
        } else if *index == 0 {
            *index = dominant;
        }
    }

    let mut commands = vec![IgsCommand::SetResolution {
        resolution: TerminalResolution::Low,
        palette: PaletteMode::IgDefault,
    }];
    commands.extend(levels.iter().enumerate().map(|(pen, [red, green, blue])| IgsCommand::SetPenColor {
        pen: pen as u8,
        red: *red,
        green: *green,
        blue: *blue,
    }));
    commands.push(IgsCommand::ScreenClear {
        mode: ScreenClearMode::ClearWholeScreenAndHome,
    });
    for (pen, runs) in runs_by_color(&indices, IGS_SIZE.0 as usize, 0).into_iter().enumerate() {
        if runs.is_empty() {
            continue;
        }
        commands.push(IgsCommand::ColorSet {
            pen: PenType::Line,
            color: pen as u8,
        });
        commands.extend(runs.into_iter().map(|(x0, x1, y)| IgsCommand::Line {
            x1: IgsParameter::Value(x0.into()),
            y1: IgsParameter::Value(y.into()),
            x2: IgsParameter::Value(x1.into()),
            y2: IgsParameter::Value(y.into()),
        }));
    }
    let items: Vec<IgsItem> = commands.iter().cloned().map(IgsItem::from).collect();
    let preview = IgsDocument::render(&items).map_err(|error| error.to_string())?;
    let size = [preview.width(), preview.height()];
    Ok(Converted {
        imported: Imported::Igs(commands),
        preview: egui::ColorImage::from_rgba_unmultiplied(size, &preview.rgba()),
    })
}

/// 16 distinct Atari ST colors (3 bits per channel) adapted to the image.
fn igs_palette(image: &image::RgbaImage) -> Result<Vec<[u8; 3]>, String> {
    use quantette::{deps::palette::Srgb, Image, PaletteSize, Pipeline};

    let pixels: Vec<Srgb<u8>> = image
        .pixels()
        .map(|pixel| {
            let alpha = u16::from(pixel[3]);
            let channel = |value: u8| (u16::from(value) * alpha / 255) as u8;
            Srgb::new(channel(pixel[0]), channel(pixel[1]), channel(pixel[2]))
        })
        .collect();
    let input = Image::new(image.width(), image.height(), pixels).map_err(|error| error.to_string())?;
    let size = PaletteSize::try_from(16u16).map_err(|_| "Invalid palette size")?;
    let indexed = Pipeline::new().palette_size(size).input_image(input.as_ref()).output_srgb8_indexed_image();
    let level = |value: u8| ((u16::from(value) * 7 + 127) / 255) as u8;
    let mut palette: Vec<[u8; 3]> = Vec::with_capacity(16);
    let grays = (0..8).map(|value| [value; 3]);
    for color in indexed
        .palette()
        .iter()
        .map(|color| [level(color.red), level(color.green), level(color.blue)])
        .chain(grays)
    {
        if palette.len() == 16 {
            break;
        }
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    let mut extra = (0..512u16).map(|value| [(value >> 6) as u8, ((value >> 3) & 7) as u8, (value & 7) as u8]);
    while palette.len() < 16 {
        let color = extra.next().ok_or("Palette exhausted")?;
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    Ok(palette)
}

fn convert_ansi(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    let reference = cropped_reference(source, options)?;
    let mut draft = Draft::new("ANSI/ASCII", options.buffer(), 0, None);
    draft.begin_turn(Some(reference));
    draft.convert_image(&json!({
        "preset": "scene", "mode": options.glyphs.mode(), "dither": options.dither,
        "fit": if options.fit == Fit::Contain { "contain" } else { "stretch" },
        "brightness": options.brightness, "contrast": options.contrast, "saturation": options.saturation,
        "shade_penalty": options.shade_penalty, "coherence": options.coherence,
        "lightness_levels": options.lightness_levels, "hue_families": options.hue_families,
        "local_contrast": options.local_contrast,
    }))?;
    let buffer = draft.buffer;
    let (size, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, options.columns, options.rows).into(), false);
    let preview = if options.aspect {
        let height = (size.height as f32 * buffer.get_aspect_ratio_stretch_factor()).round().max(1.0) as u32;
        let raw = image::RgbaImage::from_raw(size.width as u32, size.height as u32, pixels).ok_or("Invalid rendered image dimensions")?;
        let corrected = imageops::resize(&raw, size.width as u32, height, imageops::FilterType::Nearest);
        egui::ColorImage::from_rgba_unmultiplied([size.width as usize, height as usize], corrected.as_raw())
    } else {
        egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &pixels)
    };
    Ok(Converted {
        imported: Imported::Ansi(buffer),
        preview,
    })
}

fn convert_rip(source: &ReferenceImage, options: &Options) -> Result<Converted, String> {
    let image = fit_pixels(source, options, RIP_SIZE)?;
    let palette = rip_palette(&image);
    let rgb: Vec<[f32; 3]> = palette
        .iter()
        .map(|index| {
            let (r, g, b) = EGA_PALETTE[*index as usize].rgb();
            [f32::from(r), f32::from(g), f32::from(b)]
        })
        .collect();
    let indices = quantize(&image, &rgb, options.dither);
    let mut commands = vec![RipCommand::SetPalette { colors: palette.clone() }];
    for (color, runs) in runs_by_color(&indices, RIP_SIZE.0 as usize, 0).into_iter().enumerate() {
        if runs.is_empty() {
            continue;
        }
        commands.push(RipCommand::Color { c: color as u16 });
        commands.extend(runs.into_iter().map(|(x0, x1, y)| RipCommand::Line { x0, y0: y, x1, y1: y }));
    }
    let mut document = RipDocument::new();
    document.replace_editable(commands.clone()).map_err(|error| error.to_string())?;
    let preview = document.preview().map_err(|error| error.to_string())?;
    let rgba = preview.rgba();
    Ok(Converted {
        imported: Imported::Rip(commands),
        preview: egui::ColorImage::from_rgba_unmultiplied([RIP_SIZE.0 as usize, RIP_SIZE.1 as usize], &rgba),
    })
}

fn rip_palette(image: &image::RgbaImage) -> Vec<u16> {
    const DEFAULT: [u16; 16] = [0, 1, 2, 3, 4, 5, 20, 7, 56, 57, 58, 59, 60, 61, 62, 63];
    let ega: Vec<[f32; 3]> = EGA_PALETTE
        .iter()
        .map(|color| {
            let (r, g, b) = color.rgb();
            [f32::from(r), f32::from(g), f32::from(b)]
        })
        .collect();
    let mut frequency = [0usize; 64];
    for pixel in image.pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let rgb = [f32::from(pixel[0]) * alpha, f32::from(pixel[1]) * alpha, f32::from(pixel[2]) * alpha];
        let nearest = ega
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                color_distance(rgb, **left)
                    .partial_cmp(&color_distance(rgb, **right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or(0, |(index, _)| index);
        frequency[nearest] += 1;
    }
    let mut ranked: Vec<_> = (1..64).collect();
    ranked.sort_unstable_by_key(|index| std::cmp::Reverse(frequency[*index]));
    let mut palette = vec![0];
    palette.extend(ranked.into_iter().filter(|index| frequency[*index] > 0).take(15).map(|index| index as u16));
    for color in DEFAULT.into_iter().chain(0..64) {
        if palette.len() == 16 {
            break;
        }
        if !palette.contains(&color) {
            palette.push(color);
        }
    }
    palette
}

fn quantize(image: &image::RgbaImage, palette: &[[f32; 3]], dither: bool) -> Vec<u8> {
    let mut pixels: Vec<[f32; 3]> = image
        .pixels()
        .map(|pixel| {
            let alpha = f32::from(pixel[3]) / 255.0;
            [f32::from(pixel[0]) * alpha, f32::from(pixel[1]) * alpha, f32::from(pixel[2]) * alpha]
        })
        .collect();
    let width = image.width() as usize;
    let mut indices = vec![0; pixels.len()];
    for index in 0..pixels.len() {
        let old = pixels[index];
        let (color, mapped) = palette
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                color_distance(old, **left)
                    .partial_cmp(&color_distance(old, **right))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        indices[index] = color as u8;
        if !dither {
            continue;
        }
        let error = [old[0] - mapped[0], old[1] - mapped[1], old[2] - mapped[2]];
        let x = index % width;
        let distribute = |pixels: &mut [[f32; 3]], at: usize, factor: f32| {
            if let Some(pixel) = pixels.get_mut(at) {
                for channel in 0..3 {
                    pixel[channel] = (pixel[channel] + error[channel] * factor).clamp(0.0, 255.0);
                }
            }
        };
        if x + 1 < width {
            distribute(&mut pixels, index + 1, 7.0 / 16.0);
        }
        if index + width < pixels.len() {
            if x > 0 {
                distribute(&mut pixels, index + width - 1, 3.0 / 16.0);
            }
            distribute(&mut pixels, index + width, 5.0 / 16.0);
            if x + 1 < width {
                distribute(&mut pixels, index + width + 1, 1.0 / 16.0);
            }
        }
    }
    indices
}

fn color_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2)
}

enum Completed {
    Source(ReferenceImage),
    Result(Converted),
}

type Worker = Arc<Mutex<mpsc::Receiver<Result<Completed, String>>>>;

/// Settings must rest this long before the local preview is recomputed.
const PREVIEW_DELAY: f64 = 0.2;
/// Below this body width the previews and settings share one scrolling column.
const STACK_WIDTH: f32 = 760.0;
const SIDEBAR_WIDTH: f32 = 360.0;
const PANE_HEADER: f32 = 30.0;
const STATUS_HEIGHT: f32 = 44.0;
const CANVAS_PADDING: f32 = 12.0;
/// Crisp when enlarged, smooth when a large character screen is reduced to fit.
const PREVIEW_TEXTURE: egui::TextureOptions = egui::TextureOptions {
    magnification: egui::TextureFilter::Nearest,
    minification: egui::TextureFilter::Linear,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: None,
};

#[derive(Clone, Default)]
pub struct ImportDialog {
    source: Option<ReferenceImage>,
    source_texture: Option<egui::TextureHandle>,
    result: Option<Arc<Converted>>,
    result_texture: Option<egui::TextureHandle>,
    /// The previous preview, shown dimmed while the preview of changed settings is computed.
    stale_texture: Option<egui::TextureHandle>,
    options: Options,
    worker: Option<Worker>,
    /// The worker computes a preview rather than loading the source, so changed settings discard it.
    converting: bool,
    /// When the settings last changed; the local preview follows once they rest.
    changed_at: Option<f64>,
    error: Option<String>,
    drag_start: Option<egui::Pos2>,
    /// Copilot is connected and a model has been selected.
    ai_available: bool,
    ai_unavailable_reason: Option<String>,
    ai_job: Option<Arc<Mutex<Option<super::ai_chat::ImportJob>>>>,
    /// The result was drawn by the AI rather than converted locally.
    ai_result: bool,
    /// Why the last AI drawing failed. Unlike `error` it does not stop the local preview.
    ai_error: Option<String>,
}

impl ImportDialog {
    pub fn set_ai_availability(&mut self, availability: Result<(), String>) {
        self.ai_available = availability.is_ok();
        self.ai_unavailable_reason = availability.err();
        if !self.ai_available && self.ai_job.is_some() {
            self.cancel_ai();
            self.ai_error = self.ai_unavailable_reason.clone();
        }
    }

    fn start(&mut self, context: &egui::Context, work: impl FnOnce() -> Result<Completed, String> + Send + 'static) {
        self.invalidate();
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(Arc::new(Mutex::new(receiver)));
        let context = context.clone();
        std::thread::spawn(move || {
            let _ = sender.send(work());
            context.request_repaint();
        });
    }

    pub fn load(&mut self, context: &egui::Context, path: &Path) {
        self.load_file(
            context,
            egui::DroppedFile {
                path: Some(path.to_path_buf()),
                ..Default::default()
            },
        );
    }

    fn load_file(&mut self, context: &egui::Context, file: egui::DroppedFile) {
        self.source = None;
        self.source_texture = None;
        self.changed_at = None;
        self.ai_error = None;
        self.options.focus = Options::default().focus;
        self.drag_start = None;
        self.start(context, move || image_attachment::read_drop(file).map(Completed::Source));
        self.stale_texture = None;
    }

    /// Loads an image dropped anywhere on the window, as the dialog covers the whole application.
    fn take_drop(&mut self, context: &egui::Context) {
        if self.ai_job.is_some() {
            return;
        }
        let files = context.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
        if let Some(file) = files.into_iter().find(|file| file.path.is_some() || file.bytes.is_some()) {
            self.load_file(context, file);
        }
    }

    fn invalidate(&mut self) {
        self.cancel_ai();
        if self.converting {
            self.worker = None;
            self.converting = false;
        }
        if let Some(texture) = self.result_texture.take() {
            self.stale_texture = Some(texture);
        }
        self.result = None;
        self.ai_result = false;
        self.error = None;
    }

    fn poll(&mut self) {
        if let Some(job) = &self.ai_job {
            let message = job.lock().as_ref().and_then(super::ai_chat::ImportJob::poll);
            if let Some(message) = message {
                self.cancel_ai();
                match message.and_then(|buffer| self.converted_buffer(buffer)) {
                    Ok(result) => {
                        self.result = Some(Arc::new(result));
                        self.ai_result = true;
                    }
                    Err(error) => {
                        log::warn!("AI image import failed: {error}");
                        self.ai_error = Some(error);
                    }
                }
            }
        }
        let Some(worker) = &self.worker else { return };
        let message = worker.lock().try_recv();
        let result = match message {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(fl!("ai-import-worker-failed")),
        };
        self.worker = None;
        self.converting = false;
        match result {
            Ok(Completed::Source(source)) => self.source = Some(source),
            Ok(Completed::Result(result)) => self.result = Some(Arc::new(result)),
            Err(error) => {
                log::warn!("Image import failed: {error}");
                self.error = Some(error);
            }
        }
    }

    fn start_conversion(&mut self, context: &egui::Context) {
        let Some(source) = self.source.clone() else {
            self.error = Some(fl!("ai-import-select-source"));
            return;
        };
        if let Err(error) = self.options.validate() {
            self.error = Some(error);
            return;
        }
        let options = self.options.clone();
        self.start(context, move || convert(&source, &options).map(Completed::Result));
        self.converting = true;
    }

    /// Starts the local preview once the settings rest; AI drawings are only requested explicitly.
    fn update_preview(&mut self, context: &egui::Context) {
        if self.source.is_none()
            || self.result.is_some()
            || self.worker.is_some()
            || self.ai_job.is_some()
            || self.error.is_some()
            || self.options.validate().is_err()
        {
            return;
        }
        // Slider drags convert once on release rather than for every intermediate value.
        if context.input(|input| input.pointer.any_down()) {
            return;
        }
        if let Some(changed) = self.changed_at {
            let remaining = changed + PREVIEW_DELAY - context.input(|input| input.time);
            if remaining > 0.0 {
                context.request_repaint_after_secs(remaining as f32);
                return;
            }
        }
        self.changed_at = None;
        self.start_conversion(context);
    }

    /// The local preview is outdated and will be recomputed without further input.
    fn updating(&self) -> bool {
        self.converting || (self.source.is_some() && self.result.is_none() && self.ai_job.is_none() && self.error.is_none() && self.options.validate().is_ok())
    }

    pub fn show(&mut self, context: &egui::Context, blocked: bool) -> Option<Action> {
        #[derive(Clone, Copy)]
        enum Button {
            Generate,
            Stop,
            Accept,
            Cancel,
        }
        self.poll();
        if !blocked {
            self.take_drop(context);
        }
        let before = self.options.clone();
        let mut browse = false;
        let response = appearance::Dialog::new("ai-import")
            .title(fl!("ai-import-title"))
            .subtitle(fl!("ai-import-description"))
            .size(DialogSize::Width(1120.0))
            .fixed_height(820.0)
            .scroll(false)
            .show(context, |dialog| {
                let height = dialog.body_height();
                dialog.content(|ui| {
                    if blocked {
                        ui.disable();
                    }
                    browse = self.body(ui, height);
                });
                if self.options != before {
                    self.invalidate();
                    self.ai_error = None;
                    self.changed_at = Some(context.input(|input| input.time));
                }
                let idle = self.worker.is_none() && self.ai_job.is_none();
                let mut buttons = Vec::new();
                if self.ai_job.is_some() {
                    buttons.push(DialogButton::secondary(fl!("ai-import-stop"), Button::Stop).leading().enabled(!blocked));
                } else if self.options.target.refinable() {
                    let mut tooltip = fl!("ai-import-refine-hint");
                    if let Some(reason) = self.ai_unavailable_reason.as_ref().filter(|_| !self.ai_available) {
                        tooltip = format!("{tooltip}\n\n{reason}");
                    }
                    let loading = self.worker.is_some() && !self.converting;
                    buttons.push(
                        DialogButton::secondary(fl!("ai-import-generate"), Button::Generate)
                            .leading()
                            .enabled(!blocked && !loading && self.source.is_some() && self.options.validate().is_ok())
                            .tooltip(tooltip),
                    );
                }
                buttons.push(DialogButton::cancel(labels::cancel(), Button::Cancel).enabled(!blocked));
                buttons.push(
                    DialogButton::primary(fl!("ai-import-accept", format = self.options.target.label()), Button::Accept)
                        .enabled(!blocked && idle && self.result.is_some()),
                );
                dialog.buttons(buttons);
            });
        if blocked {
            return None;
        }
        let idle = self.worker.is_none() && self.ai_job.is_none();
        let action = match response.action {
            Some(Button::Accept) if idle => self.accepted().map(Action::Accept),
            Some(Button::Generate) if self.ai_job.is_none() => self.generate(),
            Some(Button::Stop) => {
                self.cancel_ai();
                None
            }
            Some(Button::Cancel) => {
                self.cancel_ai();
                Some(Action::Cancel)
            }
            _ if response.dismissed => {
                self.cancel_ai();
                Some(Action::Cancel)
            }
            _ if browse && self.ai_job.is_none() => Some(Action::Browse),
            _ => None,
        };
        if action.is_none() {
            self.update_preview(context);
        }
        action
    }

    /// Asks the AI to draw the picture, or explains why it cannot; the local preview is kept then.
    fn generate(&mut self) -> Option<Action> {
        if !self.options.target.refinable() {
            return None;
        }
        if !self.ai_available {
            self.ai_error = Some(self.ai_unavailable_reason.clone().unwrap_or_else(|| fl!("ai-import-refine-unavailable")));
            return None;
        }
        match self.ai_request() {
            Ok(request) => {
                self.invalidate();
                self.ai_error = None;
                Some(Action::Generate(Box::new(request)))
            }
            Err(error) => {
                log::warn!("Cannot prepare AI image import: {error}");
                self.ai_error = Some(error);
                None
            }
        }
    }

    fn accepted(&mut self) -> Option<Box<Accepted>> {
        let imported = self.result.as_ref()?.imported.clone();
        Some(Box::new(Accepted { imported }))
    }

    fn ai_request(&self) -> Result<AiRequest, String> {
        self.options.validate()?;
        let source = self.source.as_ref().ok_or_else(|| fl!("ai-import-select-source"))?;
        let buffer = if self.options.target == Target::Ansi {
            self.options.buffer()
        } else {
            self.options.target.retro_buffer().ok_or_else(|| fl!("ai-import-refine-unavailable"))?
        };
        let font = buffer.font_dimensions();
        let size = ((buffer.width() * font.width) as u32, (buffer.height() * font.height) as u32);
        let fitted = fit_pixels(source, &self.options, size)?;
        let mut png = Cursor::new(Vec::new());
        fitted.write_to(&mut png, ImageFormat::Png).map_err(|error| error.to_string())?;
        Ok(AiRequest {
            image: image_attachment::prepare(&source.name, &png.into_inner())?,
            format: self.options.target.label(),
            buffer,
        })
    }

    pub fn begin_ai(&mut self, job: Result<super::ai_chat::ImportJob, String>) {
        match job {
            Ok(job) => self.ai_job = Some(Arc::new(Mutex::new(Some(job)))),
            Err(error) => {
                log::warn!("Cannot start AI image import: {error}");
                self.error = Some(error);
            }
        }
    }

    fn cancel_ai(&mut self) {
        if let Some(job) = self.ai_job.take() {
            job.lock().take();
        }
    }

    fn converted_buffer(&self, buffer: TextBuffer) -> Result<Converted, String> {
        let imported = match self.options.target {
            Target::Ansi => Imported::Ansi(buffer.clone()),
            Target::Petscii => Imported::Petscii(buffer.clone()),
            Target::Vt52 => Imported::Vt52(buffer.clone()),
            _ => return Err("AI image authoring requires a character target".into()),
        };
        let (size, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, buffer.width(), buffer.height()).into(), false);
        let preview = egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &pixels);
        Ok(Converted { imported, preview })
    }

    /// Previews beside a settings column, or everything in one scrolling column when narrow.
    /// Returns whether a new source image was requested.
    fn body(&mut self, ui: &mut egui::Ui, height: f32) -> bool {
        let mut browse = false;
        if ui.available_width() >= STACK_WIDTH {
            ui.horizontal_top(|ui| {
                let gap = 20.0;
                let stage_width = (ui.available_width() - SIDEBAR_WIDTH - gap).max(0.0);
                ui.allocate_ui_with_layout(egui::vec2(stage_width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_size(egui::vec2(stage_width, height));
                    browse = self.stage(ui, height);
                });
                ui.add_space(gap - ui.spacing().item_spacing.x);
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::top_down(egui::Align::Min), |ui| {
                    scrolling(ui, "ai-import-settings", height, |ui| self.settings(ui));
                });
            });
        } else {
            scrolling(ui, "ai-import-body", height, |ui| {
                let stage = (ui.available_width() * 1.25).clamp(360.0, 640.0);
                browse = self.stage(ui, stage);
                ui.add_space(8.0);
                self.settings(ui);
            });
        }
        browse
    }

    /// Source and result side by side, or stacked when that shows them larger, above a status line.
    fn stage(&mut self, ui: &mut egui::Ui, height: f32) -> bool {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let panes = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() - STATUS_HEIGHT));
        let gap = 12.0;
        let source_size = self
            .source
            .as_ref()
            .map_or(egui::vec2(4.0, 3.0), |source| egui::vec2(source.width as f32, source.height as f32));
        let result_size = self.preview_size();
        let shown = |pane: egui::Vec2| -> f32 {
            let canvas = pane - egui::vec2(CANVAS_PADDING * 2.0, PANE_HEADER + CANVAS_PADDING * 2.0);
            [source_size, result_size]
                .into_iter()
                .map(|size| {
                    let fitted = fit_preview(size, canvas.max(egui::Vec2::splat(1.0)));
                    fitted.x * fitted.y
                })
                .sum()
        };
        let side = egui::vec2((panes.width() - gap) / 2.0, panes.height());
        let stacked = egui::vec2(panes.width(), (panes.height() - gap) / 2.0);
        let (source_rect, result_rect) = if shown(stacked) > shown(side) {
            (
                egui::Rect::from_min_size(panes.min, stacked),
                egui::Rect::from_min_max(egui::pos2(panes.left(), panes.bottom() - stacked.y), panes.max),
            )
        } else {
            (
                egui::Rect::from_min_size(panes.min, side),
                egui::Rect::from_min_max(egui::pos2(panes.right() - side.x, panes.top()), panes.max),
            )
        };
        let browse = self.source_pane(ui, source_rect);
        self.result_pane(ui, result_rect);
        self.status_line(ui, egui::Rect::from_min_max(egui::pos2(rect.left(), panes.bottom()), rect.max));
        browse
    }

    fn preview_size(&self) -> egui::Vec2 {
        if let Some(result) = &self.result {
            return egui::vec2(result.preview.size[0] as f32, result.preview.size[1] as f32);
        }
        match self.options.target.retro_geometry() {
            Some((_, pixels)) => pixels,
            None => egui::vec2(self.options.target_aspect() as f32, 1.0),
        }
    }

    /// Size of the result in characters or pixels, as shown above the preview.
    fn output_size(&self) -> (i32, i32) {
        match self.options.target {
            Target::Ansi => (self.options.columns, self.options.rows),
            Target::Rip => (RIP_SIZE.0 as i32, RIP_SIZE.1 as i32),
            Target::Igs => (IGS_SIZE.0 as i32, IGS_SIZE.1 as i32),
            Target::Petscii | Target::Vt52 => self.options.target.retro_geometry().map_or((0, 0), |(cells, _)| cells),
        }
    }

    fn source_pane(&mut self, ui: &mut egui::Ui, rect: egui::Rect) -> bool {
        let mut browse = false;
        let locked = self.ai_job.is_some();
        let (meta, tooltip) = match &self.source {
            Some(source) => (
                format!("{}  ·  {} × {}", source.name, source.width, source.height),
                (source.original_size != (source.width, source.height))
                    .then(|| fl!("ai-import-normalized", width = source.original_size.0, height = source.original_size.1)),
            ),
            None => (String::new(), None),
        };
        let loaded = self.source.is_some();
        pane_header(ui, rect, &fl!("ai-import-source"), &meta, tooltip.as_deref(), |ui| {
            if loaded {
                browse = ui.add_enabled(!locked, egui::Button::new(fl!("ai-import-browse")).small()).clicked();
            }
        });
        let canvas = canvas_rect(rect);
        let dragging_files = !locked && ui.input(|input| !input.raw.hovered_files.is_empty());
        if loaded {
            let stroke = if dragging_files {
                egui::Stroke::new(2.0, appearance::PRIMARY)
            } else {
                canvas_stroke(ui)
            };
            paint_canvas(ui, canvas, stroke);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(canvas.shrink(CANVAS_PADDING))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let height = child.available_height();
            self.source_preview(&mut child, height);
        } else {
            browse |= self.drop_zone(ui, canvas, dragging_files, locked);
        }
        browse
    }

    /// Empty source pane that loads an image when clicked or when a file is dropped on the dialog.
    fn drop_zone(&self, ui: &mut egui::Ui, rect: egui::Rect, dragging_files: bool, locked: bool) -> bool {
        let loading = self.worker.is_some();
        let sense = if locked || loading { egui::Sense::hover() } else { egui::Sense::click() };
        let response = ui.interact(rect, ui.id().with("ai-import-drop"), sense);
        let active = dragging_files || (response.hovered() && ui.is_enabled() && !loading);
        let fill = if dragging_files {
            appearance::PRIMARY.gamma_multiply(0.12)
        } else {
            canvas_fill(ui)
        };
        ui.painter().rect_filled(rect, 8, fill);
        let color = if active {
            appearance::PRIMARY
        } else {
            ui.visuals().weak_text_color().gamma_multiply(0.6)
        };
        let border = rect.shrink(1.0);
        ui.painter().extend(egui::Shape::dashed_line(
            &[
                border.left_top(),
                border.right_top(),
                border.right_bottom(),
                border.left_bottom(),
                border.left_top(),
            ],
            egui::Stroke::new(1.5, color),
            6.0,
            4.0,
        ));
        if loading {
            busy_indicator(ui, rect, &fl!("ai-import-working"), None);
            return false;
        }
        let center = rect.center();
        paint_picture_icon(ui.painter(), center - egui::vec2(0.0, 52.0), color);
        let title = if dragging_files {
            fl!("ai-import-drop-release")
        } else {
            fl!("ai-import-drop")
        };
        centered_text(
            ui,
            center - egui::vec2(0.0, 6.0),
            &title,
            16.0,
            ui.visuals().strong_text_color(),
            rect.width() - 32.0,
        );
        centered_text(
            ui,
            center + egui::vec2(0.0, 18.0),
            &fl!("ai-import-select-source"),
            12.5,
            ui.visuals().weak_text_color(),
            rect.width() - 32.0,
        );
        let button = egui::Rect::from_center_size(center + egui::vec2(0.0, 60.0), egui::vec2(150.0, 30.0));
        // A detached child, since `Ui::put` would move the cursor of the surrounding layout back up.
        let clicked = ui
            .new_child(
                egui::UiBuilder::new()
                    .max_rect(button)
                    .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
            )
            .button(fl!("ai-import-browse"))
            .clicked();
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        !locked && (clicked || response.clicked())
    }

    fn result_pane(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let (width, height) = self.output_size();
        let meta = format!("{}  ·  {} × {}", self.options.target.label(), width, height);
        let ai_result = self.ai_result && self.result.is_some();
        pane_header(ui, rect, &fl!("ai-import-preview"), &meta, None, |ui| {
            if ai_result {
                appearance::status_badge(ui, &fl!("ai-import-ai-result"), appearance::PRIMARY);
            }
        });
        let canvas = canvas_rect(rect);
        paint_canvas(ui, canvas, canvas_stroke(ui));
        let inner = canvas.shrink(CANVAS_PADDING);
        if let Some(result) = &self.result {
            self.stale_texture = None;
            let texture = self
                .result_texture
                .get_or_insert_with(|| ui.ctx().load_texture("import-result", result.preview.clone(), PREVIEW_TEXTURE));
            paint_preview(ui, texture, inner, egui::Color32::WHITE);
        } else if self.ai_job.is_some() {
            busy_indicator(ui, inner, &fl!("ai-import-ai-working"), self.stale_texture.as_ref());
        } else if self.updating() {
            busy_indicator(ui, inner, &fl!("ai-import-working"), self.stale_texture.as_ref());
        } else {
            let text = if self.source.is_none() {
                fl!("ai-import-preview-empty")
            } else {
                fl!("ai-import-preview-unavailable")
            };
            centered_text(ui, inner.center(), &text, 13.0, ui.visuals().weak_text_color(), inner.width().min(340.0));
        }
    }

    fn status_line(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let mut ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(egui::vec2(2.0, 6.0)))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let locked = self.ai_job.is_some();
        if self.source.is_some()
            && self.options.focus != Options::default().focus
            && ui.add_enabled(!locked, egui::Button::new(fl!("ai-import-reset-focus")).small()).clicked()
        {
            self.options.focus = Options::default().focus;
        }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let error = self.error.clone().or_else(|| self.ai_error.clone()).or_else(|| self.options.validate().err());
            if let Some(error) = error.filter(|_| !locked) {
                ui.add(egui::Label::new(egui::RichText::new(error).color(icy_engine_gui::egui::dialog::DANGER)).wrap());
            } else if self.source.is_some() {
                ui.add(egui::Label::new(egui::RichText::new(fl!("ai-import-focus-hint")).weak()).wrap());
            }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let locked = self.ai_job.is_some();
        ui.add_enabled_ui(!locked, |ui| {
            self.output_settings(ui);
            self.conversion_settings(ui);
            if self.options.target == Target::Ansi {
                self.tone_settings(ui);
            }
        });
    }

    fn output_settings(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!("ai-import-output"), |ui| {
            caption(ui, &fl!("ai-import-target"));
            let targets = Target::ALL.map(|target| (target, target.label(), ""));
            crate::widgets::segmented_grid(ui, &mut self.options.target, &targets, targets.len());
            ui.add(egui::Label::new(egui::RichText::new(self.options.target.description()).weak().size(12.0)).wrap());
            let ansi = self.options.target == Target::Ansi;
            if ansi {
                ui.add_space(4.0);
                caption(ui, &fl!("ai-import-canvas"));
                let mut size = (self.options.columns, self.options.rows);
                let sizes = [(80, 25), (80, 50)].map(|(columns, rows)| ((columns, rows), format!("{columns} × {rows}"), String::new()));
                if crate::widgets::segmented_grid(ui, &mut size, &sizes, sizes.len()) {
                    (self.options.columns, self.options.rows) = size;
                }
                ui.horizontal(|ui| {
                    ui.label(fl!("ai-import-columns"));
                    ui.add(egui::DragValue::new(&mut self.options.columns).range(1..=160));
                    ui.add_space(12.0);
                    ui.label(fl!("ai-import-rows"));
                    ui.add(egui::DragValue::new(&mut self.options.rows).range(1..=200));
                });
            }
            ui.add_space(4.0);
            caption(ui, &fl!("ai-import-fit"));
            let fits = [Fit::Crop, Fit::Contain, Fit::Stretch].map(|fit| (fit, fit.label(), String::new()));
            crate::widgets::segmented_grid(ui, &mut self.options.fit, &fits, fits.len());
            if ansi {
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::toggle(ui, &fl!("ai-import-ice"), &mut self.options.ice, "");
                    crate::widgets::toggle(ui, &fl!("ai-import-spacing"), &mut self.options.spacing, "");
                    crate::widgets::toggle(ui, &fl!("ai-import-aspect"), &mut self.options.aspect, "");
                });
            }
        });
    }

    fn conversion_settings(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!("ai-import-conversion"), |ui| {
            if self.options.target != Target::Ansi {
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::toggle(ui, &fl!("ai-import-dither"), &mut self.options.dither, "");
                });
                return;
            }
            caption(ui, &fl!("ai-import-style"));
            let mut preset = self.options.preset;
            let presets = [Preset::Scene, Preset::Shaded].map(|preset| (preset, preset.label(), String::new()));
            if crate::widgets::segmented_grid(ui, &mut preset, &presets, presets.len()) {
                self.options.select_preset(preset);
            }
            ui.add_space(4.0);
            caption(ui, &fl!("ai-import-glyphs"));
            let glyphs = [Glyphs::HalfBlocks, Glyphs::Blocks, Glyphs::Full, Glyphs::Ascii].map(|glyphs| (glyphs, glyphs.label(), String::new()));
            if crate::widgets::segmented_grid(ui, &mut self.options.glyphs, &glyphs, 2) {
                self.options.dither = self.options.glyphs == Glyphs::HalfBlocks;
                if self.options.glyphs == Glyphs::HalfBlocks {
                    self.options.lightness_levels = 0;
                }
            }
            ui.horizontal_wrapped(|ui| {
                crate::widgets::toggle(ui, &fl!("ai-import-hue-families"), &mut self.options.hue_families, "");
                crate::widgets::toggle(ui, &fl!("ai-import-dither"), &mut self.options.dither, "");
            });
        });
    }

    fn tone_settings(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, "", |ui| {
            egui::CollapsingHeader::new(appearance::bold(ui, fl!("ai-import-tuning")))
                .id_salt("ai-import-tuning")
                .show(ui, |ui| {
                    let options = &mut self.options;
                    slider_field(ui, &fl!("ai-import-brightness"), &mut options.brightness, -0.25..=0.25, 1.0);
                    slider_field(ui, &fl!("ai-import-contrast"), &mut options.contrast, 0.5..=2.0, 1.0);
                    slider_field(ui, &fl!("ai-import-local-contrast"), &mut options.local_contrast, 0.0..=2.0, 1.0);
                    slider_field(ui, &fl!("ai-import-saturation"), &mut options.saturation, 0.0..=2.0, 1.0);
                    slider_field(ui, &fl!("ai-import-shading"), &mut options.shade_penalty, 0.0..=2.0, 1.0);
                    // Shown in thousandths, which the slider's two decimals can still resolve.
                    slider_field(ui, &fl!("ai-import-coherence"), &mut options.coherence, 0.0..=0.02, 1000.0);
                    ui.add_space(4.0);
                    let mut levels = options.lightness_levels != 0;
                    if toggle_row(ui, &fl!("ai-import-levels"), &mut levels).changed() {
                        options.lightness_levels = if levels { 5 } else { 0 };
                    }
                    if levels {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(fl!("ai-import-levels-count")).weak());
                            ui.add(egui::DragValue::new(&mut options.lightness_levels).range(2..=16));
                        });
                    }
                    ui.add_space(4.0);
                    if ui.button(labels::restore_defaults()).clicked() {
                        let preset = options.preset;
                        options.select_preset(preset);
                    }
                });
        });
    }

    fn source_preview(&mut self, ui: &mut egui::Ui, height: f32) {
        let Some(source) = &self.source else {
            ui.allocate_space(egui::vec2(ui.available_width(), height));
            return;
        };
        let texture = self
            .source_texture
            .get_or_insert_with(|| ui.ctx().load_texture("import-source", source.color_image(), egui::TextureOptions::LINEAR));
        let (area, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
        let rect = egui::Rect::from_center_size(area.center(), fit_preview(texture.size_vec2(), area.size()));
        let sense = if self.ai_job.is_some() { egui::Sense::hover() } else { egui::Sense::drag() };
        let response = ui.interact(rect, ui.id().with("import-focus"), sense);
        egui::Image::new(&*texture).paint_at(ui, rect);
        let normalize = |point: egui::Pos2| {
            let delta = (point - rect.min) / rect.size();
            egui::pos2(delta.x.clamp(0.0, 1.0), delta.y.clamp(0.0, 1.0))
        };
        if response.drag_started() {
            self.drag_start = ui.input(|input| input.pointer.press_origin()).map(normalize);
        }
        let dragging = self
            .drag_start
            .zip(response.interact_pointer_pos())
            .map(|(start, end)| egui::Rect::from_two_pos(start, normalize(end)));
        if response.drag_stopped() {
            if let Some(rect) = dragging.filter(|rect| rect.width() * source.width as f32 >= 1.0 && rect.height() * source.height as f32 >= 1.0) {
                self.options.focus = rect;
            }
            self.drag_start = None;
        }
        let focus = dragging.unwrap_or(self.options.focus);
        let project = |focus: egui::Rect| egui::Rect::from_min_max(rect.min + focus.min.to_vec2() * rect.size(), rect.min + focus.max.to_vec2() * rect.size());
        if self.options.validate().is_ok() {
            let (x, y, width, height) = self.options.crop(source);
            let converted = project(egui::Rect::from_min_max(
                egui::pos2(x as f32 / source.width as f32, y as f32 / source.height as f32),
                egui::pos2((x + width) as f32 / source.width as f32, (y + height) as f32 / source.height as f32),
            ));
            // Darkening what is left out makes the converted area readable at a glance.
            let shade = egui::Color32::from_black_alpha(150);
            for part in [
                egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), converted.top())),
                egui::Rect::from_min_max(egui::pos2(rect.left(), converted.bottom()), rect.max),
                egui::Rect::from_min_max(egui::pos2(rect.left(), converted.top()), converted.left_bottom()),
                egui::Rect::from_min_max(converted.right_top(), egui::pos2(rect.right(), converted.bottom())),
            ] {
                if part.is_positive() {
                    ui.painter().rect_filled(part, 0, shade);
                }
            }
            ui.painter()
                .rect_stroke(converted, 0, egui::Stroke::new(2.0, appearance::PRIMARY), egui::StrokeKind::Inside);
        }
        if dragging.is_some() || self.options.focus != Options::default().focus {
            ui.painter()
                .rect_stroke(project(focus), 0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
        }
        if self.ai_job.is_none() {
            response.on_hover_cursor(egui::CursorIcon::Crosshair);
        }
    }
}

fn scrolling(ui: &mut egui::Ui, id: &str, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .max_height(height)
        .show(ui, |ui| {
            // Keeps the controls clear of the floating scroll bar.
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    right: 12,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    add(ui);
                });
        });
}

fn pane_header(ui: &mut egui::Ui, rect: egui::Rect, title: &str, meta: &str, tooltip: Option<&str>, add_right: impl FnOnce(&mut egui::Ui)) {
    let header = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), PANE_HEADER - 6.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(header).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let title = appearance::bold(&ui, title);
    ui.label(title);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        add_right(ui);
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let label = ui.add(egui::Label::new(egui::RichText::new(meta).weak()).truncate());
            if let Some(tooltip) = tooltip {
                label.on_hover_text(tooltip);
            }
        });
    });
}

fn canvas_rect(pane: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(pane.left(), pane.top() + PANE_HEADER), pane.max)
}

fn canvas_fill(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_gray(16)
    } else {
        egui::Color32::from_gray(228)
    }
}

fn canvas_stroke(ui: &egui::Ui) -> egui::Stroke {
    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
}

fn paint_canvas(ui: &egui::Ui, rect: egui::Rect, stroke: egui::Stroke) {
    ui.painter().rect(rect, 8, canvas_fill(ui), stroke, egui::StrokeKind::Inside);
}

fn paint_preview(ui: &egui::Ui, texture: &egui::TextureHandle, area: egui::Rect, tint: egui::Color32) {
    let size = texture.size_vec2();
    let scale = (area.width() / size.x).min(area.height() / size.y);
    // Whole steps keep enlarged character cells crisp.
    let scale = if scale >= 1.0 { scale.floor() } else { scale.max(0.01) };
    let rect = egui::Rect::from_center_size(area.center(), size * scale);
    ui.painter()
        .image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), tint);
}

/// A spinner and caption over the dimmed previous preview, if there is one.
fn busy_indicator(ui: &egui::Ui, area: egui::Rect, text: &str, previous: Option<&egui::TextureHandle>) {
    if let Some(previous) = previous {
        paint_preview(ui, previous, area, egui::Color32::WHITE.gamma_multiply(0.3));
    }
    let center = area.center();
    egui::Spinner::new()
        .size(24.0)
        .paint_at(ui, egui::Rect::from_center_size(center - egui::vec2(0.0, 14.0), egui::Vec2::splat(24.0)));
    centered_text(ui, center + egui::vec2(0.0, 18.0), text, 13.0, ui.visuals().text_color(), area.width() - 16.0);
}

fn centered_text(ui: &egui::Ui, center: egui::Pos2, text: &str, size: f32, color: egui::Color32, wrap_width: f32) {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), egui::FontId::proportional(size), color, wrap_width.max(40.0));
    job.halign = egui::Align::Center;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(egui::pos2(center.x, center.y - galley.size().y / 2.0), galley, color);
}

/// Framed landscape pictogram for the empty source pane.
fn paint_picture_icon(painter: &egui::Painter, center: egui::Pos2, color: egui::Color32) {
    let frame = egui::Rect::from_center_size(center, egui::vec2(46.0, 36.0));
    let stroke = egui::Stroke::new(1.8, color);
    painter.rect_stroke(frame, 5, stroke, egui::StrokeKind::Inside);
    let at = |x: f32, y: f32| frame.min + egui::vec2(x, y) * frame.size();
    painter.circle_stroke(at(0.7, 0.32), 3.5, stroke);
    painter.add(egui::Shape::line(
        vec![at(0.1, 0.84), at(0.38, 0.5), at(0.58, 0.72), at(0.72, 0.58), at(0.9, 0.84)],
        stroke,
    ));
}

fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak().size(12.0));
}

/// Caption with a check box at the end of the row; clicking the caption toggles it as well.
fn toggle_row(ui: &mut egui::Ui, label: &str, value: &mut bool) -> egui::Response {
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 18.0 - ui.spacing().item_spacing.x).max(0.0);
        let caption = ui
            .allocate_ui_with_layout(egui::vec2(width, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(width);
                ui.add(egui::Label::new(label).wrap().sense(egui::Sense::click()))
            })
            .inner;
        let check = appearance::check(ui, value);
        let mut response = check | caption.clone();
        if caption.clicked() {
            *value = !*value;
            response.mark_changed();
        }
        response
    })
    .inner
}

/// Caption and value above a full-width slider, which suits the narrow settings column.
fn slider_field(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, scale: f64) {
    let mut shown = (*value * scale) as f32;
    let before = shown;
    let range = (*range.start() * scale) as f32..=(*range.end() * scale) as f32;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let speed = f64::from(range.end() - range.start()) / 200.0;
            ui.add_sized([56.0, 22.0], egui::DragValue::new(&mut shown).range(range.clone()).max_decimals(2).speed(speed));
        });
    });
    let width = ui.available_width();
    appearance::slider(ui, &mut shown, range, width);
    if shown != before {
        *value = f64::from(shown) / scale;
    }
}

fn fit_preview(size: egui::Vec2, available: egui::Vec2) -> egui::Vec2 {
    size * (available.x / size.x).min(available.y / size.y).max(0.01)
}

#[cfg(test)]
mod tests {
    use super::super::{
        tests::{click_text, frame, use_english, Gpu},
        Dialog, DrawApp, FileAction, NewKind, Picked,
    };
    use super::*;
    use icy_engine::{Position, Size, TextPane};
    use std::time::{Duration, Instant};

    fn source() -> ReferenceImage {
        let image = image::RgbaImage::from_fn(80, 100, |x, y| {
            let face = (i64::from(x) - 40).pow(2) * 2 + (i64::from(y) - 45).pow(2) < 1100;
            image::Rgba(if face {
                [(130 + x) as u8, (70 + y) as u8, 70, 255]
            } else {
                [10, 20, 65, 255]
            })
        });
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png).unwrap();
        image_attachment::prepare("portrait.png", &png.into_inner()).unwrap()
    }

    fn ready_dialog() -> ImportDialog {
        let source = source();
        let options = Options {
            columns: 8,
            rows: 5,
            ..Options::default()
        };
        let converted = convert(&source, &options).unwrap();
        ImportDialog {
            source: Some(source),
            options,
            result: Some(Arc::new(converted)),
            ..ImportDialog::default()
        }
    }

    fn wait(app: &mut DrawApp, context: &egui::Context, size: egui::Vec2) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            frame(context, app, size, vec![]);
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog closed")
            };
            if dialog.worker.is_none() {
                break;
            }
            assert!(Instant::now() < deadline, "image worker did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
        for _ in 0..3 {
            frame(context, app, size, vec![]);
        }
    }

    /// Runs frames until the debounced local preview has finished.
    fn wait_for_preview(app: &mut DrawApp, context: &egui::Context, size: egui::Vec2) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            frame(context, app, size, vec![]);
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog closed")
            };
            if dialog.result.is_some() || dialog.error.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "preview did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn image_import_geometry_matches_fit_focus_and_display_aspect() {
        use_english();
        let source = source();
        let mut options = Options {
            columns: 10,
            rows: 10,
            ..Options::default()
        };
        assert_eq!(options.crop(&source), (15, 0, 50, 100));
        options.focus = egui::Rect::from_min_max(egui::pos2(0.25, 0.2), egui::pos2(0.75, 0.8));
        assert_eq!(options.crop(&source), (25, 20, 30, 60));
        for fit in [Fit::Contain, Fit::Stretch] {
            options.fit = fit;
            assert_eq!(options.crop(&source), (20, 20, 40, 60));
        }
        options.fit = Fit::Crop;
        options.spacing = true;
        assert_eq!(options.crop(&source).2, 34);
        options.aspect = true;
        assert!(options.crop(&source).2 < 34);
        options.focus = egui::Rect::from_min_max(egui::pos2(-0.1, 0.0), egui::pos2(1.0, 1.0));
        assert!(options.validate().is_err());
        options.focus = Options::default().focus;
        options.columns = 160;
        options.rows = 200;
        assert!(options.validate().is_err());
    }

    #[test]
    fn image_import_presets_generate_legal_editable_ansi_in_each_fit() {
        use_english();
        for preset in [Preset::Scene, Preset::Shaded] {
            for fit in [Fit::Crop, Fit::Contain, Fit::Stretch] {
                let mut options = Options {
                    columns: 8,
                    rows: 5,
                    fit,
                    ..Options::default()
                };
                options.select_preset(preset);
                let converted = convert(&source(), &options).unwrap();
                assert_eq!(converted.ansi_buffer().size(), Size::new(8, 5));
                assert_eq!(converted.ansi_buffer().buffer_type, icy_engine::BufferType::CP437);
                assert_eq!(converted.preview.size, [64, 80]);
                assert!(!converted.ansi_buffer().terminal_state.is_terminal_buffer);
                assert!(converted.preview.pixels.iter().any(|pixel| *pixel != egui::Color32::BLACK));
                for row in 0..5 {
                    for column in 0..8 {
                        let cell = converted.ansi_buffer().char_at(Position::new(column, row));
                        assert!(cell.attribute.foreground() < 16 && cell.attribute.background() < 8);
                        assert!(!cell.attribute.is_blinking());
                    }
                }
            }
        }
        let mut options = Options::default();
        options.select_preset(Preset::Shaded);
        assert!(options.hue_families);
        assert_eq!(options.lightness_levels, 8);
        options.select_preset(Preset::Scene);
        assert!(options.hue_families);
    }

    #[test]
    #[ignore = "requires ICY_IMPORT_REFERENCE and ICY_IMPORT_OUTPUT for local visual evaluation"]
    fn image_import_reference_previews() {
        let input = std::env::var("ICY_IMPORT_REFERENCE").expect("path to a local reference picture");
        let output = std::path::PathBuf::from(std::env::var("ICY_IMPORT_OUTPUT").expect("preview output directory"));
        std::fs::create_dir_all(&output).unwrap();
        let source = image_attachment::read_drop(egui::DroppedFile {
            path: Some(input.into()),
            ..Default::default()
        })
        .unwrap();
        for (name, preset, glyphs) in [
            ("scene-blocks", Preset::Scene, Glyphs::Blocks),
            ("shaded-blocks", Preset::Shaded, Glyphs::Blocks),
            ("shaded-full", Preset::Shaded, Glyphs::Full),
            ("half-blocks", Preset::Shaded, Glyphs::HalfBlocks),
            ("ascii", Preset::Shaded, Glyphs::Ascii),
        ] {
            let mut options = Options {
                rows: 25,
                fit: Fit::Stretch,
                ice: false,
                glyphs,
                ..Default::default()
            };
            options.select_preset(preset);
            options.hue_families = true;
            options.dither = glyphs == Glyphs::HalfBlocks;
            let result = convert(&source, &options).unwrap();
            let pixels: Vec<_> = result.preview.pixels.iter().flat_map(|pixel| pixel.to_array()).collect();
            image::save_buffer(
                output.join(format!("{name}.png")),
                &pixels,
                result.preview.size[0] as u32,
                result.preview.size[1] as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            let bytes = icy_engine::formats::FileFormat::Ansi
                .to_bytes(result.ansi_buffer(), &icy_engine::formats::SaveOptions::default())
                .unwrap();
            std::fs::write(output.join(format!("{name}.ans")), bytes).unwrap();
            if glyphs == Glyphs::HalfBlocks {
                if let Some(path) = std::env::var_os("ICY_IMPORT_HALF_REFERENCE") {
                    let expected = icy_engine::formats::FileFormat::Ansi.load(Path::new(&path), None).unwrap().screen.buffer;
                    assert_eq!(expected.width(), options.columns);
                    let (size, expected) = expected.render_to_rgba(&Rectangle::from(0, 0, options.columns, options.rows).into(), false);
                    assert_eq!(result.preview.size, [size.width as usize, size.height as usize]);
                    let squared: f64 = pixels
                        .chunks_exact(4)
                        .zip(expected.chunks_exact(4))
                        .flat_map(|(a, b)| (0..3).map(move |c| (f64::from(a[c]) - f64::from(b[c])).powi(2)))
                        .sum();
                    let rmse = (squared / (result.preview.pixels.len() * 3) as f64).sqrt() / 255.0;
                    eprintln!("Half-pixel reference RGB RMSE: {rmse:.6}");
                    assert!(rmse < 0.08, "half-pixel reference deviation exceeds the portrait regression limit");
                }
            }
        }
    }

    #[test]
    fn image_import_glyph_controls_invalidate_preview_and_limit_output() {
        use_english();
        for glyphs in [Glyphs::HalfBlocks, Glyphs::Ascii] {
            let context = egui::Context::default();
            appearance::apply(&context);
            let size = egui::vec2(1280.0, 900.0);
            let mut app = DrawApp::new();
            app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
            for _ in 0..3 {
                frame(&context, &mut app, size, vec![]);
            }
            click_text(&context, &mut app, size, &glyphs.label());
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog")
            };
            assert_eq!(dialog.options.glyphs, glyphs);
            assert_eq!(dialog.options.dither, glyphs == Glyphs::HalfBlocks);
            assert!(dialog.result.is_none());
            wait_for_preview(&mut app, &context, size);
            let Some(Dialog::AiImport(dialog)) = &app.dialog else {
                panic!("import dialog")
            };
            let result = dialog.result.as_ref().expect("glyph-constrained conversion");
            let mut halves = 0;
            let mut characters = 0;
            for y in 0..5 {
                for x in 0..8 {
                    let cell = result.ansi_buffer().char_at(Position::new(x, y));
                    if glyphs == Glyphs::HalfBlocks {
                        assert!([32, 219, 220, 223].contains(&(cell.ch as u32)));
                        halves += usize::from(matches!(cell.ch as u32, 220 | 223) && cell.attribute.foreground() != cell.attribute.background());
                    } else {
                        assert!((32..=126).contains(&(cell.ch as u32)));
                        characters += usize::from(cell.ch != ' ' && cell.attribute.foreground() != cell.attribute.background());
                    }
                    assert!(cell.attribute.foreground() < 16 && cell.attribute.background() < 8);
                    assert!(!cell.attribute.is_blinking());
                }
            }
            if glyphs == Glyphs::HalfBlocks {
                assert!(halves > 0, "must actually draw independent upper/lower colors, not just solids");
            } else {
                assert!(characters > 0, "must draw visible ASCII characters, not just background colors");
            }
        }
    }

    #[test]
    fn image_import_standard_sizes_produce_complete_previews() {
        let source = source();
        for (rows, preset) in [(25, Preset::Scene), (50, Preset::Shaded)] {
            let mut options = Options { rows, ..Default::default() };
            options.select_preset(preset);
            let result = convert(&source, &options).unwrap();
            assert_eq!(result.ansi_buffer().size(), Size::new(80, rows));
            assert_eq!(result.preview.size, [640, rows as usize * 16]);
            assert_eq!(result.preview.pixels.len(), 640 * rows as usize * 16);
        }
    }

    #[test]
    fn image_import_rip_uses_fixed_canvas_and_horizontal_palette_runs() {
        let source = source();
        let options = Options {
            target: Target::Rip,
            fit: Fit::Stretch,
            ..Default::default()
        };
        let result = convert(&source, &options).unwrap();
        assert_eq!(result.preview.size, [640, 350]);
        let Imported::Rip(commands) = result.imported else {
            panic!("expected RIP import")
        };
        assert!(!commands.is_empty());
        assert!(commands.iter().all(|command| match command {
            RipCommand::SetPalette { colors } => colors.len() == 16 && colors.iter().all(|color| *color < 64),
            RipCommand::Color { c } => *c < 16,
            RipCommand::Line { x0, y0, x1, y1 } => x0 <= x1 && *x1 < 640 && y0 == y1 && *y1 < 350,
            _ => false,
        }));
        let mut document = RipDocument::new();
        document.replace_editable(commands).unwrap();
        assert_eq!(document.preview().unwrap().rgba().len(), 640 * 350 * 4);
    }

    #[test]
    fn image_import_rip_adapts_palette_to_warm_photographic_colors() {
        let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([200, 150, 120, 255]));
        let palette = rip_palette(&image);
        let rgb: Vec<[f32; 3]> = palette
            .iter()
            .map(|index| {
                let (r, g, b) = EGA_PALETTE[*index as usize].rgb();
                [f32::from(r), f32::from(g), f32::from(b)]
            })
            .collect();
        let indices = quantize(&image, &rgb, false);
        let (red, green, blue) = EGA_PALETTE[palette[indices[0] as usize] as usize].rgb();
        assert!(red != green || green != blue, "warm source color must not collapse to gray");
    }

    #[test]
    fn image_import_igs_uses_low_resolution_adaptive_pens_and_line_runs() {
        let source = source();
        let options = Options {
            target: Target::Igs,
            fit: Fit::Stretch,
            ..Default::default()
        };
        let result = convert(&source, &options).unwrap();
        assert_eq!(result.preview.size, [320, 200]);
        let Imported::Igs(commands) = result.imported else {
            panic!("expected IGS import")
        };
        assert!(matches!(
            commands[0],
            IgsCommand::SetResolution {
                resolution: TerminalResolution::Low,
                ..
            }
        ));
        let pens: Vec<_> = commands
            .iter()
            .filter_map(|command| match command {
                IgsCommand::SetPenColor { pen, red, green, blue } => Some((*pen, [*red, *green, *blue])),
                _ => None,
            })
            .collect();
        assert_eq!(pens.len(), 16);
        assert!(pens.iter().all(|(pen, rgb)| *pen < 16 && rgb.iter().all(|level| *level < 8)));
        assert!(
            pens.iter().any(|(_, [red, green, blue])| red != green || green != blue),
            "the warm portrait must keep chromatic pens"
        );
        let mut colors = Vec::new();
        for command in &commands {
            match command {
                IgsCommand::ColorSet { pen: PenType::Line, color } => {
                    assert!(*color > 0 && *color < 16, "pen 0 is the cleared background");
                    colors.push(*color);
                }
                IgsCommand::Line { x1, y1, x2, y2 } => {
                    let (IgsParameter::Value(x1), IgsParameter::Value(y1), IgsParameter::Value(x2), IgsParameter::Value(y2)) = (x1, y1, x2, y2) else {
                        panic!("literal coordinates expected")
                    };
                    assert!(x1 <= x2 && *x2 < 320 && y1 == y2 && *y2 < 200);
                }
                IgsCommand::SetResolution { .. } | IgsCommand::SetPenColor { .. } | IgsCommand::ScreenClear { .. } => {}
                other => panic!("unexpected command {other:?}"),
            }
        }
        let mut unique = colors.clone();
        unique.dedup();
        assert_eq!(unique, colors, "each pen is selected once");
        let mut document = IgsDocument::new(TerminalResolution::Low);
        document.replace_items(commands.into_iter().map(IgsItem::from).collect()).unwrap();
        assert_eq!(document.resolution(), TerminalResolution::Low);
    }

    #[test]
    fn image_import_petscii_and_vt52_use_native_screens() {
        let source = source();
        for (target, buffer_type) in [
            (Target::Petscii, icy_engine::BufferType::Petscii),
            (Target::Vt52, icy_engine::BufferType::AtariSt),
        ] {
            let options = Options {
                target,
                fit: Fit::Stretch,
                ..Default::default()
            };
            let result = convert(&source, &options).unwrap();
            let buffer = match result.imported {
                Imported::Petscii(buffer) if target == Target::Petscii => buffer,
                Imported::Vt52(buffer) if target == Target::Vt52 => buffer,
                _ => panic!("unexpected import for {target:?}"),
            };
            let expected = target.retro_buffer().unwrap();
            assert_eq!(buffer.buffer_type, buffer_type);
            assert_eq!(buffer.size(), expected.size());
            let font = buffer.font_dimensions();
            assert_eq!(
                result.preview.size,
                [(buffer.width() * font.width) as usize, (buffer.height() * font.height) as usize]
            );
            let mut colors = std::collections::HashSet::new();
            for y in 0..buffer.height() {
                for x in 0..buffer.width() {
                    let cell = buffer.char_at(Position::new(x, y));
                    colors.insert(cell.attribute.foreground());
                    colors.insert(cell.attribute.background());
                }
            }
            assert!(colors.len() > 1, "{target:?} must not be a blank screen");
        }
    }

    #[test]
    fn image_import_offers_ai_authoring_only_for_character_targets_with_copilot() {
        let source = source();
        for target in Target::ALL {
            let options = Options {
                target,
                fit: Fit::Stretch,
                ..Default::default()
            };
            let result = convert(&source, &options).unwrap();
            for ai_available in [false, true] {
                let mut dialog = ImportDialog {
                    source: Some(source.clone()),
                    options: options.clone(),
                    result: Some(Arc::new(Converted {
                        imported: result.imported.clone(),
                        preview: result.preview.clone(),
                    })),
                    ai_available,
                    ai_unavailable_reason: (!ai_available).then(|| fl!("ai-chat-copilot-login")),
                    ..Default::default()
                };
                match dialog.generate() {
                    Some(Action::Generate(request)) => {
                        assert!(ai_available && target.refinable(), "{target:?}, AI available: {ai_available}");
                        assert_eq!(request.format, target.label());
                        assert!(request.image.width > 0 && request.image.height > 0);
                        assert!(super::super::ai_chat::canvas::Draft::new(request.format, request.buffer, 0, None)
                            .changes()
                            .is_empty());
                        assert!(dialog.accepted().is_none(), "the local preview must not stand in for the AI drawing");
                    }
                    None => {
                        assert!(!ai_available || !target.refinable(), "{target:?}, AI available: {ai_available}");
                        assert_eq!(dialog.ai_error.is_some(), target.refinable(), "{target:?}: the reason must be shown");
                        assert!(dialog.accepted().is_some(), "an unavailable AI must keep the local preview");
                    }
                    Some(_) => panic!("unexpected action"),
                }
            }
        }
        assert!(!Target::Rip.refinable() && !Target::Igs.refinable());
    }

    #[test]
    fn image_import_ai_proposal_is_previewed_and_accepted_exactly_before_installation() {
        use super::super::ai_chat::{canvas::Draft, ImportJob, ImportResponse as Response, ImportWorkspace as Workspace};
        let mut dialog = ImportDialog {
            source: Some(source()),
            options: Options {
                target: Target::Petscii,
                ..Default::default()
            },
            ai_available: true,
            ..Default::default()
        };
        let request = dialog.ai_request().unwrap();
        let mut draft = Draft::new(request.format, request.buffer, 0, None);
        draft.image_authoring = true;
        draft
            .call("icy_set_cells", &serde_json::json!({"cells": [{"x": 10, "y": 8, "char_code": 65, "fg": 2}]}))
            .unwrap();
        let expected_cell = draft.buffer.char_at(Position::new(10, 8));
        let expected = dialog.converted_buffer(draft.buffer.clone()).unwrap();
        let (job, sender) = ImportJob::pending();
        dialog.begin_ai(Ok(job));
        let clone = dialog.clone();
        drop(clone);
        assert!(dialog.accepted().is_none());
        sender
            .send(Ok(Response::Proposal("drawn".into(), Box::new(Workspace::Canvas(Box::new(draft))))))
            .unwrap();
        dialog.poll();
        assert!(dialog.ai_job.is_none() && dialog.error.is_none() && dialog.ai_error.is_none());
        assert!(dialog.ai_result);
        assert_eq!(dialog.result.as_ref().unwrap().preview, expected.preview);
        let Imported::Petscii(buffer) = dialog.accepted().unwrap().imported else {
            panic!("wrong target")
        };
        assert_eq!(buffer.char_at(Position::new(10, 8)), expected_cell);
    }

    #[test]
    fn image_import_ai_cancellation_and_failures_never_accept_a_local_fallback() {
        use super::super::ai_chat::{ImportJob, ImportResponse as Response};
        let mut dialog = ImportDialog::default();
        for response in [Ok(Response::Reply("advice".into())), Err("model failed".into())] {
            let (job, sender) = ImportJob::pending();
            dialog.begin_ai(Ok(job));
            sender.send(response).unwrap();
            dialog.poll();
            assert!(dialog.ai_error.is_some() && !dialog.ai_result);
            assert!(dialog.accepted().is_none());
        }
        let (job, sender) = ImportJob::pending();
        dialog.begin_ai(Ok(job));
        let clone = dialog.clone();
        dialog.invalidate();
        assert!(clone.ai_job.unwrap().lock().is_none());
        assert!(sender.send(Ok(Response::Reply("stale".into()))).is_err());
        assert!(dialog.result.is_none() && dialog.ai_job.is_none());
    }

    #[test]
    fn image_import_losing_the_connection_cancels_the_ai_drawing_and_reports_why() {
        let mut dialog = ready_dialog();
        dialog.set_ai_availability(Ok(()));
        assert!(matches!(dialog.generate(), Some(Action::Generate(_))));
        let (job, sender) = super::super::ai_chat::ImportJob::pending();
        dialog.begin_ai(Ok(job));
        dialog.set_ai_availability(Err(fl!("ai-chat-copilot-login")));
        assert!(!dialog.ai_available);
        assert!(dialog.ai_job.is_none() && dialog.accepted().is_none());
        assert_eq!(dialog.ai_error.as_deref(), Some(fl!("ai-chat-copilot-login").as_str()));
        assert!(dialog.error.is_none(), "the local preview must resume");
        assert!(sender.send(Err("stale".into())).is_err());
        dialog.set_ai_availability(Ok(()));
        assert!(dialog.ai_available && dialog.ai_job.is_none(), "reconnection must not start a drawing");
    }

    #[test]
    fn image_import_focus_drag_and_overlay_follow_the_image_not_the_column() {
        use_english();
        let context = egui::Context::default();
        let mut dialog = ImportDialog {
            source: Some(source()),
            ..Default::default()
        };
        let render = |dialog: &mut ImportDialog, events| {
            context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        ui.columns(2, |columns| dialog.source_preview(&mut columns[0], 450.0));
                    });
                },
            )
        };
        let output = render(&mut dialog, vec![]);
        let texture = dialog.source_texture.as_ref().unwrap().id();
        let image = context
            .tessellate(output.shapes.clone(), output.pixels_per_point)
            .iter()
            .find_map(|primitive| match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture => Some(mesh.calc_bounds()),
                _ => None,
            })
            .unwrap();
        let overlay = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.stroke.color == appearance::PRIMARY => Some(rect.rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(overlay.size(), egui::vec2(360.0, 450.0));
        // Tessellation adds a subpixel antialiasing fringe around the image.
        assert!((overlay.min - image.min).length() <= 1.0 && (overlay.max - image.max).length() <= 1.0);
        let start = overlay.min + overlay.size() * 0.25;
        let end = overlay.min + overlay.size() * 0.75;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(&mut dialog, vec![egui::Event::PointerMoved(start)]);
        render(&mut dialog, vec![button(start, true)]);
        render(&mut dialog, vec![egui::Event::PointerMoved(end)]);
        render(&mut dialog, vec![button(end, false)]);
        assert_eq!(dialog.options.focus, egui::Rect::from_min_max(egui::pos2(0.25, 0.25), egui::pos2(0.75, 0.75)));
    }

    #[test]
    fn image_import_welcome_new_and_file_menu_open_the_dialog() {
        use_english();
        for entry in ["welcome", "new", "file"] {
            let context = egui::Context::default();
            appearance::apply(&context);
            let mut app = DrawApp::new();
            let size = egui::vec2(1280.0, 900.0);
            match entry {
                "welcome" => app.show_start = true,
                "new" => app.request_new(),
                _ => {}
            }
            frame(&context, &mut app, size, vec![]);
            if entry == "file" {
                click_text(&context, &mut app, size, "File");
            }
            click_text(&context, &mut app, size, &fl!("menu-ai-import"));
            if entry == "new" {
                click_text(&context, &mut app, size, &fl!("new-file-create"));
            }
            assert!(matches!(app.dialog, Some(Dialog::AiImport(_))), "{entry}");
            assert!(!app.document.modified());
            for _ in 0..3 {
                frame(&context, &mut app, size, vec![]);
            }
            click_text(&context, &mut app, size, &labels::cancel());
            assert!(app.dialog.is_none(), "{entry}: cancel must close import");
            assert_eq!(app.show_start, entry == "welcome");
        }
    }

    #[test]
    fn image_import_picker_conversion_and_accept_switch_to_ansi() {
        use_english();
        let context = egui::Context::default();
        appearance::apply(&context);
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        app.open_ai_import();
        let size = egui::vec2(1280.0, 900.0);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.png");
        source().pixels().unwrap().save(&path).unwrap();
        app.picker = true;
        app.sender
            .send(Picked {
                action: FileAction::AiImport,
                path: Some(path),
            })
            .unwrap();
        wait(&mut app, &context, size);
        assert!(!app.picker);
        let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
        assert!(dialog.source.is_some());
        dialog.options.columns = 8;
        dialog.options.rows = 5;
        dialog.invalidate();
        wait_for_preview(&mut app, &context, size);
        let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
        assert!(dialog.error.is_none(), "{:?}", dialog.error);
        let expected = dialog.result.as_ref().unwrap().preview.clone();
        assert!(app.font_editor.is_some(), "conversion must not replace the live editor early");
        click_text(&context, &mut app, size, &fl!("ai-import-accept", format = "ANSI"));
        assert!(app.dialog.is_none() && app.font_editor.is_none());
        assert!(app.rip.is_none() && app.igs.is_none() && app.charfont.is_none() && app.animation.is_none());
        assert!(!app.show_start);
        assert!(app.document.path.is_none() && app.document.modified());
        app.document.with_state(|state| {
            let buffer = state.get_buffer();
            assert_eq!(buffer.size(), Size::new(8, 5));
            let (_, pixels) = buffer.render_to_rgba(&Rectangle::from(0, 0, 8, 5).into(), false);
            assert_eq!(egui::ColorImage::from_rgba_unmultiplied([64, 80], &pixels), expected);
        });
        app.document.type_text("X").unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'X');
    }

    #[test]
    fn image_import_accept_switches_to_editable_rip() {
        use_english();
        let context = egui::Context::default();
        appearance::apply(&context);
        let source = source();
        let options = Options {
            target: Target::Rip,
            fit: Fit::Stretch,
            ..Default::default()
        };
        let converted = convert(&source, &options).unwrap();
        let Imported::Rip(expected) = &converted.imported else {
            panic!("expected RIP import")
        };
        let expected = expected.clone();
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        app.dialog = Some(Dialog::AiImport(Box::new(ImportDialog {
            source: Some(source),
            options,
            result: Some(Arc::new(converted)),
            ..Default::default()
        })));
        let size = egui::vec2(1280.0, 900.0);
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, &fl!("ai-import-accept", format = "RIP"));
        assert!(app.dialog.is_none() && app.font_editor.is_none());
        assert!(app.igs.is_none() && app.charfont.is_none() && app.animation.is_none());
        let editor = app.rip.as_ref().expect("RIP editor");
        assert_eq!(editor.document.commands(), expected);
        assert!(editor.modified());
    }

    fn accept(target: Target) -> DrawApp {
        use_english();
        let context = egui::Context::default();
        appearance::apply(&context);
        let source = source();
        let options = Options {
            target,
            fit: Fit::Stretch,
            ..Default::default()
        };
        let converted = convert(&source, &options).unwrap();
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        app.dialog = Some(Dialog::AiImport(Box::new(ImportDialog {
            source: Some(source),
            options,
            result: Some(Arc::new(converted)),
            ..Default::default()
        })));
        let size = egui::vec2(1280.0, 900.0);
        for _ in 0..3 {
            frame(&context, &mut app, size, vec![]);
        }
        click_text(&context, &mut app, size, &fl!("ai-import-accept", format = target.label()));
        assert!(app.dialog.is_none() && app.font_editor.is_none(), "{target:?}");
        app
    }

    #[test]
    fn image_import_accept_switches_to_editable_igs() {
        let app = accept(Target::Igs);
        assert!(app.rip.is_none() && app.charfont.is_none() && app.animation.is_none());
        let editor = app.igs.as_ref().expect("IGS editor");
        assert_eq!(editor.document.resolution(), TerminalResolution::Low);
        assert!(editor.document.items().len() > 18);
        assert!(editor.document.modified());
    }

    #[test]
    fn image_import_accept_switches_to_native_petscii_and_vt52_editors() {
        let app = accept(Target::Petscii);
        assert!(app.petscii.is_some() && app.vt52.is_none() && app.rip.is_none() && app.igs.is_none());
        assert!(app.new_kind == NewKind::Petscii);
        assert!(app.document.modified());
        assert_eq!(app.document.with_state(|state| state.get_buffer().buffer_type), icy_engine::BufferType::Petscii);

        let app = accept(Target::Vt52);
        assert!(app.vt52.is_some() && app.petscii.is_none() && app.rip.is_none() && app.igs.is_none());
        assert!(app.new_kind == NewKind::Vt52);
        assert!(app.document.modified());
        assert_eq!(app.document.with_state(|state| state.get_buffer().buffer_type), icy_engine::BufferType::AtariSt);
    }

    #[test]
    fn image_import_setting_changes_and_errors_cannot_accept_stale_results() {
        use_english();
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
        let size = egui::vec2(1280.0, 900.0);
        frame(&context, &mut app, size, vec![]);
        click_text(&context, &mut app, size, &fl!("ai-import-shaded"));
        let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
        assert!(dialog.result.is_none());
        dialog.load(&context, Path::new("/nonexistent/icy-import-test.png"));
        wait(&mut app, &context, size);
        let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
        assert!(dialog.error.is_some() && dialog.result.is_none() && dialog.source.is_none());
        assert!(!app.document.modified());
    }

    fn shows_text(output: &egui::FullOutput, text: &str) -> bool {
        output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(shape) if shape.galley.text() == text))
    }

    #[test]
    fn image_import_offers_draw_with_ai_for_character_formats_without_changing_the_document() {
        use_english();
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.ai_chat = super::super::ai_chat::Chat::new(icy_draw::AiChatSettings {
            provider: icy_draw::AiProvider::Copilot,
            copilot_model: "vision-model".into(),
            ..Default::default()
        });
        app.ai_chat.finish_connection_for_test(Ok(super::super::ai_chat::ImportResponse::Models {
            models: vec!["vision-model".into()],
            account: Some("octocat".into()),
        }));
        let original = app.document.screen.clone();
        let size = egui::vec2(1280.0, 900.0);
        for target in Target::ALL {
            let mut dialog = ready_dialog();
            dialog.options.target = target;
            app.dialog = Some(Dialog::AiImport(Box::new(dialog)));
            frame(&context, &mut app, size, vec![]);
            let output = frame(&context, &mut app, size, vec![]);
            assert_eq!(shows_text(&output, &fl!("ai-import-generate")), target.refinable(), "{target:?}");
            let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
            assert!(dialog.ai_available && dialog.ai_job.is_none(), "showing the button must not start a request");
        }
        assert!(Arc::ptr_eq(&original, &app.document.screen));
    }

    #[test]
    fn image_import_draw_with_ai_reports_connection_errors_without_blocking_local_conversion() {
        use_english();
        for reason in [
            fl!("ai-chat-copilot-login"),
            fl!("ai-chat-copilot-missing"),
            "Copilot CLI: connection refused".into(),
        ] {
            let context = egui::Context::default();
            let mut app = DrawApp::new();
            app.ai_chat = super::super::ai_chat::Chat::new(icy_draw::AiChatSettings {
                provider: icy_draw::AiProvider::Copilot,
                copilot_model: "vision-model".into(),
                ..Default::default()
            });
            app.ai_chat.finish_connection_for_test(Err(reason.clone()));
            app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
            let size = egui::vec2(1280.0, 900.0);
            frame(&context, &mut app, size, vec![]);
            click_text(&context, &mut app, size, &fl!("ai-import-generate"));
            let output = frame(&context, &mut app, size, vec![]);
            assert!(shows_text(&output, &reason), "{reason} must be visible even when the chat is closed");
            let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
            assert!(!dialog.ai_available && dialog.ai_job.is_none());
            assert!(dialog.result.is_some(), "an unavailable AI must keep the local preview");
            click_text(&context, &mut app, size, &fl!("ai-import-shaded"));
            wait_for_preview(&mut app, &context, size);
            let Some(Dialog::AiImport(dialog)) = &mut app.dialog else { panic!("import") };
            assert!(dialog.result.is_some() && dialog.error.is_none() && dialog.ai_job.is_none());
            assert!(dialog.ai_error.is_none(), "changing a setting clears the old AI error");
        }
    }

    #[test]
    fn image_import_accept_protects_unsaved_work_on_cancel_discard_and_save() {
        use_english();
        for action in ["cancel", "discard", "save", "picker-cancel", "save-failed"] {
            let context = egui::Context::default();
            let mut app = DrawApp::new();
            app.document.type_text("KEEP").unwrap();
            let result = ready_dialog().result.unwrap().ansi_buffer().clone();
            app.import_ansi(result);
            assert!(matches!(app.dialog, Some(Dialog::Close)));
            assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
            let size = egui::vec2(1280.0, 900.0);
            match action {
                "cancel" => click_text(&context, &mut app, size, &labels::cancel()),
                "discard" => click_text(&context, &mut app, size, &fl!("ask_close_file_dialog-dont_save_button")),
                "picker-cancel" => {
                    app.continue_after_save = true;
                    app.picker = true;
                    app.sender
                        .send(Picked {
                            action: FileAction::Save,
                            path: None,
                        })
                        .unwrap();
                    frame(&context, &mut app, size, vec![]);
                }
                _ => {
                    let directory = tempfile::tempdir().unwrap();
                    let path = if action == "save" {
                        directory.path().join("old.icy")
                    } else {
                        directory.path().join("missing/old.icy")
                    };
                    app.continue_after_save = true;
                    app.save_path(&context, path.clone(), true);
                    if action == "save" {
                        let saved = icy_draw::document::Document::load(&path).unwrap();
                        assert_eq!(saved.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
                    }
                }
            }
            assert!(app.pending_import.is_none(), "{action}");
            let imported = matches!(action, "discard" | "save");
            assert_eq!(
                app.document.with_state(|state| state.get_buffer().size()) == Size::new(8, 5),
                imported,
                "{action}"
            );
            if !imported {
                assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'K');
            }
        }
    }

    #[test]
    fn image_import_live_preview_waits_for_settings_to_rest_and_discards_outdated_conversions() {
        let context = egui::Context::default();
        let run = |dialog: &mut ImportDialog, time: f64, events: Vec<egui::Event>| {
            let _ = context.run(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |context| dialog.update_preview(context),
            );
        };
        let mut dialog = ImportDialog {
            source: Some(source()),
            options: Options {
                columns: 8,
                rows: 5,
                ..Default::default()
            },
            changed_at: Some(1.0),
            ..Default::default()
        };
        run(&mut dialog, 1.0 + PREVIEW_DELAY / 2.0, vec![]);
        assert!(dialog.worker.is_none(), "settings that are still changing must not convert");
        run(&mut dialog, 1.0 + PREVIEW_DELAY, vec![]);
        assert!(dialog.converting && dialog.worker.is_some() && dialog.changed_at.is_none());
        dialog.options.rows = 6;
        dialog.invalidate();
        assert!(
            dialog.worker.is_none() && !dialog.converting,
            "a conversion of outdated settings must not arrive"
        );
        dialog.ai_available = true;
        dialog.ai_job = Some(Arc::new(Mutex::new(None)));
        run(&mut dialog, 5.0, vec![]);
        assert!(dialog.worker.is_none(), "an AI drawing in progress must not be replaced by a local preview");
        dialog.ai_job = None;
        dialog.load(&context, Path::new("/nonexistent/icy-import-test.png"));
        dialog.invalidate();
        assert!(dialog.worker.is_some(), "setting changes must not abort loading the source");
    }

    #[test]
    fn image_import_loads_images_dropped_on_the_dialog() {
        use_english();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dropped.png");
        source().pixels().unwrap().save(&path).unwrap();
        let context = egui::Context::default();
        let mut app = DrawApp::new();
        app.dialog = Some(Dialog::AiImport(Box::default()));
        let size = egui::vec2(1280.0, 900.0);
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                dropped_files: vec![egui::DroppedFile {
                    path: Some(path),
                    ..Default::default()
                }],
                ..Default::default()
            },
            |context| app.show(context),
        );
        wait(&mut app, &context, size);
        let Some(Dialog::AiImport(dialog)) = &app.dialog else { panic!("import") };
        assert_eq!(dialog.source.as_ref().map(|source| source.name.as_str()), Some("dropped.png"));
        assert!(app.document.path.is_none(), "a dropped image must not be opened as a document");
    }

    #[test]
    #[ignore = "requires a graphics adapter"]
    fn gpu_image_import_dialog_previews_at_wide_and_narrow_sizes() {
        use_english();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut gpu = Gpu::new().await;
            let mut app = DrawApp::new();
            for size in [[1280, 900], [440, 700]] {
                app.dialog = Some(Dialog::AiImport(Box::default()));
                for _ in 0..3 {
                    gpu.capture(&mut app, size, 1.0, vec![], "ai-import-warmup");
                }
                let empty = gpu.capture(&mut app, size, 1.0, vec![], &format!("ai-import-empty-{}", size[0]));
                app.dialog = Some(Dialog::AiImport(Box::new(ready_dialog())));
                for _ in 0..3 {
                    gpu.capture(&mut app, size, 1.0, vec![], "ai-import-warmup");
                }
                let rendered = gpu.capture(&mut app, size, 1.0, vec![], &format!("ai-import-preview-{}", size[0]));
                assert!(empty.chunks_exact(4).zip(rendered.chunks_exact(4)).filter(|(a, b)| a != b).count() > 500);
            }
        });
    }
}
