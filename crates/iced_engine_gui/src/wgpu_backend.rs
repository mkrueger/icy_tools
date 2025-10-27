use std::collections::HashMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::mem::size_of;
use std::num::NonZeroU64;

use bitvec::order::Lsb0;
use bitvec::slice::BitSlice;
use bitvec::vec::BitVec;
use indexmap::IndexMap;
use raqote::DrawOptions;
use raqote::DrawTarget;
use raqote::SolidSource;
use raqote::StrokeStyle;
use raqote::Transform;
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::WindowSize;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use rustybuzz::shape_with_plan;
use rustybuzz::ttf_parser::GlyphId;
use rustybuzz::ttf_parser::RasterGlyphImage;
use rustybuzz::ttf_parser::RasterImageFormat;
use rustybuzz::ttf_parser::RgbaColor;
use rustybuzz::GlyphBuffer;
use rustybuzz::UnicodeBuffer;
use unicode_bidi::Level;
use unicode_bidi::ParagraphBidiInfo;
use unicode_properties::GeneralCategoryGroup;
use unicode_properties::UnicodeEmoji;
use unicode_properties::UnicodeGeneralCategory;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use web_time::Duration;
use web_time::Instant;
use wgpu::util::BufferInitDescriptor;
use wgpu::util::DeviceExt;
use wgpu::Buffer;
use wgpu::BufferUsages;
use wgpu::CommandEncoderDescriptor;
use wgpu::Device;
use wgpu::Extent3d;
use wgpu::IndexFormat;
use wgpu::LoadOp;
use wgpu::Operations;
use wgpu::Origin3d;
use wgpu::Queue;
use wgpu::RenderPassColorAttachment;
use wgpu::RenderPassDescriptor;
use wgpu::StoreOp;
use wgpu::Surface;
use wgpu::SurfaceConfiguration;
use wgpu::Texture;
use wgpu::TextureAspect;

use crate::backend::build_wgpu_state;
use crate::backend::c2c;
use crate::backend::private::Token;
use crate::backend::PostProcessor;
use crate::backend::RenderSurface;
use crate::backend::RenderTexture;
use crate::backend::TextBgVertexMember;
use crate::backend::TextCacheBgPipeline;
use crate::backend::TextCacheFgPipeline;
use crate::backend::TextVertexMember;
use crate::backend::Viewport;
use crate::backend::WgpuState;
use crate::colors::Rgb;
use crate::fonts::Font;
use crate::fonts::Fonts;
use crate::shaders::DefaultPostProcessor;
use crate::utils::plan_cache::PlanCache;
use crate::utils::text_atlas::Atlas;
use crate::utils::text_atlas::CacheRect;
use crate::utils::text_atlas::Entry;
use crate::utils::text_atlas::Key;
use crate::utils::Outline;
use crate::utils::Painter;
use crate::RandomState;

const NULL_CELL: Cell = Cell::new("");

pub(super) struct RenderInfo {
    cell: usize,
    cached: CacheRect,
    underline_pos_min: u16,
    underline_pos_max: u16,
}
/// Map from (x, y, glyph) -> (cell index, cache entry).
/// We use an IndexMap because we want a consistent rendering order for
/// vertices.
type Rendered = IndexMap<(i32, i32, GlyphId), RenderInfo, RandomState>;

/// Set of (x, y, glyph, char width).
type Sourced = HashSet<(i32, i32, GlyphId, u32), RandomState>;

/// A ratatui backend leveraging wgpu for rendering.
///
/// Constructed using a [`Builder`](crate::Builder).
///
/// The first lifetime parameter is the lifetime of the data for referenced
/// [`Font`] objects. The second lifetime parameter is the lifetime of the
/// referenced [`Surface`] (typically the lifetime of your window object).
///
/// Limitations:
/// - The cursor is tracked but not rendered.
/// - No builtin accessibilty, although [`WgpuBackend::get_text`] is provided to
///   access the screen's contents.
pub struct WgpuBackend<
    'f,
    's,
    P: PostProcessor = DefaultPostProcessor,
    S: RenderSurface<'s> = Surface<'s>,
> {
    pub(super) post_process: P,

    pub(super) cells: Vec<Cell>,
    pub(super) dirty_rows: Vec<bool>,
    pub(super) dirty_cells: BitVec,
    pub(super) rendered: Vec<Rendered>,
    pub(super) sourced: Vec<Sourced>,
    pub(super) fast_blinking: BitVec,
    pub(super) slow_blinking: BitVec,

    pub(super) cursor: (u16, u16),

    pub(super) viewport: Viewport,

    pub(super) surface: S,
    pub(super) _surface: PhantomData<&'s S>,
    pub(super) surface_config: SurfaceConfiguration,
    pub(super) device: Device,
    pub(super) queue: Queue,

    pub(super) plan_cache: PlanCache,
    pub(super) buffer: UnicodeBuffer,
    pub(super) row: String,
    pub(super) rowmap: Vec<u16>,

    pub(super) cached: Atlas,
    pub(super) text_cache: Texture,
    pub(super) text_mask: Texture,
    pub(super) bg_vertices: Vec<TextBgVertexMember>,
    pub(super) text_indices: Vec<[u32; 6]>,
    pub(super) text_vertices: Vec<TextVertexMember>,
    pub(super) text_bg_compositor: TextCacheBgPipeline,
    pub(super) text_fg_compositor: TextCacheFgPipeline,
    pub(super) text_screen_size_buffer: Buffer,

    pub(super) wgpu_state: WgpuState,

    pub(super) fonts: Fonts<'f>,
    pub(super) reset_fg: Rgb,
    pub(super) reset_bg: Rgb,

    pub(super) fast_duration: Duration,
    pub(super) last_fast_toggle: Instant,
    pub(super) show_fast: bool,
    pub(super) slow_duration: Duration,
    pub(super) last_slow_toggle: Instant,
    pub(super) show_slow: bool,
}

