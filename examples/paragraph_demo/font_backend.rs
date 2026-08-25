use std::collections::HashMap;

use harfrust::{Direction, Feature, FontRef as HarfBuzzFontRef, ShaperData, Tag, UnicodeBuffer};
use read_fonts::model::pen::{ControlBoundsPen, OutlinePen};
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef as SkrifaFontRef, GlyphId, MetadataProvider};
use tiqian::org::tiqian::core::Geometry::{Rect, TextRange};
use tiqian::org::tiqian::core::LayoutModel::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::org::tiqian::font::FontMetrics::{
    FontMetricSource, FontMetricsRequest, FontMetricsResolver,
};
use tiqian::org::tiqian::font::FontPolicy::{
    FallbackResolver, FontCandidate, FontDecision, FontRequest, FontRole, RawFontMetrics,
};
use tiqian::org::tiqian::shaping::TextShaper::{ShapingInput, ShapingResult, ShapingSource, TextShaper};

const CJK_FONT_KEY: &str = "demo-cjk";
const LATIN_FONT_KEY: &str = "demo-latin";
const CJK_FONT_BYTES: &[u8] = include_bytes!("../../resources/fonts/SourceHanSansSC-VF.otf");
const LATIN_FONT_BYTES: &[u8] = include_bytes!("../../resources/fonts/InterVariable.ttf");

#[derive(Clone)]
pub struct DemoFontCatalog {
    faces: HashMap<&'static str, DemoFontFace>,
}

#[derive(Clone)]
struct DemoFontFace {
    key: &'static str,
    family: &'static str,
    bytes: &'static [u8],
    units_per_em: u32,
    metrics: TableMetrics,
    weight_axis: Option<WeightAxis>,
}

#[derive(Clone, Copy)]
struct WeightAxis {
    minimum: f32,
    maximum: f32,
}

#[derive(Clone, Copy)]
struct TableMetrics {
    ascent: i16,
    descent: i16,
    leading: i16,
    typo_ascent: Option<i16>,
    typo_descent: Option<i16>,
}

impl DemoFontCatalog {
    pub fn load() -> Result<Self, String> {
        let cjk = DemoFontFace::load(CJK_FONT_KEY, "Source Han Sans SC", CJK_FONT_BYTES)?;
        let latin = DemoFontFace::load(LATIN_FONT_KEY, "Inter", LATIN_FONT_BYTES)?;
        Ok(Self {
            faces: HashMap::from([(CJK_FONT_KEY, cjk), (LATIN_FONT_KEY, latin)]),
        })
    }

    pub fn validate_demo_faces(&self) -> Result<(), String> {
        for face in self.faces.values() {
            SkrifaFontRef::new(face.bytes)
                .map_err(|_| format!("font validation failed: {}", face.family))?;
            HarfBuzzFontRef::new(face.bytes)
                .map_err(|_| format!("font validation failed: {}", face.family))?;
            face.weight_for(400)?;
        }
        Ok(())
    }

    fn face_for_key(&self, key: &str) -> &DemoFontFace {
        self.faces
            .get(key)
            .unwrap_or_else(|| panic!("paragraph-demo received unknown font key: {key}"))
    }

    fn face_for_role(&self, role: FontRole) -> &DemoFontFace {
        let key = if role == FontRole::LatinText {
            LATIN_FONT_KEY
        } else {
            CJK_FONT_KEY
        };
        self.face_for_key(key)
    }

    fn resolved_font_key(face: &DemoFontFace, weight: f32) -> String {
        format!("{}@wght={weight}", face.key)
    }

