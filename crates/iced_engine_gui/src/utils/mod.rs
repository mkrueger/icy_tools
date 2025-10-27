use raqote::Color;
use raqote::DrawOptions;
use raqote::DrawTarget;
use raqote::Gradient;
use raqote::GradientStop;
use raqote::IntRect;
use raqote::Path;
use raqote::PathBuilder;
use raqote::Point;
use raqote::SolidSource;
use raqote::Source;
use raqote::Transform;
use raqote::Vector;
use rustybuzz::ttf_parser::colr::CompositeMode;
use rustybuzz::Face;

pub(crate) mod plan_cache;
pub(crate) mod text_atlas;

pub(crate) struct Outline {
    path: PathBuilder,
}

impl Default for Outline {
    fn default() -> Self {
        Self {
            path: PathBuilder::new(),
        }
    }
}

impl Outline {
    pub(crate) fn finish(self) -> Path {
        self.path.finish()
    }
}

impl rustybuzz::ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(x1, y1, x, y)
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

pub(crate) struct Painter<'f, 'd, 'p> {
    font: &'f Face<'d>,
    target: &'f mut DrawTarget<&'p mut [u32]>,
    outline: Option<Path>,
    skew: Transform,
    scale: f32,
    y_offset: f32,
    x_offset: f32,
    transforms: Vec<rustybuzz::ttf_parser::Transform>,
}

impl<'f, 'd, 'p> Painter<'f, 'd, 'p> {
    pub(crate) fn new(
        font: &'f Face<'d>,
        target: &'f mut DrawTarget<&'p mut [u32]>,
        skew: Transform,
        scale: f32,
        y_offset: f32,
        x_offset: f32,
    ) -> Self {
        Self {
            font,
            target,
            outline: None,
            skew,
            scale,
            y_offset,
            x_offset,
            transforms: vec![],
        }
    }

    fn compute_transform(&self) -> Transform {
        self.transforms
            .iter()
            // Applying the pushed transforms in reverse order empirically produces the correct
            // result. I can find no indication in the documentation of any font parsing crate nor
            // in the documention for the colr tables indicating that this is the expected way to
            // apply these transforms. It's possible I missed something (possibly this has to do
            // with the fact that layers are specified from the bottom up?).
            .rev()
            .fold(Transform::default(), |tfm, t| {
                tfm.then(&Transform::new(t.a, t.b, t.c, t.d, t.e, t.f))
            })
            .then_scale(self.scale, -self.scale)
            .then(&self.skew)
            .then_translate((self.x_offset, self.y_offset).into())
    }
}