impl<'f, 's, P: PostProcessor, S: RenderSurface<'s>> WgpuBackend<'f, 's, P, S> {
    /// Get the [`PostProcessor`] associated with this backend.
    pub fn post_processor(&self) -> &P {
        &self.post_process
    }

    /// Get a mutable reference to the [`PostProcessor`] associated with this
    /// backend.
    pub fn post_processor_mut(&mut self) -> &mut P {
        &mut self.post_process
    }

    /// Resize the rendering surface. This should be called e.g. to keep the
    /// backend in sync with your window size.
    pub fn resize(&mut self, width: u32, height: u32) {
        let limits = self.device.limits();
        let width = width.min(limits.max_texture_dimension_2d);
        let height = height.min(limits.max_texture_dimension_2d);

        if width == self.surface_config.width && height == self.surface_config.height
            || width == 0
            || height == 0
        {
            return;
        }

        let (inset_width, inset_height) = match self.viewport {
            Viewport::Full => (0, 0),
            Viewport::Shrink { width, height } => (width, height),
        };

        let dims = self.size().unwrap();
        let current_width = dims.width;
        let current_height = dims.height;

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface
            .configure(&self.device, &self.surface_config, Token);

        let width = width - inset_width;
        let height = height - inset_height;

        let chars_wide = width / self.fonts.min_width_px();
        let chars_high = height / self.fonts.height_px();

        if chars_wide != current_width as u32 || chars_high != current_height as u32 {
            self.cells.clear();
            self.rendered.clear();
            self.sourced.clear();
            self.fast_blinking.clear();
            self.slow_blinking.clear();
        }

        // This always needs to be cleared because the surface is cleared when it is
        // resized. If we don't re-render the rows, we end up with a blank surface when
        // the resize is less than a character dimension.
        self.dirty_rows.clear();

        self.wgpu_state = build_wgpu_state(
            &self.device,
            chars_wide * self.fonts.min_width_px(),
            chars_high * self.fonts.height_px(),
        );

        self.post_process.resize(
            &self.device,
            &self.wgpu_state.text_dest_view,
            &self.surface_config,
        );

        info!(
            "Resized from {}x{} to {}x{}",
            current_width, current_height, chars_wide, chars_high,
        );
    }

    /// Get the text currently displayed on the screen.
    pub fn get_text(&self) -> String {
        let bounds = self.size().unwrap();
        self.cells.chunks(bounds.width as usize).fold(
            String::with_capacity((bounds.width + 1) as usize * bounds.height as usize),
            |dest, row| {
                let mut dest = row.iter().fold(dest, |mut dest, s| {
                    dest.push_str(s.symbol());
                    dest
                });
                dest.push('\n');
                dest
            },
        )
    }

    /// Update the fonts used for rendering. This will cause a full repaint of
    /// the screen the next time [`WgpuBackend::flush`] is called.
    pub fn update_fonts(&mut self, new_fonts: Fonts<'f>) {
        self.dirty_rows.clear();
        self.cached.match_fonts(&new_fonts);
        self.fonts = new_fonts;
    }

    fn render(&mut self) {
        let bounds = self.window_size().unwrap();

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Draw Encoder"),
            });

        if !self.text_vertices.is_empty() {
            {
                let mut uniforms = self
                    .queue
                    .write_buffer_with(
                        &self.text_screen_size_buffer,
                        0,
                        NonZeroU64::new(size_of::<[f32; 4]>() as u64).unwrap(),
                    )
                    .unwrap();
                uniforms.copy_from_slice(bytemuck::cast_slice(&[
                    bounds.columns_rows.width as f32 * self.fonts.min_width_px() as f32,
                    bounds.columns_rows.height as f32 * self.fonts.height_px() as f32,
                    0.0,
                    0.0,
                ]));
            }

            let bg_vertices = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Text Bg Vertices"),
                contents: bytemuck::cast_slice(&self.bg_vertices),
                usage: BufferUsages::VERTEX,
            });

            let fg_vertices = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Text Vertices"),
                contents: bytemuck::cast_slice(&self.text_vertices),
                usage: BufferUsages::VERTEX,
            });

            let indices = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Text Indices"),
                contents: bytemuck::cast_slice(&self.text_indices),
                usage: BufferUsages::INDEX,
            });

            {
                let mut text_render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Text Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &self.wgpu_state.text_dest_view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                });

                text_render_pass.set_index_buffer(indices.slice(..), IndexFormat::Uint32);

                text_render_pass.set_pipeline(&self.text_bg_compositor.pipeline);
                text_render_pass.set_bind_group(0, &self.text_bg_compositor.fs_uniforms, &[]);
                text_render_pass.set_vertex_buffer(0, bg_vertices.slice(..));
                text_render_pass.draw_indexed(0..(self.bg_vertices.len() as u32 / 4) * 6, 0, 0..1);

                text_render_pass.set_pipeline(&self.text_fg_compositor.pipeline);
                text_render_pass.set_bind_group(0, &self.text_fg_compositor.fs_uniforms, &[]);
                text_render_pass.set_bind_group(1, &self.text_fg_compositor.atlas_bindings, &[]);

                text_render_pass.set_vertex_buffer(0, fg_vertices.slice(..));
                text_render_pass.draw_indexed(
                    0..(self.text_vertices.len() as u32 / 4) * 6,
                    0,
                    0..1,
                );
            }
        }

        let Some(texture) = self.surface.get_current_texture(Token) else {
            return;
        };

        self.post_process.process(
            &mut encoder,
            &self.queue,
            &self.wgpu_state.text_dest_view,
            &self.surface_config,
            texture.get_view(Token),
        );

        self.queue.submit(Some(encoder.finish()));
        texture.present(Token);
    }
}

impl<'s, P: PostProcessor, S: RenderSurface<'s>> Backend for WgpuBackend<'_, 's, P, S> {
    fn draw<'a, I>(&mut self, content: I) -> std::io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let bounds = self.size()?;

        self.cells
            .resize(bounds.height as usize * bounds.width as usize, Cell::EMPTY);
        self.sourced.resize_with(
            bounds.height as usize * bounds.width as usize,
            Sourced::default,
        );
        self.rendered.resize_with(
            bounds.height as usize * bounds.width as usize,
            Rendered::default,
        );
        self.fast_blinking
            .resize(bounds.height as usize * bounds.width as usize, false);
        self.slow_blinking
            .resize(bounds.height as usize * bounds.width as usize, false);
        self.dirty_rows.resize(bounds.height as usize, true);

        for (x, y, cell) in content {
            let index = y as usize * bounds.width as usize + x as usize;

            self.fast_blinking
                .set(index, cell.modifier.contains(Modifier::RAPID_BLINK));
            self.slow_blinking
                .set(index, cell.modifier.contains(Modifier::SLOW_BLINK));

            self.cells[index] = cell.clone();

            let width = cell.symbol().width().max(1);
            let start = (index + 1).min(self.cells.len());
            let end = (index + width).min(self.cells.len());
            self.cells[start..end].fill(NULL_CELL);
            self.dirty_rows[y as usize] = true;
        }

        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> std::io::Result<Position> {
        Ok(Position::new(self.cursor.0, self.cursor.1))
    }

