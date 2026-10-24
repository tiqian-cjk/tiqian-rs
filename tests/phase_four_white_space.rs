mod support;

use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::core::geometry::{LayoutConstraints, ScalarOffset, text_range};
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::layout_queries::{get_cursor_rect, get_text_for_copy, occupied_bounds, positioned_clusters};
use tiqian::core::layout_result_replay_index::{cursor_rect, to_replay_index};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, LastLineAlignment, LineLengthGrid, ParagraphStyle, TabSize, TextWrapMode, TiqianTextContent, WhiteSpaceMode};
use tiqian::core::units::Ic;

fn layout(text: &str, mode: WhiteSpaceMode, wrap: TextWrapMode, width: f32, alignment: LastLineAlignment) -> LayoutResult {
    ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(width))
            .paragraph_style(ParagraphStyle::builder().white_space(mode).text_wrap(wrap)
                .tab_size(TabSize::LayoutUnits(32.0)).first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false)).last_line_alignment(alignment).build()).build())
}

#[test]
fn preserved_spaces_keep_final_geometry_and_copy_in_all_modes() {
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        let result = layout(" a  ", mode, TextWrapMode::NoWrap, 200.0, LastLineAlignment::Start);
        let expected = if matches!(mode, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks) { 16.0 } else { 40.0 };
        assert_eq!(expected, result.lines[0].visual_width, "{mode:?}");
        assert_eq!(expected, occupied_bounds(&result).unwrap().right);
        assert_eq!(" a  ", get_text_for_copy(&result, text_range(0, 4)).as_str());
        assert!(!result.debug.white_space_units.is_empty());
    }
}

#[test]
fn conditional_hanging_separates_alignment_occupied_and_size() {
    let result = layout("a   ", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 24.0, LastLineAlignment::End);
    let line = &result.lines[0];
    assert_eq!(40.0, line.visual_width);
    assert_eq!(24.0, line.alignment_width);
    assert_eq!(24.0, line.adjusted_width);
    assert_eq!(40.0, occupied_bounds(&result).unwrap().right);
    assert_eq!(24.0, result.size.width);
    assert_eq!(40.0, get_cursor_rect(&result, ScalarOffset::new(4)).left);
}

