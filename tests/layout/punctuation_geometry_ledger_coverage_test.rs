use tiqian::clreq::clreq_profile::{PunctuationGluePlacement, PunctuationWidthPolicy};
use tiqian::common::HashMap;
use tiqian::core::geometry::{text_range, Rect};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::InlineAttachment;
use tiqian::layout::line_breaker::LineCandidate;
use tiqian::layout::punctuation_geometry_ledger::{
    cluster_index_range_for, PunctuationGeometryLedger,
};
use tiqian::layout::punctuation_geometry_stage::punctuation_atoms;
use tiqian::layout::punctuation_model::{
    PunctuationAtomBuilder, PunctuationInkInput, PunctuationSpacingAdjustment,
    PunctuationSpacingCompressionResult, PunctuationSpacingCompressor,
};

const EM: f32 = 16.0;

fn cluster(text: &str, start: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::new(
        text_range(start, start + text.chars().count() as i32),
        Text::from(text),
        font_key.to_owned(),
        advance,
    )
}

fn atoms_for(clusters: &[Cluster]) -> Vec<tiqian::layout::punctuation_model::PunctuationAtom> {
    let builder = PunctuationAtomBuilder::default();
    clusters
        .iter()
        .flat_map(|cluster| {
            punctuation_atoms(
                cluster,
                EM,
                &builder,
                &[],
                PunctuationGluePlacement::MainlandSimplified,
                PunctuationWidthPolicy::default(),
            )
        })
        .collect()
}

fn ledger_of(texts: &[&str]) -> PunctuationGeometryLedger {
    let mut start = 0;
    let clusters: Vec<_> = texts
        .iter()
        .map(|text| {
            let result = cluster(text, start, EM, "cjk");
            start = result.range.end().value();
            result
        })
        .collect();
    let atoms = atoms_for(&clusters);
    PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressor.compress(&atoms, EM),
    )
}

fn line(range: IntRange, start: i32, end: i32) -> LineCandidate {
    LineCandidate::new(range, text_range(start, end), 32.0, 32.0)
}

#[test]
fn budgets_resolve_advances_through_remaining_glue() {
    let ledger = ledger_of(&["。", "「", "中"]);
    let resolved = ledger.resolve_clusters();
    assert_eq!(8.0, resolved[0].advance);
    assert_eq!(16.0, resolved[1].advance);
    assert_eq!(16.0, resolved[2].advance);
    assert_eq!(resolved[2], ledger.resolve_clusters()[2]);
}

#[test]
fn glue_capacities_report_sides_and_pairing() {
    let ledger = ledger_of(&["。", "「"]);
    let capacities = ledger.glue_capacities();
    assert_eq!(1, capacities.len());
    assert_eq!(8.0, capacities[&1].leading);
    assert_eq!(0.0, capacities[&1].trailing);
    assert!(!capacities[&1].paired);
}

#[test]
fn side_consumption_is_capped_and_skips_non_positive_amounts() {
    let ledger = ledger_of(&["。", "「"]);
    let consumed = ledger
        .consume_leading_by_cluster(&HashMap::from([(1, 4.0)]))
        .consume_leading_by_cluster(&HashMap::from([(1, 0.0)]))
        .consume_trailing_by_cluster(&HashMap::from([(1, -1.0)]))
        .consume_leading_by_cluster(&HashMap::from([(99, 8.0)]));
    assert_eq!(4.0, consumed.glue_capacities()[&1].leading);
    let capped = ledger.consume_leading_by_cluster(&HashMap::from([(1, 100.0)]));
    assert!(!capped.glue_capacities().contains_key(&1));
}

#[test]
fn justification_deltas_and_structural_channels_feed_resolved_advance() {
    let base = ledger_of(&["「", "中"]);
    assert_eq!(16.0, base.resolve_clusters()[0].advance);

    let justified = base.add_justification_deltas(&HashMap::from([(0, 1.5)]));
    assert_eq!(17.5, justified.resolve_clusters()[0].advance);
    assert_eq!(1.5, justified.to_decision_info()[0].justification_delta);
    assert_eq!(0.0, base.to_decision_info()[0].justification_delta);

    let spread = base.with_ruby_spread(&HashMap::from([(0, 2.0)]));
    assert_eq!(18.0, spread.resolve_clusters()[0].advance);
    assert_eq!(2.0, spread.to_decision_info()[0].ruby_spread);

    let trimmed = base.with_raw_edge_trims(&HashMap::from([(0, 3.0)]));
    assert_eq!(13.0, trimmed.resolve_clusters()[0].advance);
    let trimmed_twice = trimmed.with_raw_edge_trims(&HashMap::from([(0, 20.0)]));
    assert_eq!(0.0, trimmed_twice.resolve_clusters()[0].advance);

    assert_eq!(base, base.with_ruby_spread(&HashMap::new()));
    assert_eq!(base, base.with_raw_edge_trims(&HashMap::new()));
    assert_eq!(base, base.with_inline_box_advances(&HashMap::new()));

    let boxed = base.with_inline_box_advances(&HashMap::from([(0, 4.0)]));
    assert_eq!(20.0, boxed.resolve_clusters()[0].advance);
}