    fn set_cursor_position<Pos: Into<Position>>(&mut self, position: Pos) -> std::io::Result<()> {
        let bounds = self.size()?;
        let pos: Position = position.into();
        self.cursor = (pos.x.min(bounds.width - 1), pos.y.min(bounds.height - 1));
        Ok(())
    }

    fn clear(&mut self) -> std::io::Result<()> {
        self.cells.clear();
        self.dirty_rows.clear();
        self.cursor = (0, 0);

        Ok(())
    }

    fn size(&self) -> std::io::Result<Size> {
        let (inset_width, inset_height) = match self.viewport {
            Viewport::Full => (0, 0),
            Viewport::Shrink { width, height } => (width, height),
        };
        let width = self.surface_config.width - inset_width;
        let height = self.surface_config.height - inset_height;

        Ok(Size {
            width: (width / self.fonts.min_width_px()) as u16,
            height: (height / self.fonts.height_px()) as u16,
        })
    }

    fn window_size(&mut self) -> std::io::Result<WindowSize> {
        let (inset_width, inset_height) = match self.viewport {
            Viewport::Full => (0, 0),
            Viewport::Shrink { width, height } => (width, height),
        };
        let width = self.surface_config.width - inset_width;
        let height = self.surface_config.height - inset_height;

        Ok(WindowSize {
            columns_rows: Size {
                width: (width / self.fonts.min_width_px()) as u16,
                height: (height / self.fonts.height_px()) as u16,
            },
            pixels: Size {
                width: width as u16,
                height: height as u16,
            },
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let bounds = self.size()?;
        self.dirty_cells.clear();
        self.dirty_cells.resize(self.cells.len(), false);

        let fast_toggle_dirty = self.last_fast_toggle.elapsed() >= self.fast_duration;
        if fast_toggle_dirty {
            self.last_fast_toggle = Instant::now();
            self.show_fast = !self.show_fast;

            for index in self.fast_blinking.iter_ones() {
                self.dirty_cells.set(index, true);
            }
        }

        let slow_toggle_dirty = self.last_slow_toggle.elapsed() >= self.slow_duration;
        if slow_toggle_dirty {
            self.last_slow_toggle = Instant::now();
            self.show_slow = !self.show_slow;

            for index in self.slow_blinking.iter_ones() {
                self.dirty_cells.set(index, true);
            }
        }

        let mut pending_cache_updates = HashMap::<_, _, RandomState>::default();

        for (y, (row, sourced)) in self
            .cells
            .chunks(bounds.width as usize)
            .zip(self.sourced.chunks_mut(bounds.width as usize))
            .enumerate()
        {
            if !self.dirty_rows[y] {
                continue;
            }

            self.dirty_rows[y] = false;
            let mut new_sourced = vec![Sourced::default(); bounds.width as usize];

            // This block concatenates the strings for the row into one string for bidi
            // resolution, then maps bytes for the string to their associated cell index. It
            // also maps the row's cell index to the font that can source all glyphs for
            // that cell.
            self.row.clear();
            self.rowmap.clear();
            let mut fontmap = Vec::with_capacity(self.rowmap.capacity());
            for (idx, cell) in row.iter().enumerate() {
                self.row.push_str(cell.symbol());
                self.rowmap
                    .resize(self.rowmap.len() + cell.symbol().len(), idx as u16);
                fontmap.push(self.fonts.font_for_cell(cell));
            }

            let mut x = 0;
            // rustbuzz provides a non-zero x-advance for the first character in a cluster
            // with combining characters. The remainder of the cluster doesn't account for
            // this advance, so if we advance prior to rendering them, we end up with all of
            // the associated characters being offset by a cell. To combat this, we only
            // bump the x-advance after we've finished processing all of the characters in a
            // cell. This assumes that we 1) always get a non-zero advance at the beginning
            // of a cluster and 2) the next cluster in the sequence starts with a non-zero
            // advance.
            let mut next_advance = 0;
            let mut shape = |font: &Font,
                             fake_bold,
                             fake_italic,
                             buffer: GlyphBuffer|
             -> UnicodeBuffer {
                let metrics = font.font();
                let advance_scale = self.fonts.height_px() as f32 / metrics.height() as f32;

                for (info, position) in buffer
                    .glyph_infos()
                    .iter()
                    .zip(buffer.glyph_positions().iter())
                {
                    let cell_idx = self.rowmap[info.cluster as usize] as usize;
                    let cell = &row[cell_idx];
                    let max_width = cell.symbol().width();
                    let sourced = &mut new_sourced[cell_idx];

                    let basey = y as i32 * self.fonts.height_px() as i32
                        + (position.y_offset as f32 * advance_scale) as i32;
                    let mut advance = (position.x_advance as f32 * advance_scale) as i32;
                    if advance != 0 {
                        x += next_advance;
                        advance =
                            max_width as i32 * advance.signum() * self.fonts.min_width_px() as i32;
                        next_advance = advance;
                    }
                    let basex = x + (position.x_offset as f32 * advance_scale) as i32;

                    // This assumes that we only want to underline the first character in the
                    // cluster, and that the remaining characters are all combining characters
                    // which don't need an underline.
                    let set = if advance != 0 {
                        Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED
                    } else {
                        Modifier::BOLD | Modifier::ITALIC
                    };

                    let key = Key {
                        style: cell.modifier.intersection(set),
                        glyph: info.glyph_id,
                        font: font.id(),
                    };

                    let ch = self.row[info.cluster as usize..].chars().next().unwrap();
                    let width = (metrics
                        .glyph_hor_advance(GlyphId(info.glyph_id as _))
                        .unwrap_or_default() as f32
                        * advance_scale) as u32;
                    let chars_wide = ch.width().unwrap_or(max_width) as u32;
                    let chars_wide = if chars_wide == 0 { 1 } else { chars_wide };
                    let width = if width == 0 {
                        chars_wide * self.fonts.min_width_px()
                    } else {
                        width
                    };

                    let cached = self.cached.get(
                        &key,
                        chars_wide * self.fonts.min_width_px(),
                        self.fonts.height_px(),
                    );

                    let offset = (basey.max(0) as usize / self.fonts.height_px() as usize)
                        .min(bounds.height as usize - 1)
                        * bounds.width as usize
                        + (basex.max(0) as usize / self.fonts.min_width_px() as usize)
                            .min(bounds.width as usize - 1);

                    sourced.insert((basex, basey, GlyphId(info.glyph_id as _), chars_wide));

                    let mut underline_pos_min = 0;
                    let mut underline_pos_max = 0;
                    if key.style.contains(Modifier::UNDERLINED) {
                        let underline_position = (metrics.ascender() as f32 * advance_scale) as u16;
                        let underline_thickness = metrics
                            .underline_metrics()
                            .map(|m| (m.thickness as f32 * advance_scale) as u16)
                            .unwrap_or(1);
                        underline_pos_min = underline_position;
                        underline_pos_max = underline_pos_min + underline_thickness;
                    }

                    self.rendered[offset].insert(
                        (basex, basey, GlyphId(info.glyph_id as _)),
                        RenderInfo {
                            cell: y * bounds.width as usize + cell_idx,
                            cached: *cached,
                            underline_pos_min,
                            underline_pos_max,
                        },
                    );
                    for x_offset in 0..chars_wide as usize {
                        self.dirty_cells.set(offset + x_offset, true);
                    }

                    if cached.cached() {
                        continue;
                    }

                    pending_cache_updates.entry(key).or_insert_with(|| {
                        let is_emoji = ch.is_emoji_char()
                            && !matches!(ch.general_category_group(), GeneralCategoryGroup::Number);

                        let (rect, image) = rasterize_glyph(
                            cached,
                            metrics,
                            info,
                            fake_italic & !is_emoji,
                            fake_bold & !is_emoji,
                            advance_scale,
                            width,
                        );
                        (rect, image, is_emoji)
                    });
                }

                buffer.clear()
            };

            let bidi = ParagraphBidiInfo::new(&self.row, None);
            let (levels, runs) = bidi.visual_runs(0..bidi.levels.len());

            let (mut current_font, mut current_fake_bold, mut current_fake_italic) = fontmap[0];
            let mut current_level = Level::ltr();

            for (level, range) in runs.into_iter().map(|run| (levels[run.start], run)) {
                let chars = &self.row[range.clone()];
                let cells = &self.rowmap[range.clone()];
                for (idx, ch) in chars.char_indices() {
                    let cell_idx = cells[idx] as usize;
                    let (font, fake_bold, fake_italic) = fontmap[cell_idx];

                    if font.id() != current_font.id()
                        || current_fake_bold != fake_bold
                        || current_fake_italic != fake_italic
                        || current_level != level
                    {
                        let mut buffer = std::mem::take(&mut self.buffer);

                        self.buffer = shape(
                            current_font,
                            current_fake_bold,
                            current_fake_italic,
                            shape_with_plan(
                                current_font.font(),
                                self.plan_cache.get(current_font, &mut buffer),
                                buffer,
                            ),
                        );

                        current_font = font;
                        current_fake_bold = fake_bold;
                        current_fake_italic = fake_italic;
                        current_level = level;
                    }

                    self.buffer.add(ch, (range.start + idx) as u32);
                }
            }

            let mut buffer = std::mem::take(&mut self.buffer);
            self.buffer = shape(
                current_font,
                current_fake_bold,
                current_fake_italic,
                shape_with_plan(
                    current_font.font(),
                    self.plan_cache.get(current_font, &mut buffer),
                    buffer,
                ),
            );

            for (new, old) in new_sourced.into_iter().zip(sourced.iter_mut()) {
                if new != *old {
                    for (x, y, glyph, width) in old.difference(&new) {
                        let cell = ((*y).max(0) as usize / self.fonts.height_px() as usize)
                            .min(bounds.height as usize - 1)
                            * bounds.width as usize
                            + ((*x).max(0) as usize / self.fonts.min_width_px() as usize)
                                .min(bounds.width as usize - 1);

                        for offset_x in 0..*width as usize {
                            if cell >= self.dirty_cells.len() {
                                break;
                            }

                            self.dirty_cells.set(cell + offset_x, true);
                        }

                        self.rendered[cell].shift_remove(&(*x, *y, *glyph));
                    }
                    *old = new;
                }
            }
        }

        for (_, (cached, image, mask)) in pending_cache_updates {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.text_cache,
                    mip_level: 0,
                    origin: Origin3d {
                        x: cached.x,
                        y: cached.y,
                        z: 0,
                    },
                    aspect: TextureAspect::All,
                },
                bytemuck::cast_slice(&image),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cached.width * size_of::<u32>() as u32),
                    rows_per_image: Some(cached.height),
                },
                Extent3d {
                    width: cached.width,
                    height: cached.height,
                    depth_or_array_layers: 1,
                },
            );

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.text_mask,
                    mip_level: 0,
                    origin: Origin3d {
                        x: cached.x,
                        y: cached.y,
                        z: 0,
                    },
                    aspect: TextureAspect::All,
                },
                &vec![if mask { 255 } else { 0 }; (cached.width * cached.height) as usize],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cached.width),
                    rows_per_image: Some(cached.height),
                },
                Extent3d {
                    width: cached.width,
                    height: cached.height,
                    depth_or_array_layers: 1,
                },
            )
        }

        if self.post_process.needs_update() || self.dirty_cells.any() {
            self.bg_vertices.clear();
            self.text_vertices.clear();
            self.text_indices.clear();

            let mut index_offset = 0;
            for index in self.dirty_cells.iter_ones() {
                let cell = &self.cells[index];
                let to_render = &self.rendered[index];

                let reverse = cell.modifier.contains(Modifier::REVERSED);
                let bg_color = if reverse {
                    c2c(cell.fg, self.reset_fg)
                } else {
                    c2c(cell.bg, self.reset_bg)
                };

                let [r, g, b] = bg_color;
                let bg_color_u32: u32 = u32::from_be_bytes([r, g, b, 255]);

                for (
                    (x, y, _),
                    RenderInfo {
                        cell,
                        cached,
                        underline_pos_min,
                        underline_pos_max,
                    },
                ) in to_render.iter()
                {
                    let cell = &self.cells[*cell];
                    let reverse = cell.modifier.contains(Modifier::REVERSED);
                    let fg_color = if reverse {
                        c2c(cell.bg, self.reset_bg)
                    } else {
                        c2c(cell.fg, self.reset_fg)
                    };

                    let alpha = if cell.modifier.contains(Modifier::HIDDEN)
                        | (cell.modifier.contains(Modifier::RAPID_BLINK) & !self.show_fast)
                        | (cell.modifier.contains(Modifier::SLOW_BLINK) & !self.show_slow)
                    {
                        0
                    } else if cell.modifier.contains(Modifier::DIM) {
                        127
                    } else {
                        255
                    };

                    let underline_color = fg_color;
                    let [r, g, b] = fg_color;
                    let fg_color: u32 = u32::from_be_bytes([r, g, b, alpha]);

                    let [r, g, b] = underline_color;
                    let underline_color = u32::from_be_bytes([r, g, b, alpha]);

                    for offset_x in (0..cached.width).step_by(self.fonts.min_width_px() as usize) {
                        self.text_indices.push([
                            index_offset,     // x, y
                            index_offset + 1, // x + w, y
                            index_offset + 2, // x, y + h
                            index_offset + 2, // x, y + h
                            index_offset + 3, // x + w, y + h
                            index_offset + 1, // x + w y
                        ]);
                        index_offset += 4;

                        let x = *x as f32 + offset_x as f32;
                        let y = *y as f32;
                        let uvx = cached.x + offset_x;
                        let uvy = cached.y;

                        self.bg_vertices.push(TextBgVertexMember {
                            vertex: [x, y],
                            bg_color: bg_color_u32,
                        });
                        self.bg_vertices.push(TextBgVertexMember {
                            vertex: [x + self.fonts.min_width_px() as f32, y],
                            bg_color: bg_color_u32,
                        });
                        self.bg_vertices.push(TextBgVertexMember {
                            vertex: [x, y + self.fonts.height_px() as f32],
                            bg_color: bg_color_u32,
                        });
                        self.bg_vertices.push(TextBgVertexMember {
                            vertex: [
                                x + self.fonts.min_width_px() as f32,
                                y + self.fonts.height_px() as f32,
                            ],
                            bg_color: bg_color_u32,
                        });

                        let underline_pos = ((*underline_pos_min as u32 + uvy) << 16)
                            | (*underline_pos_max as u32 + uvy);

                        self.text_vertices.push(TextVertexMember {
                            vertex: [x, y],
                            uv: [uvx as f32, uvy as f32],
                            fg_color,
                            underline_pos,
                            underline_color,
                        });
                        self.text_vertices.push(TextVertexMember {
                            vertex: [x + self.fonts.min_width_px() as f32, y],
                            uv: [uvx as f32 + self.fonts.min_width_px() as f32, uvy as f32],
                            fg_color,
                            underline_pos,
                            underline_color,
                        });
                        self.text_vertices.push(TextVertexMember {
                            vertex: [x, y + self.fonts.height_px() as f32],
                            uv: [uvx as f32, uvy as f32 + self.fonts.height_px() as f32],
                            fg_color,
                            underline_pos,
                            underline_color,
                        });
                        self.text_vertices.push(TextVertexMember {
                            vertex: [
                                x + self.fonts.min_width_px() as f32,
                                y + self.fonts.height_px() as f32,
                            ],
                            uv: [
                                uvx as f32 + self.fonts.min_width_px() as f32,
                                uvy as f32 + self.fonts.height_px() as f32,
                            ],
                            fg_color,
                            underline_pos,
                            underline_color,
                        });
                    }
                }
            }

            self.render();
        }

        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> std::io::Result<()> {
        let bounds = self.size()?;
        let line_start = self.cursor.1 as usize * bounds.width as usize;
        let idx = line_start + self.cursor.0 as usize;

        match clear_type {
            ClearType::All => self.clear(),
            ClearType::AfterCursor => {
                self.cells.truncate(idx + 1);
                Ok(())
            }
            ClearType::BeforeCursor => {
                self.cells[..idx].fill(Cell::EMPTY);
                Ok(())
            }
            ClearType::CurrentLine => {
                self.cells[line_start..line_start + bounds.width as usize].fill(Cell::EMPTY);
                Ok(())
            }
            ClearType::UntilNewLine => {
                let remain = (bounds.width - self.cursor.0) as usize;
                self.cells[idx..idx + remain].fill(Cell::EMPTY);
                Ok(())
            }
        }
    }
}

