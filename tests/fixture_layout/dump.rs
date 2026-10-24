use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::clreq::clreq_profile::{
    BuiltInClreqProfileResolver, ClreqProfileResolver, KinsokuLevel, KinsokuMode,
};
use tiqian::core::layout_model::LayoutResult;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::linebreak::english_hyphenation::english_hyphenation;
use tiqian::linebreak::hyphenation::{Hyphenator, NoHyphenator};

use super::cases::Fixture;
use crate::support::DeterministicStubFontBackend;

static NO_HYPHENATOR: NoHyphenator = NoHyphenator;

struct FixtureProfileResolver {
    pin_basic_no_hang: bool,
}

impl ClreqProfileResolver for FixtureProfileResolver {
    fn resolve(
        &self,
        profile_id: &tiqian::core::text_model::LayoutProfileId,
    ) -> tiqian::clreq::clreq_profile::ClreqProfile {
        let mut profile = BuiltInClreqProfileResolver.resolve(profile_id);
        if self.pin_basic_no_hang {
            profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        }
        profile
    }
}

pub fn dump_fixture(fixture: &Fixture) -> String {
    let input = fixture.input.clone();
    let mut output = format!(
        "fixture: {}\ntext: {}\nmaxWidth: {}\n",
        fixture.id,
        escape(&input.content.text.to_string()),
        fmt(input.constraints.max_width())
    );
    for label in ["greedy", "lookahead", "paragraph-dp"] {
        output.push_str(&dump_result(label, layout(fixture, label)));
    }
    output
}

fn layout(fixture: &Fixture, breaker: &str) -> LayoutResult {
    let line_breaker: Box<dyn tiqian::layout::line_breaker::LineBreaker> = match breaker {
        "greedy" => Box::new(GreedyLineBreaker::default()),
        "lookahead" => Box::new(LookaheadLineBreaker::default()),
        "paragraph-dp" => Box::new(ParagraphDpLineBreaker::default()),
        _ => unreachable!(),
    };
    let hyphenator = if fixture.use_english_hyphenation {
        english_hyphenation::en_us()
    } else {
        &NO_HYPHENATOR as &dyn Hyphenator
    };
    let mut engine =
        ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
            .line_breaker(line_breaker)
            .hyphenator(hyphenator)
            .clreq_profile_resolver(Box::new(FixtureProfileResolver {
                pin_basic_no_hang: fixture.pin_basic_no_hang,
            }))
            .build();
    engine.layout(fixture.input.clone())
}