#[test]
fn decision_info_lists_every_geometry_with_budgets() {
    let infos = ledger_of(&["。", "中"]).to_decision_info();
    assert_eq!(1, infos.len());
    let info = &infos[0];
    assert_eq!(text_range(0, 1), info.range);
    assert_eq!("。", info.source_text);
    assert_eq!(16.0, info.base_advance);
    assert_eq!(8.0, info.body_width);
    assert_eq!(0.0, info.leading_glue_natural);
    assert_eq!(8.0, info.trailing_glue_natural);
    assert_eq!(16.0, info.resolved_advance);
    assert_eq!("PunctuationGeometryLedger", info.source);
}

#[test]
fn spacing_plan_adjustments_consume_by_target_and_anchor() {
    let clusters = vec![cluster("「", 0, EM, "cjk"), cluster("「", 1, EM, "cjk")];
    let atoms = atoms_for(&clusters);
    let stray = PunctuationSpacingAdjustment {
        range: text_range(90, 91),
        reduction_target_range: text_range(90, 91),
        left_char: '。',
        right_char: '「',
        natural_inner_glue: 8.0,
        adjusted_inner_glue: 0.0,
        reduction: 8.0,
        reason: "stray".to_owned(),
    };
    let stray_ledger = PunctuationGeometryLedger::from(
        clusters.clone(),
        &atoms,
        &PunctuationSpacingCompressionResult::new(vec![stray]),
    );
    assert_eq!(2, stray_ledger.glue_capacities().len());

    let leading_target = PunctuationSpacingAdjustment {
        range: text_range(0, 1),
        reduction_target_range: text_range(0, 1),
        left_char: '「',
        right_char: '「',
        natural_inner_glue: 8.0,
        adjusted_inner_glue: 4.0,
        reduction: 4.0,
        reason: "leading-side".to_owned(),
    };
    let leading_ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(vec![leading_target]),
    );
    assert_eq!(4.0, leading_ledger.glue_capacities()[&0].leading);
    assert_eq!(8.0, leading_ledger.glue_capacities()[&1].leading);

    let builder = PunctuationAtomBuilder::default();
    let centred = builder
        .build(
            '·',
            text_range(0, 1),
            EM,
            Some(
                PunctuationInkInput::builder(16.0)
                    .ink_bounds(Some(Rect { left: 2.0, top: 4.0, right: 10.0, bottom: 12.0 }))
                    .halt_advance(Some(8.0))
                    .halt_placement_x(Some(-2.0))
                    .build(),
            ),
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        )
        .unwrap();
    let centred_clusters = vec![cluster("·", 0, EM, "cjk"), cluster("中", 1, EM, "cjk")];
    let centred_target = PunctuationSpacingAdjustment {
        range: text_range(0, 1),
        reduction_target_range: text_range(0, 1),
        left_char: '·',
        right_char: '中',
        natural_inner_glue: 8.0,
        adjusted_inner_glue: 2.0,
        reduction: 6.0,
        reason: "centred".to_owned(),
    };
    let centred_ledger = PunctuationGeometryLedger::from(
        centred_clusters,
        &[centred],
        &PunctuationSpacingCompressionResult::new(vec![centred_target]),
    );
    let capacity = centred_ledger.glue_capacities()[&0];
    assert!(capacity.paired);
    assert_eq!(0.0, capacity.leading);
    assert_eq!(4.0, capacity.trailing);
}

#[test]
fn attached_inline_boundaries_require_alignment_and_run_only_with_attachments() {
    let ledger = ledger_of(&["。", "中"]);
    assert!(std::panic::catch_unwind(|| {
        ledger.resolve_attached_inline_punctuation_boundaries(
            &[InlineAttachment::None],
            &[],
            EM,
        )
    })
    .is_err());

    let none = ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::None],
        &[],
        EM,
    );
    assert!(none.decisions.is_empty());
    assert!(none.trailing_glue_by_cluster.is_empty());

    let plain = ledger_of(&["中", "中"]);
    let attached = plain.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous],
        &[],
        EM,
    );
    assert!(attached.decisions.is_empty());
}