fn rasterize_glyph(
    cached: Entry,
    metrics: &rustybuzz::Face,
    info: &rustybuzz::GlyphInfo,
    fake_italic: bool,
    fake_bold: bool,
    advance_scale: f32,
    actual_width: u32,
) -> (CacheRect, Vec<u32>) {
    let scale = cached.width as f32 / actual_width as f32;
    let computed_offset_x = -(cached.width as f32 * (1.0 - scale));
    let computed_offset_y = cached.height as f32 * (1.0 - scale);
    let scale = scale * advance_scale * 2.0;

    let skew = if fake_italic {
        Transform::new(
            /* scale x */ 1.0,
            /* skew x */ 0.0,
            /* skew y */ -0.25,
            /* scale y */ 1.0,
            /* translate x */ -0.25 * cached.width as f32,
            /* translate y */ 0.0,
        )
    } else {
        Transform::default()
    };

    let mut image = vec![0u32; cached.width as usize * 2 * cached.height as usize * 2];
    let mut target = DrawTarget::from_backing(
        cached.width as i32 * 2,
        cached.height as i32 * 2,
        &mut image[..],
    );

    let mut painter = Painter::new(
        metrics,
        &mut target,
        skew,
        scale,
        metrics.ascender() as f32 * scale + computed_offset_y,
        computed_offset_x,
    );
    if metrics
        .paint_color_glyph(
            GlyphId(info.glyph_id as _),
            0,
            RgbaColor::new(255, 255, 255, 255),
            &mut painter,
        )
        .is_some()
    {
        let mut final_image = DrawTarget::new(cached.width as i32, cached.height as i32);
        final_image.draw_image_with_size_at(
            cached.width as f32,
            cached.height as f32,
            0.,
            0.,
            &raqote::Image {
                width: cached.width as i32 * 2,
                height: cached.height as i32 * 2,
                data: &image,
            },
            &DrawOptions {
                blend_mode: raqote::BlendMode::Src,
                antialias: raqote::AntialiasMode::None,
                ..Default::default()
            },
        );

        let mut final_image = final_image.into_vec();
        for argb in final_image.iter_mut() {
            let [a, r, g, b] = argb.to_be_bytes();
            *argb = u32::from_le_bytes([r, g, b, a]);
        }

        return (*cached, final_image);
    }

    if let Some(raster) = metrics.glyph_raster_image(GlyphId(info.glyph_id as _), u16::MAX) {
        if let Some(value) = extract_color_image(&mut image, raster, cached, advance_scale) {
            return value;
        }
    }

    let mut render = Outline::default();
    if let Some(bounds) = metrics.outline_glyph(GlyphId(info.glyph_id as _), &mut render) {
        let path = render.finish();

        // Some fonts return bounds that are entirely negative. I'm not sure why this
        // is, but it means the glyph won't render at all. We check for this here and
        // offset it if so. This seems to let those fonts render correctly.
        let x_off = if bounds.x_max < 0 {
            -bounds.x_min as f32
        } else {
            0.
        };
        let x_off = x_off * scale + computed_offset_x;
        let y_off = metrics.ascender() as f32 * scale + computed_offset_y;

        let mut target = DrawTarget::from_backing(
            cached.width as i32 * 2,
            cached.height as i32 * 2,
            &mut image[..],
        );
        target.set_transform(
            &Transform::scale(scale, -scale)
                .then(&skew)
                .then_translate((x_off, y_off).into()),
        );

        target.fill(
            &path,
            &raqote::Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 255, 255)),
            &DrawOptions::default(),
        );

        if fake_bold {
            target.stroke(
                &path,
                &raqote::Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 255, 255)),
                &StrokeStyle {
                    width: 1.5,
                    ..Default::default()
                },
                &DrawOptions::new(),
            );
        }

        let mut final_image = DrawTarget::new(cached.width as i32, cached.height as i32);
        final_image.draw_image_with_size_at(
            cached.width as f32,
            cached.height as f32,
            0.,
            0.,
            &raqote::Image {
                width: cached.width as i32 * 2,
                height: cached.height as i32 * 2,
                data: &image,
            },
            &DrawOptions {
                blend_mode: raqote::BlendMode::Src,
                antialias: raqote::AntialiasMode::None,
                ..Default::default()
            },
        );

        return (*cached, final_image.into_vec());
    }

    if let Some(raster) = metrics.glyph_raster_image(GlyphId(info.glyph_id as _), u16::MAX) {
        if let Some(value) = extract_bw_image(&mut image, raster, cached, advance_scale) {
            return value;
        }
    }

    (
        *cached,
        vec![0u32; cached.width as usize * cached.height as usize],
    )
}