#[test]
fn tabs_are_final_advances_and_replay_uses_the_same_positions() {
    let result = layout("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::NoWrap, 200.0, LastLineAlignment::Start);
    assert_eq!(48.0, result.lines[0].visual_width);
    let positions = positioned_clusters(&result);
    assert_eq!(32.0, positions[2].left);
    let replay = to_replay_index(&result);
    for offset in 0..=3 {
        assert_eq!(get_cursor_rect(&result, ScalarOffset::new(offset)), cursor_rect(&replay, &result, ScalarOffset::new(offset)));
    }
}

#[test]
fn break_spaces_has_each_caret_and_crlf_replay_is_atomic() {
    let result = layout("a  \r\n", WhiteSpaceMode::BreakSpaces, TextWrapMode::NoWrap, 200.0, LastLineAlignment::Start);
    assert_eq!(24.0, get_cursor_rect(&result, ScalarOffset::new(2)).left);
    assert_eq!(32.0, get_cursor_rect(&result, ScalarOffset::new(3)).left);
    let replay = to_replay_index(&result);
    assert_eq!(get_cursor_rect(&result, ScalarOffset::new(4)), cursor_rect(&replay, &result, ScalarOffset::new(4)));
    assert_eq!(2, result.lines.len());
    let last = result.lines.last().unwrap();
    assert_eq!(0.0, last.visual_width);
    assert!(occupied_bounds(&result).unwrap().bottom == last.bottom);
}

#[test]
fn empty_and_single_space_occupied_bounds_are_not_double_trimmed() {
    assert!(occupied_bounds(&layout("", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 20.0, LastLineAlignment::Start)).is_none());
    for (mode, width) in [(WhiteSpaceMode::PreserveBreaks, 0.0), (WhiteSpaceMode::BreakSpaces, 8.0)] {
        let result = layout(" ", mode, TextWrapMode::Wrap, 20.0, LastLineAlignment::Start);
        assert_eq!(width, result.lines[0].visual_width);
        assert_eq!(width, occupied_bounds(&result).unwrap().right);
    }
}

#[test]
fn collapsed_offsets_share_the_effective_space_end_and_removed_offsets_share_break_edges() {
    let result = layout("a   b", WhiteSpaceMode::PreserveBreaks, TextWrapMode::NoWrap, 200.0, LastLineAlignment::Start);
    for offset in 2..=4 {
        assert_eq!(24.0, get_cursor_rect(&result, ScalarOffset::new(offset)).left);
    }
    let result = layout("a  \r\n  b", WhiteSpaceMode::PreserveBreaks, TextWrapMode::NoWrap, 200.0, LastLineAlignment::Start);
    for offset in 1..=3 {
        assert_eq!(16.0, get_cursor_rect(&result, ScalarOffset::new(offset)).left);
    }
    for offset in 5..=7 {
        assert_eq!(0.0, get_cursor_rect(&result, ScalarOffset::new(offset)).left);
    }
    assert_eq!("a  \r\n  b", get_text_for_copy(&result, text_range(0, 8)).as_str());
}

#[test]
fn tab_grid_survives_justification_and_hanging_tail_is_not_stretched() {
    struct PushOut;
    impl tiqian::clreq::clreq_profile::ClreqProfileResolver for PushOut {
        fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> tiqian::clreq::clreq_profile::ClreqProfile {
            let mut profile = tiqian::clreq::clreq_profile::ClreqProfile::mainland_horizontal();
            profile.adjustment.line_adjustment = tiqian::clreq::clreq_profile::LineAdjustmentStrategy::PushOutOnly;
            profile
        }
    }
    for mode in [WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default()))
            .clreq_profile_resolver(Box::new(PushOut)).build().layout(
                LayoutInput::builder(TiqianTextContent::new(Text::from("中文\t中文   中文中文")), LayoutConstraints::with_defaults(100.0))
                    .paragraph_style(ParagraphStyle::builder().white_space(mode).tab_size(TabSize::LayoutUnits(32.0))
                        .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
        assert!(result.lines.len() > 1);
        assert!(!result.debug.justification_decisions.is_empty(), "{mode:?} {:?}", result.lines);
        let positions = positioned_clusters(&result);
        for position in &positions {
            let cluster = &result.clusters[position.cluster_index as usize];
            if cluster.text.as_str() == "\t" {
                assert_eq!(0.0, position.right % 32.0);
            }
        }
        for line in &result.lines {
            let sum: f32 = line.cluster_range.into_iter().map(|index| result.clusters[index as usize].advance).sum();
            assert_eq!(sum, line.visual_width);
            assert!(line.alignment_width <= line.visual_width);
            if let Some(decision) = result.debug.justification_decisions.iter().find(|decision| decision.line_range == line.range) {
                assert!((decision.deficit_after - (100.0 - line.alignment_width).max(0.0)).abs() < 0.001,
                    "{mode:?} line={line:?} decision={decision:?}");
                assert!(line.alignment_width <= 100.001, "{line:?}");
            }
        }
    }
}

#[test]
fn occupied_bounds_excludes_hidden_lines_but_preserves_overflow_and_terminal_height() {
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build();
    let result = engine.layout(LayoutInput::builder(TiqianTextContent::new(Text::from("a   \nbbbbbbbb")),
        LayoutConstraints::with_max_lines(24.0, 1)).paragraph_style(ParagraphStyle::builder()
            .white_space(WhiteSpaceMode::Preserve).text_wrap(TextWrapMode::NoWrap)
            .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
    assert_eq!(1, result.lines.len());
    let bounds = occupied_bounds(&result).unwrap();
    assert_eq!(40.0, bounds.right);
    assert_eq!(result.lines[0].bottom, bounds.bottom);
    assert!(result.debug.max_lines_decision.is_some());
}

#[test]
fn hanging_background_selection_and_hit_testing_use_final_advance() {
    use tiqian::core::layout_queries::{get_bounding_boxes, get_selection_offset_for_position};
    use tiqian::core::layout_result_replay_index::{selection_boxes, selection_offset_for_position};
    use tiqian::core::text_model::{RichTextBackgroundPaint, RichTextLayer, RichTextLayerKind, RichTextSpan};
    let mut result = layout("a   ", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 24.0, LastLineAlignment::Start);
    result.input.rich_text.push(RichTextSpan { range: text_range(0, 4), layers: vec![RichTextLayer {
        kind: RichTextLayerKind::Background { background: RichTextBackgroundPaint::default() },
        paints: vec![], id: None,
    }], semantics: vec![] });
    let replay = to_replay_index(&result);
    assert_eq!(40.0, replay.rich_text_background_segments[0].right);
    assert_eq!(40.0, replay.rich_text_layer_cluster_segments.last().unwrap().right);
    assert_eq!(40.0, get_bounding_boxes(&result, text_range(1, 4)).last().unwrap().right);
    assert_eq!(40.0, selection_boxes(&replay, &result, text_range(0, 4))[0].right);
    for (x, offset) in [(24.0, 2), (32.0, 3), (40.0, 4)] {
        let expected = ScalarOffset::new(offset);
        assert_eq!(expected, get_selection_offset_for_position(&result, x, 1.0));
        assert_eq!(expected, selection_offset_for_position(&replay, &result, x, 1.0));
    }
}

#[test]
fn soft_hanging_aligns_the_body_and_authorizes_full_paint_bounds() {
    use tiqian::core::layout_paint_bounds::legal_hanging_punctuation_clip_edge;
    let result = layout("a   b", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 24.0, LastLineAlignment::Start);
    let first = &result.lines[0];
    assert_eq!(16.0, first.alignment_width);
    assert_eq!(40.0, first.visual_width);
    assert_eq!(40.0, legal_hanging_punctuation_clip_edge(first, 24.0));
    let boundary = first.range.end();
    assert_eq!(result.lines[1].top, get_cursor_rect(&result, boundary).top);
    assert_eq!(0.0, get_cursor_rect(&result, boundary).left);
    for (alignment, indent) in [(LastLineAlignment::Center, 4.0), (LastLineAlignment::End, 8.0)] {
        let result = layout("a ", WhiteSpaceMode::Preserve, TextWrapMode::NoWrap, 32.0, alignment);
        assert_eq!(indent, result.lines[0].indent);
    }
}

#[test]
fn tab_grid_includes_block_and_first_line_indent_without_moving_origin() {
    for (block, first) in [(0.0, 0.5), (0.5, 0.0), (0.5, 0.5)] {
        let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build().layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from("a\tb\na\tb")), LayoutConstraints::with_defaults(200.0))
                .paragraph_style(ParagraphStyle::builder().white_space(WhiteSpaceMode::BreakSpaces).text_wrap(TextWrapMode::NoWrap)
                    .tab_size(TabSize::LayoutUnits(32.0)).block_indent(Ic { count: block }).first_line_indent(Some(Ic { count: first }))
                    .line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
        for position in positioned_clusters(&result) {
            if result.clusters[position.cluster_index as usize].text.as_str() == "\t" {
                assert_eq!(0.0, position.right % 32.0, "block={block} first={first}");
            }
        }
    }
}

#[test]
fn collapsed_crlf_has_no_internal_interaction_stop_and_keeps_source() {
    use tiqian::core::layout_queries::{get_offset_for_position, get_selection_offset_for_position};
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks] {
        let result = layout("a\r\nb", mode, TextWrapMode::Wrap, 24.0, LastLineAlignment::Start);
        let replay = to_replay_index(&result);
        assert_eq!(get_cursor_rect(&result, ScalarOffset::new(3)), get_cursor_rect(&result, ScalarOffset::new(2)));
        assert_eq!(get_cursor_rect(&result, ScalarOffset::new(3)), cursor_rect(&replay, &result, ScalarOffset::new(2)));
        for line in &result.lines {
            for x in 0..=48 {
                assert_ne!(ScalarOffset::new(2), get_offset_for_position(&result, x as f32, line.baseline));
                assert_ne!(ScalarOffset::new(2), get_selection_offset_for_position(&result, x as f32, line.baseline));
            }
        }
        assert_eq!("a\r\nb", get_text_for_copy(&result, text_range(0, 4)).as_str());
    }
}

#[test]
fn object_and_punctuation_geometry_do_not_consume_preserved_spaces() {
    use tiqian::core::text_model::InlineObjectSpan;
    for mode in [WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build().layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from(" x ， ")), LayoutConstraints::with_defaults(200.0))
                .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(text_range(1, 2), 24.0, 16.0, 4.0)])
                .paragraph_style(ParagraphStyle::builder().white_space(mode).text_wrap(TextWrapMode::NoWrap)
                    .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
        assert_eq!(24.0, result.clusters.iter().find(|cluster| cluster.range == text_range(1, 2)).unwrap().advance);
        assert_eq!(24.0, result.clusters.iter().filter(|cluster| cluster.text.as_str() == " ").map(|cluster| cluster.advance).sum::<f32>());
        assert_eq!(64.0, result.lines[0].visual_width);
        assert!(result.debug.inline_object_punctuation_attachment_decisions.is_empty());
    }
}