#[test]
fn attached_inline_boundary_at_line_end_consumes_trailing_glue() {
    let clusters = vec![cluster("」", 0, EM, "cjk"), cluster("ref", 1, EM, "latin")];
    let atoms = atoms_for(&clusters);
    let ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let result = ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous],
        &atoms,
        EM,
    );
    let decision = &result.decisions[0];
    assert_eq!(text_range(0, 4), decision.range);
    assert_eq!('」', decision.left_char);
    assert_eq!('\0', decision.right_char);
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:line-end", decision.reason);
    assert_eq!(8.0, decision.reduction);
    assert_eq!(8.0, result.geometry.resolve_clusters()[0].advance);
    assert!(result.trailing_glue_by_cluster.is_empty());
}

#[test]
fn attached_inline_boundary_adjacent_punctuation_halves_virtual_glue() {
    let clusters = vec![
        cluster("」", 0, EM, "cjk"),
        cluster("ref", 1, EM, "latin"),
        cluster("「", 4, EM, "cjk"),
    ];
    let atoms = atoms_for(&clusters);
    let ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let attachments = [InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None];
    let result = ledger.resolve_attached_inline_punctuation_boundaries(&attachments, &atoms, EM);
    let decision = &result.decisions[0];
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:adjacent-punctuation", decision.reason);
    assert_eq!(16.0, decision.natural_inner_glue);
    assert_eq!(8.0, decision.adjusted_inner_glue);
    assert_eq!(8.0, decision.reduction);
    assert_eq!(text_range(0, 5), decision.range);

    let bitten = ledger
        .consume_trailing_by_cluster(&HashMap::from([(0, 4.0)]))
        .resolve_attached_inline_punctuation_boundaries(&attachments, &atoms, EM);
    assert_eq!(12.0, bitten.decisions[0].natural_inner_glue);
    assert_eq!(4.0, bitten.decisions[0].adjusted_inner_glue);
    assert!(bitten.trailing_glue_by_cluster.is_empty());
    assert_eq!(4.0, bitten.geometry.glue_capacities()[&2].leading);
}

#[test]
fn attached_inline_boundary_before_ascii_point_mark_collapses_like_adjacent() {
    let clusters = vec![
        cluster("」", 0, EM, "cjk"),
        cluster("ref", 1, EM, "latin"),
        cluster(",", 4, EM, "latin"),
    ];
    let atoms = atoms_for(&clusters);
    let ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let result = ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
        &atoms,
        EM,
    );
    let decision = &result.decisions[0];
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:ascii-point-mark", decision.reason);
    assert_eq!(8.0, decision.natural_inner_glue);
    assert_eq!(0.0, decision.adjusted_inner_glue);
    assert_eq!(',', decision.right_char);
}

#[test]
fn attached_inline_boundary_skips_mandatory_break_neighbour() {
    let mut mandatory = cluster("\n", 4, 0.0, "mandatory-break");
    mandatory.display_text = Text::from("");
    let clusters = vec![cluster("」", 0, EM, "cjk"), cluster("ref", 1, EM, "latin"), mandatory];
    let atoms = atoms_for(&clusters);
    let ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let result = ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
        &atoms,
        EM,
    );
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:line-end", result.decisions[0].reason);
}