fn extract_color_image(
    image: &mut Vec<u32>,
    raster: RasterGlyphImage,
    cached: Entry,
    scale: f32,
) -> Option<(CacheRect, Vec<u32>)> {
    match raster.format {
        RasterImageFormat::PNG => {
            #[cfg(feature = "png")]
            {
                let decoder = png::Decoder::new(std::io::Cursor::new(raster.data));
                if let Ok(mut info) = decoder.read_info() {
                    image.resize(
                        info.output_buffer_size().unwrap_or_default() / size_of::<u32>(),
                        0,
                    );
                    if info.next_frame(bytemuck::cast_slice_mut(image)).is_err() {
                        return None;
                    }

                    for rgba in image.iter_mut() {
                        let [r, g, b, a] = rgba.to_be_bytes();
                        *rgba = u32::from_be_bytes([a, r, g, b]);
                    }
                } else {
                    return None;
                }
            }
            #[cfg(not(feature = "png"))]
            return None;
        }
        RasterImageFormat::BitmapPremulBgra32 => {
            image.resize(raster.width as usize * raster.height as usize, 0);
            for (y, row) in raster.data.chunks(raster.width as usize * 4).enumerate() {
                for (x, pixel) in row.chunks(4).enumerate() {
                    let pixel: &[u8; 4] = pixel.try_into().expect("Invalid chunk size");
                    let [b, g, r, a] = *pixel;
                    let pixel = u32::from_be_bytes([
                        a,
                        r.saturating_mul(255 / a),
                        g.saturating_mul(255 / a),
                        b.saturating_mul(255 / a),
                    ]);
                    image[y * raster.width as usize + x] = pixel;
                }
            }
        }
        _ => return None,
    }

    let mut final_image = DrawTarget::new(cached.width as i32, cached.height as i32);
    final_image.draw_image_with_size_at(
        cached.width as f32,
        cached.height as f32,
        raster.x as f32 * scale,
        raster.y as f32 * scale,
        &raqote::Image {
            width: raster.width as i32,
            height: raster.height as i32,
            data: &*image,
        },
        &DrawOptions {
            blend_mode: raqote::BlendMode::Src,
            antialias: raqote::AntialiasMode::None,
            ..Default::default()
        },
    );

    let mut final_image = final_image.into_vec();
    for argb in final_image.iter_mut() {
        let [a, r, g, b] = argb.to_be_bytes();
        *argb = u32::from_le_bytes([r, g, b, a]);
    }

    Some((*cached, final_image))
}

