//! Development-only fixture runner. It accepts one JSON fixture on stdin and emits Kotlin's layout dump.

use std::io::{self, Read};
use tiqian::common::HashSet;

use serde::Deserialize;
use tiqian::clreq::ClreqProfile::{
    BuiltInClreqProfileResolver, ClreqProfileResolver, KinsokuLevel, KinsokuMode,
};
use tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{
    DecorationKind, DecorationSpan, InlineAttachment, LastLineAlignment, LayoutInput,
    LayoutProfileId, LineBreakPolicy, LineBreakSpan, LineLengthGrid,
    MeasureAdaptiveFirstLineIndent, ParagraphStyle, RubyKind, RubyLineHeightMode, RubySpan,
    TextStyle, TiqianTextContent, WritingMode,
};
use tiqian::core::Units::Ic;
use tiqian::layout::LineBreaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::ParagraphDpLineBreaker::ParagraphDpLineBreaker;
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::EnglishHyphenation::english_hyphenation;
use tiqian::linebreak::Hyphenation::{Hyphenator, NoHyphenator};

static NO_HYPHENATOR: NoHyphenator = NoHyphenator;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureWire {
    id: String,
    use_english_hyphenation: bool,
    pin_basic_no_hang: bool,
    input: InputWire,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputWire {
    content: ContentWire,
    text_style: TextStyleWire,
    paragraph_style: ParagraphStyleWire,
    constraints: ConstraintsWire,
    profile_id: String,
    decorations: Vec<DecorationWire>,
    ruby_spans: Vec<RubyWire>,
    inline_boxes: Vec<serde_json::Value>,
    inline_objects: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentWire {
    text: String,
    source_boundaries: Vec<i32>,
    line_break_spans: Vec<LineBreakWire>,
    auto_space_suppressed_ranges: Vec<RangeWire>,
    spans: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct RangeWire {
    start: i32,
    end: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextStyleWire {
    font_families: Vec<String>,
    font_size: f32,
    locale: String,
    font_weight: i32,
    italic: bool,
    baseline_shift: f32,
    inline_attachment: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParagraphStyleWire {
    last_line_alignment: String,
    writing_mode: String,
    line_height: Option<f32>,
    first_line_indent: Option<f32>,
    block_indent: f32,
    first_line_indent_policy: FirstLineIndentPolicyWire,
    line_length_grid: GridWire,
    ruby_line_height_mode: String,
    inline_object_minimum_clearance_em: f32,
    emphasis_dot_gap_em: f32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FirstLineIndentPolicyWire {
    short_below_em: f32,
    short_em: f32,
    long_em: f32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GridWire {
    enabled: bool,
    body_alignment: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConstraintsWire {
    max_width: f32,
    max_height: Option<f32>,
    max_lines: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LineBreakWire {
    range: RangeWire,
    policy: String,
}
#[derive(Deserialize)]
struct DecorationWire {
    range: RangeWire,
    kind: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RubyWire {
    base_range: RangeWire,
    text: String,
    font_families: Vec<String>,
    kind: String,
    locale: Option<String>,
}

struct FixtureProfileResolver {
    pin_basic_no_hang: bool,
}
impl ClreqProfileResolver for FixtureProfileResolver {
    fn resolve(&self, profile_id: &LayoutProfileId) -> tiqian::clreq::ClreqProfile::ClreqProfile {
        let mut profile = BuiltInClreqProfileResolver.resolve(profile_id);
        if self.pin_basic_no_hang {
            profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        }
        profile
    }
}

fn main() {
    let mut json = String::new();
    io::stdin()
        .read_to_string(&mut json)
        .expect("read fixture JSON from stdin");
    let wire: FixtureWire = serde_json::from_str(&json).expect("parse fixture JSON");
    print!(
        "fixture: {}\ntext: {}\nmaxWidth: {}\n",
        wire.id,
        escape(&wire.input.content.text),
        fmt(wire.input.constraints.max_width)
    );
    for label in ["greedy", "lookahead", "paragraph-dp"] {
        let result = layout(&wire, label);
        print!("{}", dump_result(label, &result));
    }
}

fn layout(wire: &FixtureWire, breaker: &str) -> tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = match breaker {
        "greedy" => Box::new(GreedyLineBreaker::default()),
        "lookahead" => Box::new(LookaheadLineBreaker::default()),
        "paragraph-dp" => Box::new(ParagraphDpLineBreaker::default()),
        _ => unreachable!(),
    };
    engine.hyphenator = if wire.use_english_hyphenation {
        english_hyphenation::en_us()
    } else {
        &NO_HYPHENATOR as &dyn Hyphenator
    };
    engine.clreq_profile_resolver = Box::new(FixtureProfileResolver {
        pin_basic_no_hang: wire.pin_basic_no_hang,
    });
    engine.layout(to_input(&wire.input))
}

fn to_input(wire: &InputWire) -> LayoutInput {
    assert!(
        wire.content.spans.is_empty(),
        "fixture runner does not yet support non-empty text spans"
    );
    assert!(
        wire.inline_boxes.is_empty(),
        "fixture runner does not yet support inline boxes"
    );
    assert!(
        wire.inline_objects.is_empty(),
        "fixture runner does not yet support inline objects"
    );
    let content = TiqianTextContent::builder(Text::from(wire.content.text.clone()))
        .source_boundaries(
            wire.content
                .source_boundaries
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        )
        .line_break_spans(
            wire.content
                .line_break_spans
                .iter()
                .map(|span| LineBreakSpan {
                    range: range(&span.range),
                    policy: policy(&span.policy),
                })
                .collect(),
        )
        .auto_space_suppressed_ranges(
            wire.content
                .auto_space_suppressed_ranges
                .iter()
                .map(range)
                .collect(),
        )
        .build();
    let text_style = TextStyle {
        font_families: wire.text_style.font_families.clone(),
        font_size: wire.text_style.font_size,
        locale: wire.text_style.locale.clone(),
        font_weight: wire.text_style.font_weight,
        italic: wire.text_style.italic,
        baseline_shift: wire.text_style.baseline_shift,
        inline_attachment: if wire.text_style.inline_attachment == "Previous" {
            InlineAttachment::Previous
        } else {
            InlineAttachment::None
        },
    };
    let paragraph_style = ParagraphStyle {
        last_line_alignment: last_line_alignment(&wire.paragraph_style.last_line_alignment),
        writing_mode: writing_mode(&wire.paragraph_style.writing_mode),
        line_height: wire.paragraph_style.line_height,
        first_line_indent: wire
            .paragraph_style
            .first_line_indent
            .map(|count| Ic { count }),
        block_indent: Ic {
            count: wire.paragraph_style.block_indent,
        },
        first_line_indent_policy: MeasureAdaptiveFirstLineIndent::new(
            wire.paragraph_style.first_line_indent_policy.short_below_em,
            wire.paragraph_style.first_line_indent_policy.short_em,
            wire.paragraph_style.first_line_indent_policy.long_em,
        ),
        line_length_grid: LineLengthGrid::new(
            wire.paragraph_style.line_length_grid.enabled,
            alignment(
                wire.paragraph_style
                    .line_length_grid
                    .body_alignment
                    .as_deref(),
            ),
        ),
        ruby_line_height_mode: ruby_line_height_mode(&wire.paragraph_style.ruby_line_height_mode),
        inline_object_minimum_clearance_em: wire.paragraph_style.inline_object_minimum_clearance_em,
        emphasis_dot_gap_em: wire.paragraph_style.emphasis_dot_gap_em,
    };
    let constraints = LayoutConstraints::new(
        wire.constraints.max_width,
        wire.constraints.max_height.unwrap_or(f32::INFINITY),
        wire.constraints.max_lines,
    );
    LayoutInput::builder(content, constraints)
        .text_style(text_style)
        .paragraph_style(paragraph_style)
        .profile_id(LayoutProfileId {
            value: wire.profile_id.clone(),
        })
        .decorations(
            wire.decorations
                .iter()
                .map(|span| DecorationSpan {
                    range: range(&span.range),
                    kind: decoration_kind(&span.kind),
                })
                .collect(),
        )
        .ruby_spans(
            wire.ruby_spans
                .iter()
                .map(|span| RubySpan {
                    base_range: range(&span.base_range),
                    text: Text::from(span.text.clone()),
                    font_families: span.font_families.clone(),
                    kind: ruby_kind(&span.kind),
                    locale: span.locale.clone(),
                })
                .collect(),
        )
        .build()
}

fn range(wire: &RangeWire) -> TextRange {
    TextRange::new(wire.start, wire.end)
}
fn policy(value: &str) -> LineBreakPolicy {
    match value {
        "ProgressiveTechnical" => LineBreakPolicy::ProgressiveTechnical,
        _ => panic!("unsupported line-break policy {value}"),
    }
}
fn decoration_kind(value: &str) -> DecorationKind {
    match value {
        "Emphasis" => DecorationKind::Emphasis,
        "Mourning" => DecorationKind::Mourning,
        "ProperNoun" => DecorationKind::ProperNoun,
        "BookTitle" => DecorationKind::BookTitle,
        _ => panic!("unsupported decoration kind {value}"),
    }
}
fn ruby_kind(value: &str) -> RubyKind {
    if value == "Bopomofo" {
        RubyKind::Bopomofo
    } else {
        RubyKind::Pinyin
    }
}
fn ruby_line_height_mode(value: &str) -> RubyLineHeightMode {
    if value == "UniformParagraph" {
        RubyLineHeightMode::UniformParagraph
    } else {
        RubyLineHeightMode::PerLine
    }
}
fn alignment(value: Option<&str>) -> Option<tiqian::core::TextModel::LastLineAlignment> {
    match value {
        None | Some("Start") => value.map(|_| tiqian::core::TextModel::LastLineAlignment::Start),
        Some("Center") => Some(tiqian::core::TextModel::LastLineAlignment::Center),
        Some("End") => Some(tiqian::core::TextModel::LastLineAlignment::End),
        Some(other) => panic!("unsupported alignment {other}"),
    }
}
fn last_line_alignment(value: &str) -> LastLineAlignment {
    match value {
        "Start" => LastLineAlignment::Start,
        "Center" => LastLineAlignment::Center,
        "End" => LastLineAlignment::End,
        _ => panic!("unsupported last-line alignment {value}"),
    }
}
fn writing_mode(value: &str) -> WritingMode {
    match value {
        "HorizontalTb" => WritingMode::HorizontalTb,
        "VerticalRl" => WritingMode::VerticalRl,
        _ => panic!("unsupported writing mode {value}"),
    }
}

fn dump_result(label: &str, result: &tiqian::core::LayoutModel::LayoutResult) -> String {
    let mut out = format!(
        "== {label} ==\nsize {}x{}\n",
        fmt(result.size.width),
        fmt(result.size.height)
    );
    if let Some(grid) = &result.debug.line_length_grid_decision
        && grid.enabled
        && grid.slack > 0.0
    {
        out.push_str(&format!(
            "grid container={} measure={}({}字) slack={} body={}@{}\n",
            fmt(grid.container_width),
            fmt(grid.measure),
            grid.cells,
            fmt(grid.slack),
            grid.body_alignment,
            fmt(grid.body_offset)
        ));
    }
    if let Some(indent) = &result.debug.first_line_indent_decision
        && indent.source != "Explicit"
    {
        out.push_str(&format!(
            "firstindent {}字 measure={}字 threshold={}字 {}\n",
            fmt(indent.resolved_em),
            fmt(indent.measure_em),
            fmt(indent.threshold_em),
            indent.source
        ));
    }
    if let Some(kinsoku) = &result.debug.kinsoku_decision {
        out.push_str(&format!(
            "kinsoku measure={}字 level={} hang={} reason={}\n",
            fmt(kinsoku.measure_em),
            kinsoku.level,
            kinsoku.hanging,
            kinsoku.reason
        ));
    }
    for decision in &result.debug.contextual_kinsoku_decisions {
        out.push_str(&format!(
            "context-kinsoku {}-{} source='{}' cluster={} forbid={} reason={}{}\n",
            decision.range.start(),
            decision.range.end(),
            escape(&decision.source_text),
            decision.cluster_index,
            decision.forbidden_position,
            decision.reason,
            decision
                .impossible_measure_fallback
                .as_ref()
                .map(|value| format!(" fallback={value}"))
                .unwrap_or_default()
        ));
    }
    for decision in &result.debug.break_opportunity_decisions {
        out.push_str(&format!(
            "break-opportunity {}-{} source='{}' offsets={}{} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(&decision.source_text),
            decision
                .break_offsets
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(","),
            decision
                .tier
                .as_ref()
                .map(|value| format!(" tier={value}"))
                .unwrap_or_default(),
            decision.reason
        ));
    }
    for decision in &result.debug.emergency_tracking_eligibility_decisions {
        out.push_str(&format!(
            "tracking-eligibility {}-{} source='{}' reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(&decision.source_text),
            decision.reason
        ));
    }
    for attachment in &result.debug.inline_object_punctuation_attachment_decisions {
        out.push_str(&format!("inline-object-punctuation {}-{} separator={}-{} punctuation={}-{} source='{}' collapsed={} protected={}-{} reason={}\n", attachment.object_range.start(), attachment.object_range.end(), attachment.separator_range.start(), attachment.separator_range.end(), attachment.punctuation_range.start(), attachment.punctuation_range.end(), escape(&attachment.punctuation_text), fmt(attachment.collapsed_advance), attachment.protected_range.start(), attachment.protected_range.end(), attachment.reason));
    }
    for (index, line) in result.lines.iter().enumerate() {
        let decision = result.debug.line_decisions.get(index);
        let repair = decision
            .and_then(|value| value.repair_decision.as_ref())
            .map_or("-".to_owned(), |value| {
                format!(
                    "{}({} shrink={})",
                    value.kind,
                    value.reason_code,
                    fmt(value.shrink)
                )
            });
        let candidates = decision
            .map(|value| {
                value
                    .repair_candidates
                    .iter()
                    .map(|candidate| {
                        format!(
                            "{}{}",
                            candidate.kind,
                            if candidate.accepted { "+" } else { "-" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "-".to_owned());
        let justify = result
            .debug
            .justification_decisions
            .iter()
            .find(|value| value.line_range == line.range)
            .map(|value| {
                let allocations = value
                    .allocations
                    .iter()
                    .map(|allocation| {
                        format!(
                            "{}@{}+{}{}",
                            allocation.kind,
                            allocation.cluster_range.start(),
                            fmt(allocation.delta),
                            if allocation.reason == allocation.kind {
                                String::new()
                            } else {
                                format!("({})", allocation.reason)
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "deficit={}->{}{}",
                    fmt(value.deficit_before),
                    fmt(value.deficit_after),
                    if allocations.is_empty() {
                        String::new()
                    } else {
                        format!(" {allocations}")
                    }
                )
            })
            .unwrap_or_else(|| "-".to_owned());
        out.push_str(&format!("line[{index}] {}-{} {}{}natural={} adjusted={} visual={} repair={} candidates={} justify={}\n", line.range.start(), line.range.end(), if line.indent > 0.0 { format!("indent={} ", fmt(line.indent)) } else { String::new() }, if line.hyphen_advance > 0.0 { format!("hyphen={} ", fmt(line.hyphen_advance)) } else { String::new() }, fmt(line.natural_width), fmt(line.adjusted_width), fmt(line.visual_width), repair, candidates, justify));
    }
    for cluster in &result.clusters {
        out.push_str(&format!(
            "cluster {}-{} '{}' adv={}{}\n",
            cluster.range.start(),
            cluster.range.end(),
            cluster.display_text,
            fmt(cluster.advance),
            if cluster.glyph_inline_shift != 0.0 {
                format!(" glyphShift={}", fmt(cluster.glyph_inline_shift))
            } else {
                String::new()
            }
        ));
    }
    for font in &result.debug.font_decisions {
        out.push_str(&format!(
            "font {}-{} role={} key={} display='{}' sub={}\n",
            font.range.start(),
            font.range.end(),
            font.role,
            font.font_key,
            font.display_text,
            font.substitution_reason
        ));
    }
    for role in &result.debug.role_overrides {
        out.push_str(&format!(
            "role-override {}-{} source='{}' {}->{} policy={} reason={}\n",
            role.range.start(),
            role.range.end(),
            escape(&role.source_text),
            role.original_role,
            role.overridden_role,
            role.source,
            role.reason
        ));
    }
    for punctuation in &result.debug.punctuation_decisions {
        out.push_str(&format!("punct {}-{} '{}' class={} adv={} body={} lead={} trail={} {}anchor={} source={}{}{}{}{}{}{}\n", punctuation.range.start(), punctuation.range.end(), punctuation.ch, punctuation.punctuation_class, fmt(punctuation.advance), fmt(punctuation.body_width), fmt(punctuation.leading_glue_natural), fmt(punctuation.trailing_glue_natural), if punctuation.leading_glue_initially_consumed != 0.0 || punctuation.trailing_glue_initially_consumed != 0.0 { format!("initial={}/{} ", fmt(punctuation.leading_glue_initially_consumed), fmt(punctuation.trailing_glue_initially_consumed)) } else { String::new() }, punctuation.anchor, punctuation.geometry_source, if punctuation.advance_expansion != 0.0 { format!(" expand={}", fmt(punctuation.advance_expansion)) } else { String::new() }, if punctuation.glyph_inline_shift != 0.0 { format!(" glyphShift={}", fmt(punctuation.glyph_inline_shift)) } else { String::new() }, punctuation.glyph_placement_reason.as_ref().map(|value| format!(" placement={value}")).unwrap_or_default(), punctuation.halt_advance.map(|value| format!(" halt={}", fmt(value))).unwrap_or_default(), punctuation.ink_bounds_fallback.as_ref().map(|value| format!(" fallback={value}")).unwrap_or_default(), punctuation.halt_validation.as_ref().map(|value| format!(" haltWarn={value}")).unwrap_or_default()));
    }
    for geometry in &result.debug.geometry_decisions {
        out.push_str(&format!(
            "geom {}-{} body={} lead={}/{} trail={}/{} justify={}{}{}{} resolved={}\n",
            geometry.range.start(),
            geometry.range.end(),
            fmt(geometry.body_width),
            fmt(geometry.leading_glue_consumed),
            fmt(geometry.leading_glue_natural),
            fmt(geometry.trailing_glue_consumed),
            fmt(geometry.trailing_glue_natural),
            fmt(geometry.justification_delta),
            if geometry.ruby_spread != 0.0 {
                format!(" ruby={}", fmt(geometry.ruby_spread))
            } else {
                String::new()
            },
            if geometry.glyph_inline_shift != 0.0 {
                format!(" glyphShift={}", fmt(geometry.glyph_inline_shift))
            } else {
                String::new()
            },
            geometry
                .glyph_placement_reason
                .as_ref()
                .map(|value| format!(" placement={value}"))
                .unwrap_or_default(),
            fmt(geometry.resolved_advance)
        ));
    }
    for decision in &result.debug.inline_box_decisions {
        out.push_str(&format!(
            "inline-box {}-{} start={} end={} outer={} clusters={}-{} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            fmt(decision.inline_start),
            fmt(decision.inline_end),
            decision.outer_spacing,
            decision.first_cluster_index,
            decision.last_cluster_index,
            decision.reason
        ));
    }
    for decision in &result.debug.inline_object_decisions {
        let edge =
            |uniform, kind: Option<&str>, natural, target, capacity, prevents, shrink, discard| {
                format!(
                    "{}/{}/{}→{}/{}/{}/{}/{}",
                    if uniform { "stretch" } else { "fixed" },
                    kind.unwrap_or("-"),
                    fmt(natural),
                    fmt(target),
                    fmt(capacity),
                    if prevents { "closed" } else { "natural" },
                    fmt(shrink),
                    fmt(discard)
                )
            };
        out.push_str(&format!("inline-object {}-{} advance={} ascent={} descent={} cluster={} line={} edges={}..{} reason={}\n", decision.range.start(), decision.range.end(), fmt(decision.advance), fmt(decision.ascent), fmt(decision.descent), decision.cluster_index, decision.line_index, edge(decision.leading_uniform_stretch, decision.leading_preferred_stretch_kind.as_deref(), decision.leading_preferred_stretch_natural_width, decision.leading_preferred_stretch_target_width, decision.leading_preferred_stretch_capacity, decision.leading_prevents_line_break, decision.leading_shrink_capacity, decision.leading_line_end_discardable_advance), edge(decision.trailing_uniform_stretch, decision.trailing_preferred_stretch_kind.as_deref(), decision.trailing_preferred_stretch_natural_width, decision.trailing_preferred_stretch_target_width, decision.trailing_preferred_stretch_capacity, decision.trailing_prevents_line_break, decision.trailing_shrink_capacity, decision.trailing_line_end_discardable_advance), decision.reason));
    }
    for decision in &result.debug.spacing_decisions {
        out.push_str(&format!(
            "spacing {}-{} '{}{}' inner={}->{} target={}-{}\n",
            decision.range.start(),
            decision.range.end(),
            decision.left_char,
            decision.right_char,
            fmt(decision.natural_inner_glue),
            fmt(decision.adjusted_inner_glue),
            decision.reduction_target_range.start(),
            decision.reduction_target_range.end()
        ));
    }
    for decision in &result.debug.auto_space_decisions {
        out.push_str(&format!(
            "autospace {}-{} side={} boundary={} reduction={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            decision.side,
            decision.boundary_role,
            fmt(decision.total_reduction)
        ));
    }
    for decision in &result.debug.mandatory_break_decisions {
        out.push_str(&format!(
            "mandatorybreak {}-{} afterCluster={} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            decision.break_after_cluster_index,
            decision.reason
        ));
    }
    for decision in &result.debug.zero_width_break_decisions {
        out.push_str(&format!(
            "zerowidthbreak {}-{} source='{}' cluster={} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(&decision.source_text),
            decision.cluster_index,
            decision.reason
        ));
    }
    for decision in &result.debug.line_edge_trim_decisions {
        out.push_str(&format!(
            "edgetrim {}-{} side={} trim={} reason={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            decision.side,
            fmt(decision.trim_amount),
            decision.reason
        ));
    }
    for decision in &result.debug.decoration_decisions {
        out.push_str(&format!(
            "deco {}-{} '{}' kind={} applied={} anchor={},{} diameter={} reason={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            decision.source_text,
            decision.kind,
            decision.applied,
            fmt(decision.anchor_x),
            fmt(decision.anchor_y),
            fmt(decision.dot_diameter),
            decision.reason
        ));
    }
    if let Some(spacing) = &result.debug.line_spacing_decision {
        out.push_str(&format!(
            "linespacing natural={} requested={} resolved={} floor={} applied={} reason={}\n",
            fmt(spacing.natural_height),
            spacing
                .requested_line_height
                .map(fmt)
                .unwrap_or_else(|| "-".to_owned()),
            fmt(spacing.resolved_height),
            fmt(spacing.spacing_floor),
            spacing.floor_applied,
            spacing.reason
        ));
    }
    if let Some(decision) = &result.debug.ruby_line_height_decision {
        out.push_str(&format!("rubylineheight mode={} base={} face={} ruby={} available={} maxExtra={} extras={} lines={} reason={}\n", decision.mode, fmt(decision.base_line_height), fmt(decision.base_face_height), fmt(decision.ruby_extent), fmt(decision.available_interline_space), fmt(decision.max_extra), fmts(&decision.line_extras), ints(&decision.expanded_line_indices), decision.reason));
    }
    if let Some(decision) = &result.debug.inline_object_line_height_decision {
        out.push_str(&format!("inlineobjectlineheight base={} face={}+{} available={} clearance={} ascents={} descents={} extras={} boundaries={} trailing={} lines={} reason={}\n", fmt(decision.base_line_height), fmt(decision.base_face_ascent), fmt(decision.base_face_descent), fmt(decision.available_interline_space), fmt(decision.minimum_clearance), fmts(&decision.line_ascents), fmts(&decision.line_descents), fmts(&decision.line_extras), fmts(&decision.boundary_shifts_after), fmt(decision.trailing_extra), ints(&decision.expanded_line_indices), decision.reason));
    }
    if let Some(decision) = &result.debug.max_lines_decision {
        out.push_str(&format!(
            "maxlines laidOut={} visible={} reason={}\n",
            decision.laid_out_lines, decision.visible_lines, decision.reason
        ));
    }
    for segment in &result.debug.decoration_segments {
        out.push_str(&format!(
            "decobox {}-{} kind={} line={} rect={},{},{},{} open={}/{} reason={}\n",
            segment.source_range.start(),
            segment.source_range.end(),
            segment.kind,
            segment.line_index,
            fmt(segment.left),
            fmt(segment.top),
            fmt(segment.right),
            fmt(segment.bottom),
            if segment.open_start { "start" } else { "-" },
            if segment.open_end { "end" } else { "-" },
            segment.reason
        ));
    }
    for decision in &result.debug.ruby_decisions {
        out.push_str(&format!("ruby {}-{} '{}' line={} centerX={} baselineY={} size={} box={}/{} width={} overhang={} locale={}\n", decision.base_range.start(), decision.base_range.end(), decision.text, decision.line_index, fmt(decision.center_x), fmt(decision.baseline_y), fmt(decision.font_size), fmt(decision.ascent), fmt(decision.descent), fmt(decision.width), fmt(decision.overhang), decision.locale));
    }
    for decision in &result.debug.bopomofo_decisions {
        out.push_str(&format!(
            "bopomofo {}-{} '{}' line={} locale={}\n",
            decision.base_range.start(),
            decision.base_range.end(),
            decision.text,
            decision.line_index,
            decision.locale
        ));
        for placement in &decision.placements {
            out.push_str(&format!(
                "  {:?} '{}' rect={},{},{},{} draw={},{} size={}\n",
                placement.role,
                placement.text,
                fmt(placement.left),
                fmt(placement.top),
                fmt(placement.width),
                fmt(placement.height),
                fmt(placement.draw_x),
                fmt(placement.baseline_y),
                fmt(placement.font_size)
            ));
        }
    }
    out
}

fn fmt(value: f32) -> String {
    format!("{value:.1}")
}
fn fmts(values: &[f32]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values
            .iter()
            .map(|value| fmt(*value))
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn ints(values: &[i32]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn escape(value: &str) -> String {
    value
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\u{000B}', "\\v")
        .replace('\u{000C}', "\\f")
        .replace('\u{0085}', "\\u0085")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .replace('\u{200B}', "\\u200B")
}