#[test]
fn attached_inline_boundary_without_glue_emits_no_decision() {
    let clusters = vec![
        cluster("「", 0, EM, "cjk"),
        cluster("ref", 1, EM, "latin"),
        cluster("中", 4, EM, "cjk"),
    ];
    let atoms = atoms_for(&clusters);
    let ledger = PunctuationGeometryLedger::from(
        clusters,
        &atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let result = ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
        &atoms,
        EM,
    );
    assert!(result.decisions.is_empty());

    let closing = vec![
        cluster("」", 0, EM, "cjk"),
        cluster("ref", 1, EM, "latin"),
        cluster("中", 4, EM, "cjk"),
    ];
    let closing_atoms = atoms_for(&closing);
    let closing_ledger = PunctuationGeometryLedger::from(
        closing,
        &closing_atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let natural = closing_ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
        &closing_atoms,
        EM,
    );
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:natural", natural.decisions[0].reason);
    assert_eq!(8.0, natural.decisions[0].natural_inner_glue);
    assert_eq!(8.0, natural.decisions[0].adjusted_inner_glue);

    let wide_atoms: Vec<_> = closing_atoms
        .iter()
        .cloned()
        .map(|mut atom| {
            if atom.character == '」' {
                atom.trailing_glue.natural = 12.0;
                atom.trailing_glue.max = 12.0;
            }
            atom
        })
        .collect();
    let closing = vec![
        cluster("」", 0, EM, "cjk"),
        cluster("ref", 1, EM, "latin"),
        cluster("中", 4, EM, "cjk"),
    ];
    let wide_ledger = PunctuationGeometryLedger::from(
        closing,
        &wide_atoms,
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let residual = wide_ledger.resolve_attached_inline_punctuation_boundaries(
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
        &wide_atoms,
        EM,
    );
    assert_eq!(HashMap::from([(1, 12.0)]), residual.trailing_glue_by_cluster);
    assert_eq!(28.0, residual.geometry.resolve_clusters()[1].advance);
}

#[test]
fn line_edge_trim_consumes_half_width_at_edges_and_skips_empty_inputs() {
    let ledger = ledger_of(&["」", "中"]);
    assert!(ledger.consume_line_edge_glue(&[], true).decisions.is_empty());
    let plain = ledger_of(&["中", "中"]);
    assert!(plain.consume_line_edge_glue(&[line(IntRange::new(0, 1), 0, 2)], true).decisions.is_empty());
    assert!(ledger.consume_line_edge_glue(&[line(IntRange::new(1, 0), 0, 0)], true).decisions.is_empty());

    let trimmed = ledger.consume_line_edge_glue(&[line(IntRange::new(0, 0), 0, 1)], true);
    let decision = &trimmed.decisions[0];
    assert_eq!("trailing", decision.side);
    assert_eq!(8.0, decision.trim_amount);
    assert_eq!(8.0, decision.natural_glue);
    assert_eq!("LineEndHalfWidthPunctuation", decision.reason);
    assert_eq!(text_range(0, 1), decision.cluster_range);
    assert_eq!(8.0, trimmed.geometry.resolve_clusters()[0].advance);

    let relaxed = ledger.consume_line_edge_glue(&[line(IntRange::new(0, 0), 0, 1)], false);
    assert!(relaxed.decisions.is_empty());
    assert_eq!(16.0, relaxed.geometry.resolve_clusters()[0].advance);
}

#[test]
fn line_edge_trim_consumes_centred_punctuation_once_per_line() {
    let builder = PunctuationAtomBuilder::default();
    let atom = builder
        .build(
            '·',
            text_range(0, 1),
            EM,
            Some(
                PunctuationInkInput::builder(16.0)
                    .ink_bounds(Some(Rect { left: 2.0, top: 4.0, right: 10.0, bottom: 12.0 }))
                    .halt_advance(Some(8.0))
                    .halt_placement_x(Some(-2.0))
                    .build(),
            ),
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        )
        .unwrap();
    let ledger = PunctuationGeometryLedger::from(
        vec![cluster("·", 0, EM, "cjk")],
        &[atom],
        &PunctuationSpacingCompressionResult::new(Vec::new()),
    );
    let trimmed = ledger.consume_line_edge_glue(&[line(IntRange::new(0, 0), 0, 1)], true);
    let decision = &trimmed.decisions[0];
    assert_eq!("both", decision.side);
    assert_eq!(4.0, decision.trim_amount);
    assert_eq!(8.0, decision.natural_glue);
    assert_eq!("LineEndCenteredPunctuationPairedCompression", decision.reason);
    let capacity = trimmed.geometry.glue_capacities()[&0];
    assert_eq!(0.0, capacity.leading);
    assert_eq!(4.0, capacity.trailing);
}

#[test]
fn cluster_index_range_finds_covered_clusters() {
    let clusters = vec![
        cluster("中", 0, EM, "cjk"),
        cluster("中", 1, EM, "cjk"),
        cluster("中", 2, EM, "cjk"),
    ];
    assert_eq!(None, cluster_index_range_for(&[], text_range(0, 3)));
    assert_eq!(Some((0, 2)), cluster_index_range_for(&clusters, text_range(0, 3)));
    assert_eq!(Some((1, 1)), cluster_index_range_for(&clusters, text_range(1, 2)));
    assert_eq!(None, cluster_index_range_for(&clusters, text_range(5, 6)));
    assert_eq!(Some((0, 0)), cluster_index_range_for(&clusters, text_range(0, 1)));
}