#[test]
fn cached_justification_stages_match_existing_priorities_and_capacities() {
    use tiqian::common::{HashMap, HashSet};
    use tiqian::core::east_asian_spacing::unicode_east_asian_spacing;
    use tiqian::core::font_face::FontFaceId;
    use tiqian::core::int_range::IntRange;
    use tiqian::core::layout_model::Cluster;
    use tiqian::core::text_model::{
        InlineObjectPreferredStretch, InlineObjectPreferredStretchKind,
    };
    use tiqian::font::font_policy::FontRole;
    use tiqian::layout::justifier::{JustificationRequest, Justifier};
    use tiqian::layout::progressive_break_decisions::ProgressiveBreakTier;

    let texts = ["中", "a", " ", "b", "中", "中"];
    let advances = [16.0, 16.0, 4.0, 16.0, 16.0, 16.0];
    let clusters: Vec<_> = texts
        .iter()
        .zip(advances)
        .enumerate()
        .map(|(index, (text, advance))| {
            Cluster::new(
                text_range(index as i32, index as i32 + 1),
                Text::from(*text),
                FontFaceId::with_resource_id(if *text == " " || *text == "a" || *text == "b" {
                    "latin"
                } else {
                    "cjk"
                }),
                advance,
            )
        })
        .collect();
    let roles = [
        FontRole::CjkText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::CjkText,
        FontRole::CjkText,
    ];
    let edges: Vec<_> = clusters
        .iter()
        .map(|cluster| unicode_east_asian_spacing::resolved_edges(&cluster.text, "zh-Hans"))
        .collect();
    let justifier = Justifier::default();
    let empty_set = HashSet::new();
    let empty_map = HashMap::new();

    for count in 0..=2 {
        let mut preferred_inline = HashMap::new();
        for (index, kind) in [
            InlineObjectPreferredStretchKind::Relation,
            InlineObjectPreferredStretchKind::BinaryOperator,
        ]
        .into_iter()
        .take(count)
        .enumerate()
        {
            preferred_inline.insert(
                if index == 0 { 0 } else { 3 },
                InlineObjectPreferredStretch::new(kind, 1.0 + index as f32, 8.0),
            );
        }
        let emergency: HashMap<i32, String> = (0..count)
            .map(|index| (index as i32 + 1, format!("emergency-{index}")))
            .collect();
        let preferred_emergency: HashMap<i32, String> = (0..count)
            .map(|index| (index as i32 + 4, format!("preferred-{index}")))
            .collect();
        let technical = HashMap::from([(2, ProgressiveBreakTier::Whitespace)]);
        let request = JustificationRequest {
            adjusted_clusters: &clusters,
            cluster_roles: &roles,
            east_asian_spacing_edges: &edges,
            line_cluster_range: IntRange::new(0, clusters.len() as i32 - 1),
            max_width: advances.iter().sum(),
            font_size: 16.0,
            skip: false,
            skip_reason: None,
            allow_sino_western_gap_stretch: true,
            cjk_latin_space_base_em: 0.25,
            cjk_latin_space_max_em: 0.5,
            no_stretch_boundary_clusters: &empty_set,
            no_stretch_boundary_after_clusters: &empty_set,
            western_bracket_cjk_inter_char_boundary_after_clusters: &empty_set,
            attached_inline_physical_boundary_after_clusters: &empty_set,
            attached_inline_virtual_boundary_after_clusters: &empty_map,
            attached_inline_virtual_sino_western_boundary_after_clusters: &empty_set,
            uniform_inline_object_boundary_after_clusters: &empty_set,
            preferred_inline_object_boundary_after_clusters: &preferred_inline,
            technical_boundary_after_clusters: &technical,
            emergency_tracking_boundary_after_clusters: &emergency,
            preferred_emergency_tracking_boundary_after_clusters: &preferred_emergency,
        };

        let mut seed = request.clone();
        seed.max_width += 0.125;
        let prepared = justifier.prepare_stages(seed);
        for budget in [0.125, 2.0, 4.0, 8.0, 16.0, 32.0, 100.0] {
            let request_width = request.max_width;
            let mut request = request.clone();
            request.max_width = request_width + budget;
            let regular = justifier.justify(request.clone());
            let stages = prepared.plan(budget);

            assert_eq!(regular.allocations.len(), stages.allocations.len());
            for (expected, actual) in regular.allocations.iter().zip(&stages.allocations) {
                assert_eq!(expected.target_cluster_index, actual.target_cluster_index);
                assert_eq!(expected.kind, actual.kind);
                assert_eq!(expected.priority, actual.priority);
                assert_eq!(expected.reason, actual.reason);
                assert!((expected.delta - actual.delta).abs() < 1e-4);
            }
            assert!((regular.unfilled_deficit - stages.unfilled_deficit).abs() < 1e-4);

            let mut filled_deltas = vec![0.0; clusters.len()];
            let remaining = prepared.fill_deltas(budget, &mut filled_deltas);
            assert!((remaining - stages.unfilled_deficit).abs() < 1e-4);
            for (index, delta) in filled_deltas.iter().enumerate() {
                let planned: f32 = stages
                    .allocations
                    .iter()
                    .filter(|allocation| allocation.target_cluster_index == index as i32)
                    .map(|allocation| allocation.delta)
                    .sum();
                assert!((*delta - planned).abs() < 1e-4);
            }
            assert_eq!(None, stages.fallback_reason);
        }
    }
}