    pub fn paint_glyph(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        render_font_key: &str,
        glyph_id: u32,
        font_size: f32,
        origin_x: f32,
        origin_y: f32,
        color: tiny_skia::Color,
    ) -> Result<(), String> {
        let (key, weight) = render_font_key
            .split_once("@wght=")
            .ok_or_else(|| format!("invalid paragraph-demo render font key: {render_font_key}"))?;
        let weight = weight
            .parse::<f32>()
            .map_err(|_| format!("invalid paragraph-demo render weight: {render_font_key}"))?;
        let face = self.face_for_key(key);
        let font = SkrifaFontRef::new(face.bytes)
            .map_err(|_| format!("font decode failed during glyph replay: {}", face.family))?;
        let location = match face.weight_axis {
            Some(axis) if weight >= axis.minimum && weight <= axis.maximum => {
                font.axes().location([("wght", weight)])
            }
            Some(axis) => {
                return Err(format!(
                    "render weight {weight} is outside {} wght axis [{:.0}, {:.0}]",
                    face.family, axis.minimum, axis.maximum
                ));
            }
            None if weight == 400.0 => font.axes().location(Vec::<(&str, f32)>::new()),
            None => {
                return Err(format!(
                    "font {} cannot replay non-regular render key {render_font_key}",
                    face.family
                ));
            }
        };
        let Some(glyph) = font.outline_glyphs().get(GlyphId::new(glyph_id)) else {
            return Err(format!("font {} has no glyph id {glyph_id}", face.family));
        };
        let mut pen = TinySkiaOutlinePen::new(
            font_size / face.units_per_em as f32,
            origin_x,
            origin_y,
        );
        glyph
            .draw(
                skrifa::outline::DrawSettings::unhinted(
                    Size::unscaled(),
                    LocationRef::new(location.coords()),
                ),
                &mut pen,
            )
            .map_err(|error| format!("glyph outline replay failed for {glyph_id}: {error:?}"))?;
        if let Some(path) = pen.path.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color(color);
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::identity(),
                None,
            );
        }
        Ok(())
    }
}

struct TinySkiaOutlinePen {
    path: tiny_skia::PathBuilder,
    scale: f32,
    origin_x: f32,
    origin_y: f32,
}

impl TinySkiaOutlinePen {
    fn new(scale: f32, origin_x: f32, origin_y: f32) -> Self {
        Self {
            path: tiny_skia::PathBuilder::new(),
            scale,
            origin_x,
            origin_y,
        }
    }

    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.origin_x + x * self.scale, self.origin_y - y * self.scale)
    }
}

impl OutlinePen for TinySkiaOutlinePen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.point(cx0, cy0);
        let (x, y) = self.point(x, y);
        self.path.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.point(cx0, cy0);
        let (cx1, cy1) = self.point(cx1, cy1);
        let (x, y) = self.point(x, y);
        self.path.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

impl DemoFontFace {
    fn load(key: &'static str, family: &'static str, bytes: &'static [u8]) -> Result<Self, String> {
        SkrifaFontRef::new(bytes).map_err(|_| format!("font decode failed: {family}"))?;
        let units_per_em = units_per_em(bytes)
            .ok_or_else(|| format!("font has no valid units-per-em: {family}"))?;
        let metrics = table_metrics(bytes)
            .ok_or_else(|| format!("font has no readable hhea metrics: {family}"))?;
        Ok(Self {
            key,
            family,
            bytes,
            units_per_em,
            metrics,
            weight_axis: weight_axis(bytes),
        })
    }

    fn weight_for(&self, requested: i32) -> Result<f32, String> {
        let requested = requested as f32;
        match self.weight_axis {
            Some(axis) if requested >= axis.minimum && requested <= axis.maximum => Ok(requested),
            Some(axis) => Err(format!(
                "font weight {requested} is outside {} wght axis [{:.0}, {:.0}]",
                self.family, axis.minimum, axis.maximum
            )),
            None if requested == 400.0 => Ok(requested),
            None => Err(format!(
                "font {} has no wght axis for requested weight {requested}",
                self.family
            )),
        }
    }