fn extract_bw_image(
    image: &mut Vec<u32>,
    raster: RasterGlyphImage,
    cached: Entry,
    scale: f32,
) -> Option<(CacheRect, Vec<u32>)> {
    image.resize(raster.width as usize * raster.height as usize, 0);

    match raster.format {
        RasterImageFormat::BitmapMono => {
            from_gray_unpacked::<1, 2>(image, raster, LUT_1);
        }
        RasterImageFormat::BitmapMonoPacked => {
            from_gray_packed::<1, 2>(image, raster, LUT_1);
        }
        RasterImageFormat::BitmapGray2 => {
            from_gray_unpacked::<2, 4>(image, raster, LUT_2);
        }
        RasterImageFormat::BitmapGray2Packed => {
            from_gray_packed::<2, 4>(image, raster, LUT_2);
        }
        RasterImageFormat::BitmapGray4 => {
            from_gray_unpacked::<4, 16>(image, raster, LUT_4);
        }
        RasterImageFormat::BitmapGray4Packed => {
            from_gray_packed::<4, 16>(image, raster, LUT_4);
        }
        RasterImageFormat::BitmapGray8 => {
            for (byte, dst) in raster.data.iter().zip(image.iter_mut()) {
                *dst = u32::from_be_bytes([*byte, 255, 255, 255]);
            }
        }
        _ => return None,
    }

    let mut final_image = DrawTarget::new(cached.width as i32, cached.height as i32);
    final_image.draw_image_with_size_at(
        cached.width as f32,
        cached.height as f32,
        raster.x as f32 * scale,
        raster.y as f32 * scale,
        &raqote::Image {
            width: raster.width as i32,
            height: raster.height as i32,
            data: &*image,
        },
        &DrawOptions {
            blend_mode: raqote::BlendMode::Src,
            antialias: raqote::AntialiasMode::None,
            ..Default::default()
        },
    );

    let mut final_image = final_image.into_vec();
    for argb in final_image.iter_mut() {
        let [a, r, g, b] = argb.to_be_bytes();
        *argb = u32::from_le_bytes([r, g, b, a]);
    }

    Some((*cached, final_image))
}