#[test]
fn final_tail_tab_hanging_records_match_the_line_geometry() {
    for text in ["中文\t   后文", "中文 \t\n", "   \t", "中文\t中文 \t后文"] {
        for width in [24.0, 48.0, 76.0, 100.0] {
            let result = layout(text, WhiteSpaceMode::Preserve, TextWrapMode::Wrap, width, LastLineAlignment::Start);
            for (index, line) in result.lines.iter().enumerate() {
                let hanging: f32 = result.debug.line_white_space_adjustments.iter()
                    .filter(|decision| decision.line_index == index as i32)
                    .map(|decision| decision.adjustment.hanging_advance).sum();
                assert!((line.visual_width - line.alignment_width - line.hanging_punctuation_advance - hanging).abs() < 0.001,
                    "{text:?} width={width} line={line:?}");
                for decision in result.debug.line_white_space_adjustments.iter().filter(|decision| decision.line_index == index as i32) {
                    if decision.adjustment.action == tiqian::layout::white_space::LineWhiteSpaceAction::ResolveTab {
                        assert_eq!(result.clusters[decision.adjustment.cluster_index as usize].advance, decision.adjustment.occupied_advance);
                    }
                }
            }
        }
    }
}

#[test]
fn punctuation_hanging_keeps_occupied_advance_outside_alignment_measure() {
    let mut observed = false;
    for width in [16.0, 24.0, 32.0, 40.0, 48.0] {
        let result = layout("中文，中文。", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, width, LastLineAlignment::Start);
        for line in &result.lines {
            if line.hanging_punctuation_advance <= 0.0 { continue; }
            observed = true;
            assert!((line.visual_width - line.alignment_width - line.hanging_punctuation_advance).abs() < 0.001);
            let bounds = tiqian::core::layout_queries::line_occupied_bounds(line);
            assert_eq!(line.indent + line.visual_width + line.hyphen_advance, bounds.right);
            assert!(bounds.right > line.indent + line.alignment_width);
        }
    }
    assert!(observed);
}

#[test]
fn tab_justification_stop_jumps_keep_a_feasible_final_measure() {
    for mode in [WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for interval in [0.0, 2.0, 8.0, 24.0, 32.0] {
            for width in [36.0, 52.0, 68.0, 84.0, 100.0] {
                let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build().layout(
                    LayoutInput::builder(TiqianTextContent::new(Text::from("中文\t中文 中\t中文中文")), LayoutConstraints::with_defaults(width))
                        .paragraph_style(ParagraphStyle::builder().white_space(mode).tab_size(TabSize::LayoutUnits(interval))
                            .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
                for line in &result.lines {
                    if let Some(decision) = result.debug.justification_decisions.iter().find(|decision| decision.line_range == line.range) {
                        assert!(line.alignment_width <= width + 0.001,
                            "{mode:?} interval={interval} width={width} line={line:?}");
                        assert!((decision.deficit_after - (width - line.alignment_width).max(0.0)).abs() < 0.001);
                    }
                }
            }
        }
    }
}