    fn shape(&self, input: &ShapingInput, features: &[String]) -> RawShaping {
        let weight = self
            .weight_for(input.style.font_weight)
            .unwrap_or_else(|error| panic!("paragraph-demo shaping failed: {error}"));
        let font = HarfBuzzFontRef::new(self.bytes)
            .unwrap_or_else(|_| panic!("paragraph-demo shaping failed: {} cannot decode", self.family));
        let data = ShaperData::new(&font);
        let instance = self.weight_axis.map(|_| {
            harfrust::ShaperInstance::from_variations(
                &font,
                [harfrust::Variation {
                    tag: Tag::new(b"wght"),
                    value: weight,
                }],
            )
        });
        let shaper = data.shaper(&font).instance(instance.as_ref()).build();
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(&input.display_text);
        buffer.guess_segment_properties();
        buffer.set_direction(Direction::LeftToRight);
        buffer.set_language(input.style.locale.parse().unwrap_or_else(|_| "c".parse().unwrap()));
        let feature_values: Vec<_> = features
            .iter()
            .map(|feature| Feature::new(Tag::new(&tag_bytes(feature)), feature_value(feature), ..))
            .collect();
        let output = shaper.shape(
            buffer,
            harfrust::ShapeOptions::new().features(&feature_values),
        );
        let scale = input.style.font_size / self.units_per_em as f32;
        let mut pen_x = 0_i64;
        let mut glyphs = Vec::with_capacity(output.len());
        for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
            let x = (pen_x + i64::from(position.x_offset)) as f32 * scale;
            let y = -position.y_offset as f32 * scale;
            let advance = position.x_advance as f32 * scale;
            glyphs.push(RawGlyph {
                id: info.glyph_id,
                cluster: utf8_offset_to_utf16(&input.display_text, info.cluster)
                    .unwrap_or_else(|| panic!("paragraph-demo received invalid HarfRust cluster {}", info.cluster)),
                advance,
                x,
                y,
                bounds: self.glyph_bounds(info.glyph_id, weight, scale),
            });
            pen_x += i64::from(position.x_advance);
        }
        RawShaping {
            glyphs,
            advance: pen_x as f32 * scale,
        }
    }

    fn glyph_bounds(&self, glyph_id: u32, weight: f32, scale: f32) -> Option<Rect> {
        let font = SkrifaFontRef::new(self.bytes).ok()?;
        let location = match self.weight_axis {
            Some(_) => font.axes().location([("wght", weight)]),
            None => font.axes().location(Vec::<(&str, f32)>::new()),
        };
        let glyph = font.outline_glyphs().get(GlyphId::new(glyph_id))?;
        let mut pen = ControlBoundsPen::new();
        glyph
            .draw(
                skrifa::outline::DrawSettings::unhinted(
                    Size::unscaled(),
                    LocationRef::new(location.coords()),
                ),
                &mut pen,
            )
            .ok()?;
        let bounds = pen.bounding_box()?;
        Some(Rect {
            left: bounds.x_min * scale,
            top: -bounds.y_max * scale,
            right: bounds.x_max * scale,
            bottom: -bounds.y_min * scale,
        })
    }
}

impl FallbackResolver for DemoFontCatalog {
    fn resolve(&self, _text: &str, range: TextRange, request: &FontRequest) -> FontDecision {
        let face = self.face_for_role(request.role);
        FontDecision {
            range,
            candidate: FontCandidate {
                key: face.key.to_owned(),
                family: face.family.to_owned(),
                role: request.role,
            },
            role: request.role,
            reason: format!("ParagraphDemoControlledFontCatalog:{}", face.family),
        }
    }
}

impl FontMetricsResolver for DemoFontCatalog {
    fn resolve(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        let face = self.face_for_key(&request.font_key);
        let weight = face
            .weight_for(request.font_weight)
            .unwrap_or_else(|error| panic!("paragraph-demo metrics failed: {error}"));
        let _ = weight;
        let scale = request.font_size / face.units_per_em as f32;
        RawFontMetrics {
            ascent: face.metrics.ascent as f32 * scale,
            descent: -(face.metrics.descent as f32) * scale,
            leading: face.metrics.leading as f32 * scale,
            source: FontMetricSource::RawTables,
            typo_ascent: face.metrics.typo_ascent.map(|value| value as f32 * scale),
            typo_descent: face.metrics.typo_descent.map(|value| -(value as f32) * scale),
        }
    }
}