fn from_gray_unpacked<const BITS: usize, const ENTRIES: usize>(
    image: &mut [u32],
    raster: RasterGlyphImage,
    steps: [u8; ENTRIES],
) {
    for (bits, dst) in raster
        .data
        .chunks((raster.width as usize / (8 / BITS)) + 1)
        .zip(image.chunks_mut(raster.width as usize))
    {
        let bits = BitSlice::<_, Lsb0>::from_slice(bits);
        for (bits, dst) in bits.chunks(BITS).zip(dst.iter_mut()) {
            let mut index = 0;
            for idx in bits.iter_ones() {
                index |= 1 << (BITS - idx - 1);
            }
            let value = steps[index as usize];
            *dst = u32::from_be_bytes([value, 255, 255, 255]);
        }
    }
}

fn from_gray_packed<const BITS: usize, const ENTRIES: usize>(
    image: &mut [u32],
    raster: RasterGlyphImage,
    steps: [u8; ENTRIES],
) {
    let bits = BitSlice::<_, Lsb0>::from_slice(raster.data);
    for (bits, dst) in bits.chunks(BITS).zip(image.iter_mut()) {
        let mut index = 0;
        for idx in bits.iter_ones() {
            index |= 1 << (BITS - idx - 1);
        }
        let value = steps[index as usize];
        *dst = u32::from_be_bytes([value, 255, 255, 255]);
    }
}