impl<'a> rustybuzz::ttf_parser::colr::Painter<'a> for Painter<'_, '_, '_> {
    fn outline_glyph(&mut self, glyph_id: rustybuzz::ttf_parser::GlyphId) {
        let mut outline = Outline::default();
        self.outline = self
            .font
            .outline_glyph(glyph_id, &mut outline)
            .map(|_| outline.finish());
    }

    /// The documentation for this states "Paint the stored outline using the
    /// provided color", but this is only true for colr v0 outlines. For colr
    /// v1, the outline will have been pushed using `push_clip`, and we
    /// should instead fill the clipped region with the provided paint. We
    /// handle this by storing the outline in an optional, which will be taken
    /// from during the `push_clip` operation, leaving it empty. This way we
    /// know if the outline is present that we are supposed to paint the
    /// path directly.
    fn paint(&mut self, paint: rustybuzz::ttf_parser::colr::Paint<'a>) {
        let paint = match paint {
            rustybuzz::ttf_parser::colr::Paint::Solid(color) => {
                Source::Solid(SolidSource::from_unpremultiplied_argb(
                    color.alpha,
                    color.red,
                    color.green,
                    color.blue,
                ))
            }
            rustybuzz::ttf_parser::colr::Paint::LinearGradient(grad) => {
                // https://learn.microsoft.com/en-us/typography/opentype/spec/colr#linear-gradients
                let mut stops = grad
                    .stops(0, &[])
                    .map(|stop| GradientStop {
                        position: stop.stop_offset,
                        color: Color::new(
                            stop.color.alpha,
                            stop.color.red,
                            stop.color.green,
                            stop.color.blue,
                        ),
                    })
                    .collect::<Vec<_>>();
                stops.sort_by(|l, r| l.position.total_cmp(&r.position));

                let transform = self.compute_transform();
                let p0 = transform.transform_point(Point::new(grad.x0, grad.y0));
                let p1 = transform.transform_point(Point::new(grad.x1, grad.y1));
                let p2 = transform.transform_point(Point::new(grad.x2, grad.y2));

                if p0 == p1 || p0 == p2 {
                    return;
                }

                // Get the line perpendicular to p0p2
                let dist = p2 - p0;
                let perp = Vector::new(dist.y, -dist.x);

                // Project the vector p0p1 onto the perpendicular line passing through p0
                let dist = p1 - p0;
                let p3 = p0 + dist.project_onto_vector(perp);

                Source::new_linear_gradient(
                    Gradient { stops },
                    p0,
                    p3,
                    match grad.extend {
                        rustybuzz::ttf_parser::colr::GradientExtend::Pad => raqote::Spread::Pad,
                        rustybuzz::ttf_parser::colr::GradientExtend::Repeat => {
                            raqote::Spread::Repeat
                        }
                        rustybuzz::ttf_parser::colr::GradientExtend::Reflect => {
                            raqote::Spread::Reflect
                        }
                    },
                )
            }
            rustybuzz::ttf_parser::colr::Paint::RadialGradient(grad) => {
                let mut stops = grad
                    .stops(0, &[])
                    .map(|stop| GradientStop {
                        position: stop.stop_offset,
                        color: Color::new(
                            stop.color.alpha,
                            stop.color.red,
                            stop.color.green,
                            stop.color.blue,
                        ),
                    })
                    .collect::<Vec<_>>();
                stops.sort_by(|l, r| l.position.total_cmp(&r.position));

                let p0 = Point::new(grad.x0, grad.y0);
                let p1 = Point::new(grad.x1, grad.y1);
                let r0 = grad.r0;
                let r1 = grad.r1;

                if p0 == p1 && r0 == r1 {
                    return;
                }

                Source::TwoCircleRadialGradient(
                    Gradient { stops },
                    match grad.extend {
                        rustybuzz::ttf_parser::colr::GradientExtend::Pad => raqote::Spread::Pad,
                        rustybuzz::ttf_parser::colr::GradientExtend::Repeat => {
                            raqote::Spread::Repeat
                        }
                        rustybuzz::ttf_parser::colr::GradientExtend::Reflect => {
                            raqote::Spread::Reflect
                        }
                    },
                    p0,
                    r0,
                    p1,
                    r1,
                    self.compute_transform().inverse().unwrap_or_default(),
                )
            }
            rustybuzz::ttf_parser::colr::Paint::SweepGradient(grad) => {
                let mut stops = grad
                    .stops(0, &[])
                    .map(|stop| GradientStop {
                        position: stop.stop_offset,
                        color: Color::new(
                            stop.color.alpha,
                            stop.color.red,
                            stop.color.green,
                            stop.color.blue,
                        ),
                    })
                    .collect::<Vec<_>>();
                stops.sort_by(|l, r| l.position.total_cmp(&r.position));

                Source::SweepGradient(
                    Gradient { stops },
                    match grad.extend {
                        rustybuzz::ttf_parser::colr::GradientExtend::Pad => raqote::Spread::Pad,
                        rustybuzz::ttf_parser::colr::GradientExtend::Repeat => {
                            raqote::Spread::Repeat
                        }
                        rustybuzz::ttf_parser::colr::GradientExtend::Reflect => {
                            raqote::Spread::Reflect
                        }
                    },
                    grad.start_angle,
                    grad.end_angle,
                    self.compute_transform()
                        .inverse()
                        .unwrap_or_default()
                        .then_translate((-grad.center_x, -grad.center_y).into()),
                )
            }
        };

        let draw_options = DrawOptions {
            antialias: raqote::AntialiasMode::None,
            ..Default::default()
        };

        self.target.set_transform(&Transform::default());
        if let Some(outline) = self.outline.take() {
            let outline = outline.transform(&self.compute_transform());
            self.target.fill(&outline, &paint, &draw_options);
        } else {
            self.target.fill_rect(
                0.,
                0.,
                self.target.width() as f32,
                self.target.height() as f32,
                &paint,
                &draw_options,
            );
        }
    }

    fn push_clip(&mut self) {
        self.target.set_transform(&self.compute_transform());
        self.target.push_clip(
            &self
                .outline
                .take()
                .unwrap_or_else(|| PathBuilder::new().finish()),
        );
    }

    fn push_clip_box(&mut self, clipbox: rustybuzz::ttf_parser::colr::ClipBox) {
        let transform = self.compute_transform();

        let xy0 = transform.transform_point((clipbox.x_min, clipbox.y_min).into());
        let xy1 = transform.transform_point((clipbox.x_max, clipbox.y_max).into());
        let xy2 = transform.transform_point((clipbox.x_min, clipbox.y_max).into());
        let xy3 = transform.transform_point((clipbox.x_max, clipbox.y_min).into());
        let min_xy = xy0.min(xy1).min(xy2).min(xy3);
        let max_xy = xy0.max(xy1).max(xy2).max(xy3);

        self.target.push_clip_rect(IntRect {
            min: min_xy.to_i32(),
            max: max_xy.to_i32(),
        });
    }

    fn pop_clip(&mut self) {
        self.target.pop_clip();
    }

    fn push_layer(&mut self, mode: rustybuzz::ttf_parser::colr::CompositeMode) {
        self.target.push_layer_with_blend(
            1.0,
            match mode {
                CompositeMode::Clear => raqote::BlendMode::Clear,
                CompositeMode::Source => raqote::BlendMode::Src,
                CompositeMode::Destination => raqote::BlendMode::Dst,
                CompositeMode::SourceOver => raqote::BlendMode::SrcOver,
                CompositeMode::DestinationOver => raqote::BlendMode::DstOver,
                CompositeMode::SourceIn => raqote::BlendMode::SrcIn,
                CompositeMode::DestinationIn => raqote::BlendMode::DstIn,
                CompositeMode::SourceOut => raqote::BlendMode::SrcOut,
                CompositeMode::DestinationOut => raqote::BlendMode::DstOut,
                CompositeMode::SourceAtop => raqote::BlendMode::SrcAtop,
                CompositeMode::DestinationAtop => raqote::BlendMode::DstAtop,
                CompositeMode::Xor => raqote::BlendMode::Xor,
                CompositeMode::Plus => raqote::BlendMode::Add,
                CompositeMode::Screen => raqote::BlendMode::Screen,
                CompositeMode::Overlay => raqote::BlendMode::Overlay,
                CompositeMode::Darken => raqote::BlendMode::Darken,
                CompositeMode::Lighten => raqote::BlendMode::Lighten,
                CompositeMode::ColorDodge => raqote::BlendMode::ColorDodge,
                CompositeMode::ColorBurn => raqote::BlendMode::ColorBurn,
                CompositeMode::HardLight => raqote::BlendMode::HardLight,
                CompositeMode::SoftLight => raqote::BlendMode::SoftLight,
                CompositeMode::Difference => raqote::BlendMode::Difference,
                CompositeMode::Exclusion => raqote::BlendMode::Exclusion,
                CompositeMode::Multiply => raqote::BlendMode::Multiply,
                CompositeMode::Hue => raqote::BlendMode::Hue,
                CompositeMode::Saturation => raqote::BlendMode::Saturation,
                CompositeMode::Color => raqote::BlendMode::Color,
                CompositeMode::Luminosity => raqote::BlendMode::Luminosity,
            },
        );
    }

    fn pop_layer(&mut self) {
        self.target.pop_layer();
    }

    fn push_transform(&mut self, transform: rustybuzz::ttf_parser::Transform) {
        self.transforms.push(transform);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }
}