impl TextShaper for DemoFontCatalog {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let face = self.face_for_key(&input.font_decision.candidate.key);
        let features = input.open_type_features.clone();
        let shaped = face.shape(input, &features);
        let halt = if input.font_decision.role == FontRole::CjkPunctuation
            && !features.iter().any(|feature| feature.starts_with("halt"))
        {
            let mut halt_features = features.clone();
            halt_features.push("halt=1".to_owned());
            Some(face.shape(input, &halt_features))
        } else {
            None
        };
        let exact_key = Self::resolved_font_key(
            face,
            face.weight_for(input.style.font_weight)
                .unwrap_or_else(|error| panic!("paragraph-demo shaping failed: {error}")),
        );
        let halt_matches = halt
            .as_ref()
            .filter(|candidate| candidate.glyphs.len() == shaped.glyphs.len());
        let clusters = clustered_glyphs(input, &shaped, halt_matches, &exact_key);
        let glyphs: Vec<_> = clusters
            .iter()
            .flat_map(|cluster| cluster.glyphs.iter().cloned())
            .collect();
        let missing_glyphs = glyphs.iter().filter(|glyph| glyph.id == 0).count() as i32;
        let glyphs_without_ink_bounds = glyphs
            .iter()
            .filter(|glyph| glyph.bounds.is_none())
            .count() as i32;
        let source_text = source_slice(&input.text, input.range).to_owned();
        let glyph_count = glyphs.len() as i32;
        let decision = ShapingDecisionInfo::builder(
            input.range,
            source_text.clone(),
            input.display_text.clone(),
            face.key.to_owned(),
            glyph_count,
            shaped.advance,
            format!("{:?}", ShapingSource::HarfBuzz),
            "ParagraphDemoControlledFontCatalog:harfrust".to_owned(),
        )
        .glyphs_without_ink_bounds(glyphs_without_ink_bounds)
        .missing_glyphs(missing_glyphs)
        .resolved_face(Some(exact_key))
        .language(Some(input.style.locale.clone()))
        .feature_evidence((!features.is_empty()).then(|| features.join(",")))
        .build();
        ShapingResult::with_decisions(
            clusters
                .iter()
                .map(|cluster| {
                    Cluster::with_display_text(
                        cluster.range,
                        source_slice(&input.text, cluster.range).to_owned(),
                        utf16_slice(&input.display_text, cluster.display_start, cluster.display_end)
                            .to_owned(),
                        face.key.to_owned(),
                        cluster.advance,
                    )
                })
                .collect(),
            vec![GlyphRun::with_open_type_features(
                input.range,
                face.key.to_owned(),
                glyphs,
                shaped.advance,
                features,
            )],
            vec![decision],
        )
    }
}

struct RawShaping {
    glyphs: Vec<RawGlyph>,
    advance: f32,
}

#[derive(Clone)]
struct RawGlyph {
    id: u32,
    cluster: i32,
    advance: f32,
    x: f32,
    y: f32,
    bounds: Option<Rect>,
}

struct ClusteredGlyphs {
    range: TextRange,
    display_start: i32,
    display_end: i32,
    advance: f32,
    glyphs: Vec<Glyph>,
}