const LUT_1: [u8; 2] = [0, 255];
const LUT_2: [u8; 4] = [0, 255 / 3, 2 * (255 / 3), 255];
const LUT_4: [u8; 16] = [
    0,
    (255 / 15),
    2 * (255 / 15),
    3 * (255 / 15),
    4 * (255 / 15),
    5 * (255 / 15),
    6 * (255 / 15),
    7 * (255 / 15),
    8 * (255 / 15),
    9 * (255 / 15),
    10 * (255 / 15),
    11 * (255 / 15),
    12 * (255 / 15),
    13 * (255 / 15),
    14 * (255 / 15),
    255,
];

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use image::load_from_memory;
    use image::GenericImageView;
    use image::ImageBuffer;
    use image::Rgba;
    use ratatui::style::Color;
    use ratatui::style::Stylize;
    use ratatui::text::Line;
    use ratatui::widgets::Block;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;
    use rustybuzz::ttf_parser::RasterGlyphImage;
    use rustybuzz::ttf_parser::RasterImageFormat;
    use serial_test::serial;
    use wgpu::wgt::PollType;
    use wgpu::CommandEncoderDescriptor;
    use wgpu::Device;
    use wgpu::Extent3d;
    use wgpu::Queue;
    use wgpu::TextureFormat;

    use crate::backend::wgpu_backend::extract_bw_image;
    use crate::backend::wgpu_backend::LUT_2;
    use crate::backend::wgpu_backend::LUT_4;
    use crate::backend::HeadlessSurface;
    use crate::shaders::DefaultPostProcessor;
    use crate::utils::text_atlas::CacheRect;
    use crate::utils::text_atlas::Entry;
    use crate::Builder;
    use crate::Dimensions;
    use crate::Font;

    fn tex2buffer(device: &Device, queue: &Queue, surface: &HeadlessSurface) {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            surface.texture.as_ref().unwrap().as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: surface.buffer.as_ref().unwrap(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(surface.buffer_width),
                    rows_per_image: Some(surface.height),
                },
            },
            Extent3d {
                width: surface.width,
                height: surface.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));
    }

    #[test]
    #[serial]
    fn a_z() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/CascadiaMono-Regular.ttf"))
                        .expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(512).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("ABCDEFGHIJKLMNOPQRSTUVWXYZ"), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/a_z.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn arabic() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/CascadiaMono-Regular.ttf"))
                        .expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(256).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("مرحبا بالعالم"), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/arabic.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn really_wide() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/Fairfax.ttf")).expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(512).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("Ｈｅｌｌｏ, ｗｏｒｌｄ!"), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/really_wide.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn mixed() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/CascadiaMono-Regular.ttf"))
                        .expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(512).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(
                    Paragraph::new("Hello World! مرحبا بالعالم 0123456789000000000"),
                    area,
                );
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/mixed.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn mixed_colors() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/CascadiaMono-Regular.ttf"))
                        .expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(512).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        "Hello World!".green(),
                        "مرحبا بالعالم".blue(),
                        "0123456789".dim(),
                    ])),
                    area,
                );
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/mixed_colors.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn overlap() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/Fairfax.ttf")).expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(256).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("H̴̢͕̠͖͇̻͓̙̞͔͕͓̰͋͛͂̃̌͂͆͜͠".underlined()), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/overlap_initial.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }
        surface.buffer.as_ref().unwrap().unmap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("H".underlined()), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/overlap_post.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }

        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn overlap_colors() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/Fairfax.ttf")).expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(256).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .build_headless(),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("H̴̢͕̠͖͇̻͓̙̞͔͕͓̰͋͛͂̃̌͂͆͜͠".blue().on_red().underlined()), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/overlap_colors.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }
        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn rgb_conversion() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/Fairfax.ttf")).expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(256).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .with_bg_color(Color::Rgb(0x1E, 0x23, 0x26))
                .with_fg_color(Color::White)
                .build_headless_with_format(TextureFormat::Rgba8Unorm),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("TEST"), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/rgb_conversion.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }
        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[serial]
    fn srgb_conversion() {
        let mut terminal = Terminal::new(
            futures_lite::future::block_on(
                Builder::<DefaultPostProcessor>::from_font(
                    Font::new(include_bytes!("fonts/Fairfax.ttf")).expect("Invalid font file"),
                )
                .with_width_and_height(Dimensions {
                    width: NonZeroU32::new(256).unwrap(),
                    height: NonZeroU32::new(72).unwrap(),
                })
                .with_bg_color(Color::Rgb(0x1E, 0x23, 0x26))
                .with_fg_color(Color::White)
                .build_headless_with_format(TextureFormat::Rgba8UnormSrgb),
            )
            .unwrap(),
        )
        .unwrap();

        terminal
            .draw(|f| {
                let block = Block::bordered();
                let area = block.inner(f.area());
                f.render_widget(block, f.area());
                f.render_widget(Paragraph::new("TEST"), area);
            })
            .unwrap();

        let surface = &terminal.backend().surface;
        tex2buffer(
            &terminal.backend().device,
            &terminal.backend().queue,
            surface,
        );
        {
            let buffer = surface.buffer.as_ref().unwrap().slice(..);

            let (send, recv) = oneshot::channel();
            buffer.map_async(wgpu::MapMode::Read, move |data| {
                send.send(data).unwrap();
            });
            terminal.backend().device.poll(PollType::Wait).unwrap();
            recv.recv().unwrap().unwrap();

            let data = buffer.get_mapped_range();
            let image =
                ImageBuffer::<Rgba<u8>, _>::from_raw(surface.width, surface.height, data).unwrap();

            let pixels = image.pixels().copied().collect::<Vec<_>>();
            let golden = load_from_memory(include_bytes!("goldens/srgb_conversion.png")).unwrap();
            let golden_pixels = golden.pixels().map(|(_, _, px)| px).collect::<Vec<_>>();

            assert!(
                pixels == golden_pixels,
                "Rendered image differs from golden"
            );
        }
        surface.buffer.as_ref().unwrap().unmap();
    }

    #[test]
    #[cfg(feature = "png")]
    fn png() {
        use crate::backend::wgpu_backend::extract_color_image;
        let golden = load_from_memory(include_bytes!("goldens/A.png")).unwrap();
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: golden.width() as u16,
            height: golden.height() as u16,
            pixels_per_em: 0,
            format: RasterImageFormat::PNG,
            data: include_bytes!("goldens/A.png"),
        };

        let mut image = vec![];
        let extracted = extract_color_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: golden.width(),
                height: golden.height(),
            }),
            1.0,
        )
        .expect("Didn't extract png")
        .1;

        for (l, r) in bytemuck::cast_slice::<_, u8>(&extracted)
            .chunks(4)
            .zip(golden.pixels())
        {
            let [r, g, b, a] = r.2 .0;
            assert_eq!(l, [a, b, g, r]);
        }
    }

    #[test]
    fn bgra() {
        use crate::backend::wgpu_backend::extract_color_image;

        const BLUE: u8 = 2;
        const GREEN: u8 = 4;
        const RED: u8 = 8;
        const ALPHA: u8 = 127;
        let data = [BLUE, GREEN, RED, ALPHA];
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapPremulBgra32,
            data: &data,
        };

        let mut image = vec![];
        let extracted = extract_color_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
            1.0,
        )
        .expect("Didn't extract bgra")
        .1;

        assert_eq!(
            bytemuck::bytes_of(&extracted[0]),
            [RED * 2, GREEN * 2, BLUE * 2, ALPHA]
        );
    }

    #[test]
    fn bmp1() {
        let data0 = 0b1000_0001;
        let data1 = 0b0001_1000;
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 4,
            height: 2,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapMono,
            data: &[data0, data1],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 4,
                height: 2,
            }),
            1.0,
        )
        .expect("Didn't extract bmp1")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [
                    [255u8, 255, 255, 255,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                ],
                [
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 255,],
                ],
            ])
        );
    }

    #[test]
    fn bmp1_packed() {
        let data0 = 0b1000_0001;
        let data1 = 0b0001_1000;
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 8,
            height: 2,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapMonoPacked,
            data: &[data0, data1],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 8,
                height: 2,
            }),
            1.0,
        )
        .expect("Didn't extract bmp1")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [
                    [255u8, 255, 255, 255,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 255,],
                ],
                [
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 255,],
                    [255, 255, 255, 255,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                    [255, 255, 255, 0,],
                ],
            ])
        );
    }

    #[test]
    fn bmp2() {
        let data0 = 0b1010_1101u8.reverse_bits();
        let data1 = 0b0000_1000u8.reverse_bits();
        let data2 = 0b0111_1010u8.reverse_bits();
        let data3 = 0b0000_1000u8.reverse_bits();
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 6,
            height: 2,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapGray2,
            data: &[data0, data1, data2, data3],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 6,
                height: 2,
            }),
            1.0,
        )
        .expect("Didn't extract bmp2")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b11],],
                    [255, 255, 255, LUT_2[0b01],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                ],
                [
                    [255, 255, 255, LUT_2[0b01],],
                    [255, 255, 255, LUT_2[0b11],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                ],
            ])
        );
    }

    #[test]
    fn bmp2_packed() {
        let data0 = 0b1010_1101u8.reverse_bits();
        let data1 = 0b0000_1000u8.reverse_bits();
        let data2 = 0b0111_1010u8.reverse_bits();
        let data3 = 0b0000_1000u8.reverse_bits();
        let data4 = 0b0000_1000u8.reverse_bits();
        let data5 = 0b0000_1000u8.reverse_bits();
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 6,
            height: 4,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapGray2Packed,
            data: &[data0, data1, data2, data3, data4, data5],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 6,
                height: 4,
            }),
            1.0,
        )
        .expect("Didn't extract bmp2")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b11],],
                    [255, 255, 255, LUT_2[0b01],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                ],
                [
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b01],],
                    [255, 255, 255, LUT_2[0b11],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b10],],
                ],
                [
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                ],
                [
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b00],],
                    [255, 255, 255, LUT_2[0b10],],
                    [255, 255, 255, LUT_2[0b00],],
                ]
            ])
        );
    }

    #[test]
    fn bmp4() {
        let data0 = 0b1010_1000u8.reverse_bits();
        let data1 = 0b0000_1000u8.reverse_bits();
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 1,
            height: 2,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapGray4,
            data: &[data0, data1],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 1,
                height: 2,
            }),
            1.0,
        )
        .expect("Didn't extract bmp4")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [[255, 255, 255, LUT_4[0b1010],],],
                [[255, 255, 255, LUT_4[0b0000],],],
            ])
        );
    }

    #[test]
    fn bmp4_packed() {
        let data0 = 0b1111_0001u8.reverse_bits();
        let data1 = 0b0011_1100u8.reverse_bits();
        let raster = RasterGlyphImage {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            pixels_per_em: 0,
            format: RasterImageFormat::BitmapGray4Packed,
            data: &[data0, data1],
        };

        let mut image = vec![];
        let extracted = extract_bw_image(
            &mut image,
            raster,
            Entry::Cached(CacheRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }),
            1.0,
        )
        .expect("Didn't extract bmp4")
        .1;

        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&extracted),
            bytemuck::cast_slice(&[
                [
                    [255, 255, 255, LUT_4[0b1111],],
                    [255, 255, 255, LUT_4[0b0001],],
                ],
                [
                    [255, 255, 255, LUT_4[0b0011],],
                    [255, 255, 255, LUT_4[0b1100],],
                ],
            ])
        );
    }
}