fn dump_result(label: &str, result: LayoutResult) -> String {
    let mut output = format!(
        "== {label} ==\nsize {}x{}\n",
        fmt(result.size.width),
        fmt(result.size.height)
    );
    if let Some(grid) = &result.debug.line_length_grid_decision
        && grid.enabled
        && grid.slack > 0.0
    {
        output.push_str(&format!(
            "grid container={} measure={}({}字) slack={} body={}@{}\n",
            fmt(grid.container_width),
            fmt(grid.measure),
            grid.cells,
            fmt(grid.slack),
            format!("{:?}", grid.body_alignment),
            fmt(grid.body_offset)
        ));
    }
    if let Some(indent) = &result.debug.first_line_indent_decision
        && indent.source != tiqian::core::layout_model::FirstLineIndentSource::Explicit
    {
        output.push_str(&format!(
            "firstindent {}字 measure={}字 threshold={}字 {}\n",
            fmt(indent.resolved_em),
            fmt(indent.measure_em),
            fmt(indent.threshold_em),
            indent.source.name()
        ));
    }
    if let Some(kinsoku) = &result.debug.kinsoku_decision {
        output.push_str(&format!(
            "kinsoku measure={}字 level={:?} hang={:?} reason={}\n",
            fmt(kinsoku.measure_em),
            kinsoku.level,
            kinsoku.hanging,
            kinsoku_reason_text(kinsoku)
        ));
    }
    for decision in &result.debug.contextual_kinsoku_decisions {
        output.push_str(&format!(
            "context-kinsoku {}-{} source='{}' cluster={} forbid={} reason={}{}\n",
            decision.range.start(),
            decision.range.end(),
            escape(decision.source_text.as_str()),
            decision.cluster_index,
            decision.forbidden_position.name(),
            decision.reason.name(),
            decision
                .impossible_measure_fallback
                .as_ref()
                .map(|value| format!(" fallback={}", value.name()))
                .unwrap_or_default()
        ));
    }
    for decision in &result.debug.break_opportunity_decisions {
        output.push_str(&format!(
            "break-opportunity {}-{} source='{}' offsets={}{} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(decision.source_text.as_str()),
            decision
                .break_offsets
                .iter()
                .map(|offset| offset.value().to_string())
                .collect::<Vec<_>>()
                .join(","),
            decision
                .tier
                .as_ref()
                .map(|value| format!(" tier={value:?}"))
                .unwrap_or_default(),
            decision.reason
        ));
    }
    for decision in &result.debug.emergency_tracking_eligibility_decisions {
        output.push_str(&format!(
            "tracking-eligibility {}-{} source='{}' reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(decision.source_text.as_str()),
            decision.reason.name()
        ));
    }
    for attachment in &result.debug.inline_object_punctuation_attachment_decisions {
        output.push_str(&format!("inline-object-punctuation {}-{} separator={}-{} punctuation={}-{} source='{}' collapsed={} protected={}-{} reason={}\n", attachment.object_range.start(), attachment.object_range.end(), attachment.separator_range.start(), attachment.separator_range.end(), attachment.punctuation_range.start(), attachment.punctuation_range.end(), escape(attachment.punctuation_text.as_str()), fmt(attachment.collapsed_advance), attachment.protected_range.start(), attachment.protected_range.end(), attachment.reason));
    }
    for (index, line) in result.lines.iter().enumerate() {
        let decision = result.debug.line_decisions.get(index);
        let repair = decision
            .and_then(|value| value.repair_decision.as_ref())
            .map_or("-".to_owned(), |value| {
                format!(
                    "{:?}({:?} shrink={})",
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
                            "{:?}{}",
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
                        let kind = glue_kind_name(allocation.kind);
                        let reason = allocation.reason.name();
                        format!(
                            "{}@{}+{}{}",
                            kind,
                            allocation.cluster_range.start(),
                            fmt(allocation.delta),
                            if reason == kind {
                                String::new()
                            } else {
                                format!("({})", reason)
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
        output.push_str(&format!("line[{index}] {}-{} {}{}natural={} adjusted={} visual={} repair={} candidates={} justify={}\n", line.range.start(), line.range.end(), if line.indent > 0.0 { format!("indent={} ", fmt(line.indent)) } else { String::new() }, if line.hyphen_advance > 0.0 { format!("hyphen={} ", fmt(line.hyphen_advance)) } else { String::new() }, fmt(line.natural_width), fmt(line.adjusted_width), fmt(line.visual_width), repair, candidates, justify));
    }
    for unit in &result.debug.white_space_units {
        output.push_str(&format!("whitespace {}-{} kind={:?} action={:?}\n",
            unit.range.start(), unit.range.end(), unit.kind, unit.action));
    }
    for decision in &result.debug.line_white_space_adjustments {
        let adjustment = &decision.adjustment;
        output.push_str(&format!("line-whitespace line={} cluster={} action={:?} occupied={} hanging={}\n",
            decision.line_index, adjustment.cluster_index, adjustment.action,
            fmt(adjustment.occupied_advance), fmt(adjustment.hanging_advance)));
    }
    for (index, line) in result.lines.iter().enumerate() {
        let bounds = tiqian::core::layout_queries::line_occupied_bounds(line);
        output.push_str(&format!("line-geometry[{index}] alignment={} occupied={},{},{},{}\n",
            fmt(line.alignment_width), fmt(bounds.left), fmt(bounds.top), fmt(bounds.right), fmt(bounds.bottom)));
    }
    for cluster in &result.clusters {
        output.push_str(&format!(
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
        output.push_str(&format!(
            "font {}-{} role={:?} key={} display='{}' sub={}\n",
            font.range.start(),
            font.range.end(),
            font.role,
            font.candidate_key,
            font.display_text,
            font.substitution_reason
        ));
    }
    for role in &result.debug.role_overrides {
        output.push_str(&format!(
            "role-override {}-{} source='{}' {}->{} policy={} reason={}\n",
            role.range.start(),
            role.range.end(),
            escape(role.source_text.as_str()),
            format!("{:?}", role.original_role),
            format!("{:?}", role.overridden_role),
            role.source,
            role.reason
        ));
    }
    for punctuation in &result.debug.punctuation_decisions {
        output.push_str(&format!("punct {}-{} '{}' class={} adv={} body={} lead={} trail={} {}anchor={} source={}{}{}{}{}{}{}\n", punctuation.range.start(), punctuation.range.end(), punctuation.ch, format!("{:?}", punctuation.punctuation_class), fmt(punctuation.advance), fmt(punctuation.body_width), fmt(punctuation.leading_glue_natural), fmt(punctuation.trailing_glue_natural), if punctuation.leading_glue_initially_consumed != 0.0 || punctuation.trailing_glue_initially_consumed != 0.0 { format!("initial={}/{} ", fmt(punctuation.leading_glue_initially_consumed), fmt(punctuation.trailing_glue_initially_consumed)) } else { String::new() }, format!("{:?}", punctuation.anchor), punctuation.geometry_source.name(), if punctuation.advance_expansion != 0.0 { format!(" expand={}", fmt(punctuation.advance_expansion)) } else { String::new() }, if punctuation.glyph_inline_shift != 0.0 { format!(" glyphShift={}", fmt(punctuation.glyph_inline_shift)) } else { String::new() }, punctuation.glyph_placement_reason.as_ref().map(|value| format!(" placement={value}")).unwrap_or_default(), punctuation.halt_advance.map(|value| format!(" halt={}", fmt(value))).unwrap_or_default(), punctuation.ink_bounds_fallback.as_ref().map(|value| format!(" fallback={value}")).unwrap_or_default(), punctuation.halt_validation.as_ref().map(|value| format!(" haltWarn={value}")).unwrap_or_default()));
    }
    for geometry in &result.debug.geometry_decisions {
        output.push_str(&format!(
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
        output.push_str(&format!(
            "inline-box {}-{} start={} end={} outer={} clusters={}-{} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            fmt(decision.inline_start),
            fmt(decision.inline_end),
            format!("{:?}", decision.outer_spacing),
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
        output.push_str(&format!("inline-object {}-{} advance={} ascent={} descent={} cluster={} line={} edges={}..{} reason={}\n", decision.range.start(), decision.range.end(), fmt(decision.advance), fmt(decision.ascent), fmt(decision.descent), decision.cluster_index, decision.line_index, edge(decision.leading_uniform_stretch, decision.leading_preferred_stretch_kind.map(preferred_stretch_kind_name), decision.leading_preferred_stretch_natural_width, decision.leading_preferred_stretch_target_width, decision.leading_preferred_stretch_capacity, decision.leading_prevents_line_break, decision.leading_shrink_capacity, decision.leading_line_end_discardable_advance), edge(decision.trailing_uniform_stretch, decision.trailing_preferred_stretch_kind.map(preferred_stretch_kind_name), decision.trailing_preferred_stretch_natural_width, decision.trailing_preferred_stretch_target_width, decision.trailing_preferred_stretch_capacity, decision.trailing_prevents_line_break, decision.trailing_shrink_capacity, decision.trailing_line_end_discardable_advance), decision.reason));
    }
    for decision in &result.debug.spacing_decisions {
        output.push_str(&format!(
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
        output.push_str(&format!(
            "autospace {}-{} side={} boundary={} reduction={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            auto_space_side_name(decision.side),
            decision.boundary_role.name(),
            fmt(decision.total_reduction)
        ));
    }
    for decision in &result.debug.mandatory_break_decisions {
        output.push_str(&format!(
            "mandatorybreak {}-{} afterCluster={} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            decision.break_after_cluster_index,
            decision.reason
        ));
    }
    for decision in &result.debug.zero_width_break_decisions {
        output.push_str(&format!(
            "zerowidthbreak {}-{} source='{}' cluster={} reason={}\n",
            decision.range.start(),
            decision.range.end(),
            escape(decision.source_text.as_str()),
            decision.cluster_index,
            decision.reason
        ));
    }
    for decision in &result.debug.line_edge_trim_decisions {
        output.push_str(&format!(
            "edgetrim {}-{} side={} trim={} reason={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            decision.side.name(),
            fmt(decision.trim_amount),
            decision.reason.name()
        ));
    }
    for decision in &result.debug.decoration_decisions {
        output.push_str(&format!(
            "deco {}-{} '{}' kind={} applied={} anchor={},{} diameter={} reason={}\n",
            decision.cluster_range.start(),
            decision.cluster_range.end(),
            decision.source_text,
            decision.kind,
            decision.applied,
            fmt(decision.anchor_x),
            fmt(decision.anchor_y),
            fmt(decision.dot_diameter),
            decision.reason.name()
        ));
    }
    if let Some(spacing) = &result.debug.line_spacing_decision {
        output.push_str(&format!(
            "linespacing natural={} requested={} resolved={} floor={} applied={} reason={}\n",
            fmt(spacing.natural_height),
            spacing
                .requested_line_height
                .map(fmt)
                .unwrap_or_else(|| "-".to_owned()),
            fmt(spacing.resolved_height),
            fmt(spacing.spacing_floor),
            spacing.floor_applied,
            spacing.reason.name()
        ));
    }
    if let Some(decision) = &result.debug.ruby_line_height_decision {
        output.push_str(&format!("rubylineheight mode={:?} base={} face={} ruby={} available={} maxExtra={} extras={} lines={} reason={}\n", decision.mode, fmt(decision.base_line_height), fmt(decision.base_face_height), fmt(decision.ruby_extent), fmt(decision.available_interline_space), fmt(decision.max_extra), fmts(&decision.line_extras), ints(&decision.expanded_line_indices), decision.reason.name()));
    }
    if let Some(decision) = &result.debug.inline_object_line_height_decision {
        output.push_str(&format!("inlineobjectlineheight base={} face={}+{} available={} clearance={} ascents={} descents={} extras={} boundaries={} trailing={} lines={} reason={}\n", fmt(decision.base_line_height), fmt(decision.base_face_ascent), fmt(decision.base_face_descent), fmt(decision.available_interline_space), fmt(decision.minimum_clearance), fmts(&decision.line_ascents), fmts(&decision.line_descents), fmts(&decision.line_extras), fmts(&decision.boundary_shifts_after), fmt(decision.trailing_extra), ints(&decision.expanded_line_indices), decision.reason.name()));
    }
    if let Some(decision) = &result.debug.max_lines_decision {
        output.push_str(&format!(
            "maxlines laidOut={} visible={} reason={}\n",
            decision.laid_out_lines, decision.visible_lines, decision.reason
        ));
    }
    for segment in &result.debug.decoration_segments {
        output.push_str(&format!(
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
            segment_reason_text(segment)
        ));
    }
    for decision in &result.debug.ruby_decisions {
        output.push_str(&format!("ruby {}-{} '{}' line={} centerX={} baselineY={} size={} box={}/{} width={} overhang={} locale={}\n", decision.base_range.start(), decision.base_range.end(), decision.text, decision.line_index, fmt(decision.center_x), fmt(decision.baseline_y), fmt(decision.font_size), fmt(decision.ascent), fmt(decision.descent), fmt(decision.width), fmt(decision.overhang), decision.locale));
    }
    for decision in &result.debug.bopomofo_decisions {
        output.push_str(&format!(
            "bopomofo {}-{} '{}' line={} locale={}\n",
            decision.base_range.start(),
            decision.base_range.end(),
            decision.text,
            decision.line_index,
            decision.locale
        ));
        for placement in &decision.placements {
            output.push_str(&format!(
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
    output
}

fn fmt(value: f32) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "Infinity".to_owned()
        } else {
            "-Infinity".to_owned()
        };
    }
    let negative = value.to_bits() & 0x8000_0000 != 0;
    let scaled = ((value.abs() as f64 * 10.0) + 0.5).floor() as i64;
    format!(
        "{}{}.{}",
        if negative { "-" } else { "" },
        scaled / 10,
        scaled % 10
    )
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

/// `KinsokuDecisionInfo` 的 dump 文本：`Fixed:{level}{+Hang}` 或
/// `MeasureAdaptiveKinsoku:{n}字→{level}{+Hang}`。
///
/// dump 必须逐字节复现类型化之前的输出，因此这里自行拼装，
/// 不依赖 `src/` 提供同义函数。
fn kinsoku_reason_text(kinsoku: &tiqian::core::layout_model::KinsokuDecisionInfo) -> String {
    use tiqian::clreq::clreq_profile::HangingPunctuationStyle;
    use tiqian::core::layout_model::KinsokuPolicySource;
    let hang = if kinsoku.hanging != HangingPunctuationStyle::Disabled {
        "+Hang"
    } else {
        ""
    };
    match kinsoku.source {
        KinsokuPolicySource::Fixed => format!("Fixed:{:?}{hang}", kinsoku.level),
        KinsokuPolicySource::MeasureAdaptiveKinsoku => format!(
            "MeasureAdaptiveKinsoku:{}字→{:?}{hang}",
            kinsoku.adaptive_measure_cells.unwrap_or(0),
            kinsoku.level
        ),
    }
}

/// `InlineObjectPreferredStretchKind` 的 dump 文本，与类型化之前的 `{:?}` 输出一致。
fn preferred_stretch_kind_name(
    kind: tiqian::core::text_model::InlineObjectPreferredStretchKind,
) -> &'static str {
    use tiqian::core::text_model::InlineObjectPreferredStretchKind;
    match kind {
        InlineObjectPreferredStretchKind::PunctuationTrailing => "PunctuationTrailing",
        InlineObjectPreferredStretchKind::Relation => "Relation",
        InlineObjectPreferredStretchKind::BinaryOperator => "BinaryOperator",
    }
}

/// `DecorationSegmentInfo` 的 dump 文本：基础原因，且相邻行间线相向缩短时追加后缀。
fn segment_reason_text(segment: &tiqian::core::layout_model::DecorationSegmentInfo) -> String {
    if segment.adjacent_shortened {
        format!(
            "{};AdjacentInterlinearLineShortening",
            segment.reason.name()
        )
    } else {
        segment.reason.name().to_owned()
    }
}

/// `GlueKind` 的 dump 文本，与类型化之前的 `{:?}` 输出一致。
fn glue_kind_name(kind: tiqian::layout::punctuation_model::GlueKind) -> &'static str {
    use tiqian::layout::punctuation_model::GlueKind;
    match kind {
        GlueKind::PunctuationLeading => "PunctuationLeading",
        GlueKind::PunctuationTrailing => "PunctuationTrailing",
        GlueKind::CjkLatinSpace => "CjkLatinSpace",
        GlueKind::WordSpace => "WordSpace",
        GlueKind::CjkInterChar => "CjkInterChar",
        GlueKind::ProgressiveTechnical => "ProgressiveTechnical",
        GlueKind::EmergencyGraphemeTracking => "EmergencyGraphemeTracking",
        GlueKind::InlineObjectPunctuationTrailing => "InlineObjectPunctuationTrailing",
        GlueKind::InlineObjectRelation => "InlineObjectRelation",
        GlueKind::InlineObjectBinaryOperator => "InlineObjectBinaryOperator",
        GlueKind::InlineObjectBoundary => "InlineObjectBoundary",
    }
}

/// `AutoSpaceSide` 的 dump 文本，与类型化之前的小写输出一致。
fn auto_space_side_name(side: tiqian::core::layout_model::AutoSpaceSide) -> &'static str {
    use tiqian::core::layout_model::AutoSpaceSide;
    match side {
        AutoSpaceSide::Leading => "leading",
        AutoSpaceSide::Trailing => "trailing",
        AutoSpaceSide::Gap => "gap",
    }
}