fn clustered_glyphs(
    input: &ShapingInput,
    shaped: &RawShaping,
    halt: Option<&RawShaping>,
    render_font_key: &str,
) -> Vec<ClusteredGlyphs> {
    let display_length = input.display_text.encode_utf16().count() as i32;
    if input.range.length() != display_length {
        return vec![clustered_glyph_group(
            input.range,
            0,
            display_length,
            &shaped.glyphs,
            halt.map(|candidate| candidate.glyphs.as_slice()),
            0.0,
            render_font_key,
        )];
    }
    let mut starts: Vec<_> = shaped.glyphs.iter().map(|glyph| glyph.cluster).collect();
    starts.sort_unstable();
    starts.dedup();
    if starts.is_empty() {
        return vec![ClusteredGlyphs {
            range: input.range,
            display_start: 0,
            display_end: display_length,
            advance: shaped.advance,
            glyphs: Vec::new(),
        }];
    }
    starts.push(display_length);
    starts
        .windows(2)
        .scan(0.0, |pen_x, pair| {
            let start = pair[0];
            let end = pair[1];
            let glyphs: Vec<_> = shaped
                .glyphs
                .iter()
                .filter(|glyph| glyph.cluster == start)
                .cloned()
                .collect();
            let cluster = clustered_glyph_group(
                TextRange::new(input.range.start() + start, input.range.start() + end),
                start,
                end,
                &glyphs,
                halt.map(|candidate| candidate.glyphs.as_slice()),
                *pen_x,
                render_font_key,
            );
            *pen_x += cluster.advance;
            Some(cluster)
        })
        .collect()
}

fn clustered_glyph_group(
    range: TextRange,
    display_start: i32,
    display_end: i32,
    glyphs: &[RawGlyph],
    halt: Option<&[RawGlyph]>,
    pen_x: f32,
    render_font_key: &str,
) -> ClusteredGlyphs {
    let advance = glyphs.iter().map(|glyph| glyph.advance).sum();
    let glyphs = glyphs
        .iter()
        .map(|glyph| {
            let halt_glyph = halt.and_then(|candidate| {
                candidate
                    .iter()
                    .find(|value| value.cluster == glyph.cluster && value.id == glyph.id)
            });
            Glyph::builder(glyph.id, range, glyph.advance)
                .x(glyph.x - pen_x)
                .y(glyph.y)
                .render_font_key(Some(render_font_key.to_owned()))
                .bounds(glyph.bounds)
                .halt_advance(halt_glyph.map(|value| value.advance))
                .halt_placement_x(halt_glyph.map(|value| value.x - glyph.x))
                .build()
        })
        .collect();
    ClusteredGlyphs {
        range,
        display_start,
        display_end,
        advance,
        glyphs,
    }
}

fn source_slice(text: &str, range: TextRange) -> &str {
    let start = tiqian::org::tiqian::core::TextIndex::utf16_offset_to_utf8_byte_index(text, range.start())
        .expect("paragraph-demo shape range start must be a scalar boundary");
    let end = tiqian::org::tiqian::core::TextIndex::utf16_offset_to_utf8_byte_index(text, range.end())
        .expect("paragraph-demo shape range end must be a scalar boundary");
    &text[start..end]
}

fn utf16_slice(text: &str, start: i32, end: i32) -> &str {
    let start = tiqian::org::tiqian::core::TextIndex::utf16_offset_to_utf8_byte_index(text, start)
        .expect("paragraph-demo display cluster start must be a scalar boundary");
    let end = tiqian::org::tiqian::core::TextIndex::utf16_offset_to_utf8_byte_index(text, end)
        .expect("paragraph-demo display cluster end must be a scalar boundary");
    &text[start..end]
}

fn utf8_offset_to_utf16(text: &str, byte_offset: u32) -> Option<i32> {
    let byte_offset = usize::try_from(byte_offset).ok()?;
    text.get(..byte_offset)?;
    if !text.is_char_boundary(byte_offset) {
        return None;
    }
    Some(text[..byte_offset].encode_utf16().count() as i32)
}

fn tag_bytes(feature: &str) -> [u8; 4] {
    let mut tag = [b' '; 4];
    for (slot, byte) in tag.iter_mut().zip(feature.bytes()) {
        if byte == b'=' {
            break;
        }
        *slot = byte;
    }
    tag
}

fn feature_value(feature: &str) -> u32 {
    feature
        .split_once('=')
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(1)
}

fn table<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = usize::from(u16_at(bytes, 4)?);
    for index in 0..count {
        let record = 12 + index * 16;
        if bytes.get(record..record + 4)? != tag {
            continue;
        }
        let offset = usize::try_from(u32_at(bytes, record + 8)?).ok()?;
        let length = usize::try_from(u32_at(bytes, record + 12)?).ok()?;
        return bytes.get(offset..offset.checked_add(length)?);
    }
    None
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn i16_at(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn i32_at(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn units_per_em(bytes: &[u8]) -> Option<u32> {
    let value = u16_at(table(bytes, b"head")?, 18)?;
    (16..=16384).contains(&value).then_some(u32::from(value))
}

fn table_metrics(bytes: &[u8]) -> Option<TableMetrics> {
    let hhea = table(bytes, b"hhea")?;
    let os2 = table(bytes, b"OS/2");
    Some(TableMetrics {
        ascent: i16_at(hhea, 4)?,
        descent: i16_at(hhea, 6)?,
        leading: i16_at(hhea, 8)?,
        typo_ascent: os2.and_then(|value| i16_at(value, 68)),
        typo_descent: os2.and_then(|value| i16_at(value, 70)),
    })
}

fn weight_axis(bytes: &[u8]) -> Option<WeightAxis> {
    let fvar = table(bytes, b"fvar")?;
    if u16_at(fvar, 0)? != 1 || fvar.len() < 16 || u16_at(fvar, 10)? != 20 {
        return None;
    }
    let data_offset = usize::from(u16_at(fvar, 4)?);
    let count = usize::from(u16_at(fvar, 8)?);
    for index in 0..count {
        let axis = data_offset.checked_add(index.checked_mul(20)?)?;
        if fvar.get(axis..axis + 4)? != b"wght" {
            continue;
        }
        let minimum = fixed_at(fvar, axis + 4)?;
        let maximum = fixed_at(fvar, axis + 12)?;
        return Some(WeightAxis {
            minimum: minimum.min(maximum),
            maximum: minimum.max(maximum),
        });
    }
    None
}

fn fixed_at(bytes: &[u8], offset: usize) -> Option<f32> {
    Some(i32_at(bytes, offset)? as f32 / 65536.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiqian::org::tiqian::core::TextModel::TextStyle;
    use tiqian::org::tiqian::shaping::TextShaper::ShapingInput;

    #[test]
    fn controlled_faces_shape_and_measure_their_own_roles() {
        let catalog = DemoFontCatalog::load().unwrap();
        catalog.validate_demo_faces().unwrap();
        let cjk = FallbackResolver::resolve(
            &catalog,
            "中文",
            TextRange::new(0, 2),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::CjkText,
            },
        );
        let latin = FallbackResolver::resolve(
            &catalog,
            "Latin",
            TextRange::new(0, 5),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::LatinText,
            },
        );
        assert_eq!(cjk.candidate.key, CJK_FONT_KEY);
        assert_eq!(latin.candidate.key, LATIN_FONT_KEY);
        let result = catalog.shape(&ShapingInput::new(
            "中文".to_owned(),
            TextRange::new(0, 2),
            TextStyle::default(),
            cjk,
        ));
        assert!(result.glyph_runs[0].glyphs.iter().all(|glyph| glyph.id != 0));
        assert!(result.glyph_runs[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.render_font_key.is_some() && glyph.bounds.is_some()));
        let metrics = FontMetricsResolver::resolve(&catalog, &FontMetricsRequest::new(
            CJK_FONT_KEY.to_owned(),
            16.0,
            FontRole::CjkText,
            "zh-Hans".to_owned(),
        ));
        assert!(metrics.ascent > 0.0 && metrics.descent > 0.0);
        assert!(metrics.typo_ascent.is_some() && metrics.typo_descent.is_some());
    }

    #[test]
    fn cjk_feature_shapes_preserve_replay_evidence() {
        let catalog = DemoFontCatalog::load().unwrap();
        let punctuation = FallbackResolver::resolve(
            &catalog,
            "（",
            TextRange::new(0, 1),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::CjkPunctuation,
            },
        );
        let halt = catalog.shape(
            &ShapingInput::builder(
                "（".to_owned(),
                TextRange::new(0, 1),
                TextStyle::default(),
                punctuation,
            )
            .open_type_features(vec!["fwid=1".to_owned()])
            .build(),
        );
        assert_eq!(halt.glyph_runs[0].open_type_features, vec!["fwid=1"]);
        assert_eq!(halt.decisions[0].feature_evidence.as_deref(), Some("fwid=1"));
        assert!(halt.glyph_runs[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.halt_advance.is_some()));

        let bopomofo = FallbackResolver::resolve(
            &catalog,
            "ㄅ",
            TextRange::new(0, 1),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::CjkText,
            },
        );
        let vertical = catalog.shape(
            &ShapingInput::builder(
                "ㄅ".to_owned(),
                TextRange::new(0, 1),
                TextStyle::default(),
                bopomofo,
            )
            .open_type_features(vec!["vert=1".to_owned()])
            .build(),
        );
        assert_eq!(vertical.glyph_runs[0].open_type_features, vec!["vert=1"]);
        assert_eq!(vertical.decisions[0].feature_evidence.as_deref(), Some("vert=1"));
        assert!(vertical.glyph_runs[0].glyphs.iter().all(|glyph| glyph.id != 0));
    }

    #[test]
    fn harfrust_clusters_are_remapped_to_utf16_source_ranges() {
        let catalog = DemoFontCatalog::load().unwrap();
        let decision = FallbackResolver::resolve(
            &catalog,
            "a😀b",
            TextRange::new(0, 4),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::LatinText,
            },
        );
        let shaped = catalog.shape(&ShapingInput::new(
            "a😀b".to_owned(),
            TextRange::new(0, 4),
            TextStyle::default(),
            decision,
        ));
        assert_eq!(shaped.clusters[0].range, TextRange::new(0, 1));
        assert_eq!(shaped.clusters[1].range, TextRange::new(1, 3));
        assert_eq!(shaped.clusters[2].range, TextRange::new(3, 4));
        assert!(shaped
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| shaped.clusters.iter().any(|cluster| cluster.range == glyph.cluster_range)));
    }

    #[test]
    fn catalog_replaces_the_engine_stub_font_path() {
        use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
        use tiqian::org::tiqian::core::TextModel::{LayoutInput, TiqianTextContent};
        use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
            ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
        };

        let catalog = DemoFontCatalog::load().unwrap();
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.fallback_resolver = Box::new(catalog.clone());
        engine.font_metrics_resolver = Box::new(catalog.clone());
        engine.text_shaper = Box::new(catalog);
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new("中文（English）".to_owned()),
                LayoutConstraints::with_defaults(160.0),
            )
            .build(),
        );

        assert!(result
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.render_font_key.is_some() && glyph.id != 0));
        assert!(result.debug.metric_decisions.iter().all(|decision| {
            decision.raw_source == "RawTables"
        }));
    }

    #[test]
    fn render_font_key_replays_a_shaped_glyph_outline() {
        let catalog = DemoFontCatalog::load().unwrap();
        let decision = FallbackResolver::resolve(
            &catalog,
            "中",
            TextRange::new(0, 1),
            &FontRequest {
                preferred_families: Vec::new(),
                locale: "zh-Hans".to_owned(),
                role: FontRole::CjkText,
            },
        );
        let shaped = catalog.shape(&ShapingInput::new(
            "中".to_owned(),
            TextRange::new(0, 1),
            TextStyle::default(),
            decision,
        ));
        let glyph = &shaped.glyph_runs[0].glyphs[0];
        let mut pixmap = tiny_skia::Pixmap::new(48, 48).unwrap();
        catalog
            .paint_glyph(
                &mut pixmap,
                glyph.render_font_key.as_deref().unwrap(),
                glyph.id,
                16.0,
                12.0 + glyph.x,
                28.0 + glyph.y,
                tiny_skia::Color::BLACK,
            )
            .unwrap();
        assert!(pixmap.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
