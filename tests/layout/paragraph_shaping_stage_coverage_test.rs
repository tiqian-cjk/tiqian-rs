use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use tiqian::common::{HashMap, HashSet};
use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::clreq::clreq_profile::{BuiltInClreqProfileResolver, ClreqPunctuationGlyphSubstitutor};
use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::{text_range, LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_model::{
    Cluster, Glyph, GlyphRun, ShapingDecisionInfo, SyntheticClusterKind,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineBreakPolicy, LineBreakSpan, TextStyle, TiqianTextContent,
};
use tiqian::font::font_policy::{CjkFontRoleClassifier, FontRole};
use tiqian::layout::paragraph_shaping_stage::{
    is_inline_object_cluster, is_mandatory_break_cluster, is_zero_width_soft_break_cluster,
    map_to_cluster_range, shape_paragraph,
};
use tiqian::layout::cluster_role_resolution::ResolvedClusterRange;
use tiqian::layout::punctuation_model::{PunctuationAtomBuilder, PunctuationSpacingCompressor};
use tiqian::layout::progressive_break_decisions::ProgressiveBreakTier;
use tiqian::layout::quote_pair_analyzer::QuotePairAnalyzer;
use tiqian::layout::width_independent_annotation_cache::{
    build_paragraph_layout_prep, prepare_width_independent_annotation,
};
use tiqian::linebreak::hyphenation::Hyphenator;
use tiqian::shaping::font_backend::{
    FontBackend, FontBackendRequest, FontBackendShapingResult, FontResolution,
};
use tiqian::shaping::text_shaper::{
    ShapingResult, UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE,
};

use crate::support::DeterministicStubFontBackend;
use super::font_backend_test_support::stub_backend_with_transform;

#[test]
fn map_to_cluster_range_with_zero_and_positive_advance() {
    let cluster = Cluster::with_display_text(
        text_range(0, 4),
        Text::from("test"),
        Text::from("test"),
        FontFaceId::with_resource_id("k"),
        20.0,
    );

    let mapped_zero = map_to_cluster_range(
        &[
            Glyph::builder(1, text_range(0, 2), 0.0).build(),
            Glyph::builder(2, text_range(2, 4), 0.0).build(),
        ],
        &cluster,
    );
    assert_eq!(2, mapped_zero.len());
    assert_eq!(10.0, mapped_zero[0].advance);
    assert_eq!(10.0, mapped_zero[1].advance);
    assert_eq!(text_range(0, 4), mapped_zero[0].cluster_range);

    let mapped_normal = map_to_cluster_range(
        &[
            Glyph::builder(1, text_range(0, 2), 8.0).build(),
            Glyph::builder(2, text_range(2, 4), 12.0).x(8.0).build(),
        ],
        &cluster,
    );
    assert_eq!(2, mapped_normal.len());
    assert_eq!(8.0, mapped_normal[0].advance);
    assert_eq!(12.0, mapped_normal[1].advance);
}

#[test]
fn cluster_predicates_and_curly_quote_features() {
    let mandatory = Cluster::synthetic(
        text_range(0, 1),
        Text::from("\n"),
        Text::new(),
        SyntheticClusterKind::MandatoryBreak,
        0.0,
    );
    assert!(is_mandatory_break_cluster(&mandatory));
    assert!(!is_zero_width_soft_break_cluster(&mandatory));
    assert!(!is_inline_object_cluster(&mandatory));

    let zero_width = Cluster::synthetic(
        text_range(0, 1),
        Text::from("\u{200B}"),
        Text::new(),
        SyntheticClusterKind::ZeroWidthSoftBreak,
        0.0,
    );
    assert!(is_zero_width_soft_break_cluster(&zero_width));
    assert!(!is_mandatory_break_cluster(&zero_width));

    let inline_object = Cluster::synthetic(
        text_range(0, 1),
        Text::from("x"),
        Text::new(),
        SyntheticClusterKind::InlineObject,
        20.0,
    );
    assert!(is_inline_object_cluster(&inline_object));
    assert!(!is_mandatory_break_cluster(&inline_object));

    let normal = Cluster::with_display_text(
        text_range(0, 1),
        Text::from("中"),
        Text::from("中"),
        FontFaceId::with_resource_id("font"),
        16.0,
    );
    assert!(!is_mandatory_break_cluster(&normal));
    assert!(!is_zero_width_soft_break_cluster(&normal));
    assert!(!is_inline_object_cluster(&normal));

    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("“双引号”与‘单引号’")),
            LayoutConstraints::with_defaults(300.0),
        )
        .build(),
    );
    assert!(!result.clusters.is_empty());
}

#[test]
fn hyphen_advance_fallback_when_shaper_returns_empty_clusters() {
    let backend = stub_backend_with_transform(
        |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            if input.text.slice_text(input.range) == "-" || input.display_text == "-" {
                result.shaping.clusters.clear();
                result.shaping.glyph_runs.clear();
            }
            result
        },
    );

    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(backend)).build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("supercalifragilisticexpialidocious")),
            LayoutConstraints::with_defaults(50.0),
        )
        .build(),
    );
    assert!(!result.lines.is_empty());
}

#[test]
fn dash_substitution_rollback_and_coverage_branches() {
    let ink_coverage_backend = |ink_right: f32| {
        stub_backend_with_transform(move |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            let face = result.face.clone();
            result.shaping.clusters = vec![Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                face.clone(),
                32.0,
            )];
            result.shaping.glyph_runs = vec![GlyphRun::new(
                input.range,
                face.clone(),
                vec![Glyph::builder(1, input.range, 32.0)
                    .render_font_face(Some(face))
                    .bounds(Some(Rect {
                        left: 0.0,
                        top: 0.0,
                        right: ink_right,
                        bottom: 10.0,
                    }))
                    .build()],
                32.0,
            )];
            result
        })
    };

    let rollback_backend = {
        let calls = AtomicI32::new(0);
        stub_backend_with_transform(move |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            let source = input.text.slice_text(input.range);
            let face = result.face.clone();
            let cluster = Cluster::with_display_text(
                input.range,
                source.clone(),
                input.display_text.clone(),
                face.clone(),
                16.0,
            );
            let decision = ShapingDecisionInfo::builder(
                input.range,
                source,
                input.display_text.clone(),
                Some(face.clone()),
                1,
                16.0,
                "Test".to_owned(),
                "test".to_owned(),
            )
            .capability_issue((call == 1)
                .then(|| UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE.to_owned()))
            .missing_glyphs(if call == 2 { 1 } else { 0 })
            .build();
            if call == 2 {
                result.attempts[0].missing_glyphs = 1;
            }
            result.shaping = ShapingResult::with_decisions(
                vec![cluster],
                vec![GlyphRun::new(input.range, face, Vec::new(), 16.0)],
                vec![decision],
            );
            result
        })
    };

    let multi_and_null_glyph_backend = {
        let calls = AtomicI32::new(0);
        stub_backend_with_transform(move |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            let face = result.face.clone();
            let glyphs = match call % 3 {
                0 => Vec::new(),
                1 => vec![
                    Glyph::builder(1, input.range, 16.0)
                        .render_font_face(Some(face.clone()))
                        .build(),
                    Glyph::builder(2, input.range, 16.0)
                        .render_font_face(Some(face.clone()))
                        .x(16.0)
                        .build(),
                ],
                _ => vec![
                    Glyph::builder(1, input.range, 32.0)
                        .render_font_face(Some(face.clone()))
                        .build(),
                ],
            };
            result.shaping.glyph_runs = vec![GlyphRun::new(input.range, face, glyphs, 32.0)];
            result
        })
    };

    let layout = |text: &str, font_backend: Box<dyn FontBackend>| {
        let mut engine = ParagraphLayoutEngineBuilder::new(font_backend).build();
        engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(300.0),
            )
            .build(),
        )
    };

    assert!(!layout("——", Box::new(ink_coverage_backend(20.0)))
        .lines
        .is_empty());
    assert!(!layout("——", Box::new(ink_coverage_backend(30.0)))
        .lines
        .is_empty());
    assert!(!layout(
        "……",
        Box::new(rollback_backend),
    )
    .lines
    .is_empty());

    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(multi_and_null_glyph_backend)).build();
    for _ in 0..4 {
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("——")),
                LayoutConstraints::with_defaults(300.0),
            )
            .build(),
        );
        assert!(!result.lines.is_empty());
    }
}

struct HyphenWordHyphenator;

impl Hyphenator for HyphenWordHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        if word.as_str().contains("hyphen") {
            vec![2, 4]
        } else {
            Vec::new()
        }
    }
}

static HYPHEN_WORD_HYPHENATOR: HyphenWordHyphenator = HyphenWordHyphenator;

#[test]
fn latin_segmentation_and_cuts_branches() {
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .hyphenator(&HYPHEN_WORD_HYPHENATOR)
        .build();

    let inputs = [
        (
            "Text with ,Hello Machine2Machine XMLHttp HTTPServer TeX/LaTeX /start end/ /a a/ a/b https://example.com/path www.test.org sub.domain.co .com a. a..b a.b --.com test.-com test.c test.123 test.co123 12(3):45 12(3):45. 12(3):45-50 12(3):45–50 12(3):45—50 (1):2 a(1):2 1():2 1(2)a:3 1(2): 1(2):a-b 1(2):-5 1(2):5- 1(2):a 12():34 12(34): a(b):c-d 12(3):. 12a(3):45 12(3a):45 12(3):-45 12(3):45- 12(3):45-6a 12(3):4a-65 12(3):abc hyphenatedword VERYLONGALLCAPSWORDTHATISNOTANABBREVIATIONANDSHOULDBEOPAQ",
            80.0,
        ),
        ("antidisestablishmentarianism abc def xyz", 30.0),
        ("semi-conductor co-19 a-b 3-4 COVID-19 cross-module-link", 80.0),
        (
            "aaaaaaaaaaaaaaaa 0123456789abcdef a1b2c3d4e5f6g7h8 aaaaaa111111 aaaaaaaaaaaa1 a1",
            100.0,
        ),
        ("aBc ABc abC myIdentifier XML fooBAR aBC XMLHTTP", 100.0),
    ];
    for (text, max_width) in inputs {
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(max_width),
            )
            .build(),
        );
        assert!(!result.lines.is_empty());
    }
}

struct MachineHyphenator;

impl Hyphenator for MachineHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        if word.as_str().contains("Machine") {
            vec![
                -1,
                0,
                3,
                word.scalar_len().value(),
                word.scalar_len().value() + 1,
            ]
        } else {
            vec![2]
        }
    }
}

static MACHINE_HYPHENATOR: MachineHyphenator = MachineHyphenator;

#[test]
fn progressive_technical_span_breaks_and_tiers() {
    let text = "Machine2Machine /v2.0_alpha=beta&gamma supercalifragilisticexpialidocious short";
    let span = text_range(0, Text::from(text).scalar_len().value());
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from(text))
            .line_break_spans(vec![
                LineBreakSpan {
                    range: span,
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: text_range(5, 10),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
            ])
            .build(),
        LayoutConstraints::with_defaults(80.0),
    )
    .build();
    let clreq_profile_resolver = BuiltInClreqProfileResolver;
    let font_role_classifier = CjkFontRoleClassifier;
    let font_backend = DeterministicStubFontBackend::default();
    let quote_pair_analyzer = QuotePairAnalyzer;
    let hyphenator = &MACHINE_HYPHENATOR;
    let punctuation_atom_builder = PunctuationAtomBuilder::default();
    let punctuation_spacing_compressor = PunctuationSpacingCompressor;
    for tier in [
        ProgressiveBreakTier::Structural,
        ProgressiveBreakTier::Syllable,
        ProgressiveBreakTier::Emergency,
    ] {
        let rejected = HashMap::from([(span, HashSet::from([tier]))]);
        let annotation = prepare_width_independent_annotation(
            &input,
            &rejected,
            &clreq_profile_resolver,
            &font_role_classifier,
            &font_backend,
            &quote_pair_analyzer,
            hyphenator,
        );
        let prep = build_paragraph_layout_prep(
            &input,
            &annotation,
            &rejected,
            &font_backend,
            hyphenator,
            &punctuation_atom_builder,
            &punctuation_spacing_compressor,
        );
        assert!(!prep.clusters.is_empty());
    }

    let rejected = HashMap::from([(
        span,
        HashSet::from([
            ProgressiveBreakTier::Structural,
            ProgressiveBreakTier::Syllable,
        ]),
    )]);
    let annotation = prepare_width_independent_annotation(
        &input,
        &rejected,
        &clreq_profile_resolver,
        &font_role_classifier,
        &font_backend,
        &quote_pair_analyzer,
        hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        &font_backend,
        hyphenator,
        &punctuation_atom_builder,
        &punctuation_spacing_compressor,
    );
    assert!(!prep.clusters.is_empty());
}

struct OneTwoThreeHyphenator;

impl Hyphenator for OneTwoThreeHyphenator {
    fn hyphenate(&self, _: &Text) -> Vec<i32> {
        vec![1, 2, 3]
    }
}

static ONE_TWO_THREE_HYPHENATOR: OneTwoThreeHyphenator = OneTwoThreeHyphenator;

#[test]
fn multi_cluster_shaper_for_word_cuts_and_opaque_hard_cuts() {
    let split = AtomicBool::new(false);
    let backend = stub_backend_with_transform(
        move |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            if input.range.length() <= 1 || split.fetch_xor(true, Ordering::Relaxed) {
                return result;
            }
            let mid = input.range.start() + input.range.length() / 2;
            let face = result.face.clone();
            result.shaping.clusters = vec![
                Cluster::with_display_text(
                    TextRange::new(input.range.start(), mid),
                    Text::from("a"),
                    Text::from("a"),
                    face.clone(),
                    100.0,
                ),
                Cluster::with_display_text(
                    TextRange::new(mid, input.range.end()),
                    Text::from("b"),
                    Text::from("b"),
                    face,
                    100.0,
                ),
            ];
            result
        },
    );

    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(backend))
        .hyphenator(&ONE_TWO_THREE_HYPHENATOR)
        .build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(
                "antidisestablishmentarianism some_opaque_token_with_separators/and/more",
            )),
            LayoutConstraints::with_defaults(20.0),
        )
        .build(),
    );
    assert!(!result.lines.is_empty());
}

#[test]
fn latin_separator_cuts_and_solidus_branches() {
    let text = "http://example.com/path a/b /start end/ a//b foo_bar";
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build();
    for max_width in [500.0, 1.0] {
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(max_width),
            )
            .build(),
        );
        assert!(!result.lines.is_empty());
    }
}

struct LatinWordCutsHyphenator;

impl Hyphenator for LatinWordCutsHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        match word.as_str() {
            "abcdef" => vec![1],
            "ghijkl" => vec![2],
            "mnopqr" => vec![3],
            "empty" => vec![2],
            _ => Vec::new(),
        }
    }
}

static LATIN_WORD_CUTS_HYPHENATOR: LatinWordCutsHyphenator = LatinWordCutsHyphenator;

#[test]
fn latin_word_cuts_lo_hi_and_empty_branches() {
    let backend = stub_backend_with_transform(
        |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            if input.range.length() != 2 || input.text.slice_text(input.range) != "em" {
                return result;
            }
            let face = result.face.clone();
            result.shaping.clusters = vec![
                Cluster::with_display_text(
                    TextRange::new(input.range.start(), input.range.start() + 1),
                    Text::from("e"),
                    Text::from("e"),
                    face.clone(),
                    10.0,
                ),
                Cluster::with_display_text(
                    TextRange::new(input.range.start() + 1, input.range.end()),
                    Text::from("m"),
                    Text::from("m"),
                    face,
                    10.0,
                ),
            ];
            result
        },
    );

    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(backend))
        .hyphenator(&LATIN_WORD_CUTS_HYPHENATOR)
        .build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("abcdef ghijkl mnopqr empty")),
            LayoutConstraints::with_defaults(1.0),
        )
        .build(),
    );
    assert!(!result.lines.is_empty());
}

struct DirectShapeHyphenator;

impl Hyphenator for DirectShapeHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        match word.as_str() {
            "abcdef" => vec![1],
            "abcdeg" => vec![2],
            "antidisestablishmentarianism" => Vec::new(),
            "Machine" => vec![
                -1,
                0,
                2,
                word.scalar_len().value(),
                word.scalar_len().value() + 2,
            ],
            _ => vec![2],
        }
    }
}

static DIRECT_SHAPE_HYPHENATOR: DirectShapeHyphenator = DirectShapeHyphenator;

#[test]
fn direct_shape_paragraph_edge_cases() {
    let backend = stub_backend_with_transform(
        |input: &FontBackendRequest, mut result: FontBackendShapingResult| {
            if input.text.slice_text(input.range) == "singlecluster"
                || input.display_text == "singlecluster"
            {
                result.shaping.clusters.clear();
            }
            result
        },
    );

    let text = Text::from(
        "abcdef abcdeg antidisestablishmentarianism singlecluster Machine2Machine /a/b/c 12(3):. 12a(3):45 12(3a):45 12(3):-45 12(3):45- 12(3):45-6a 12(3):4a-65 12(3):abc aaaaaa111111 a1b2c3d4e5f6 http://example.com/foo https://example.com/foo?a=1&b=2#x%20~y abc.d abc.12 abc.de abc.de12 --.com foo.-bar /start end/ a/b a//b",
    );
    let range = text_range(0, text.scalar_len().value());
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text.clone())
            .line_break_spans(vec![LineBreakSpan {
                range: text_range(0, 10),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .build(),
        LayoutConstraints::with_defaults(1.0),
    )
    .build();
    let substitutor = ClreqPunctuationGlyphSubstitutor::default();
    let style = |_| TextStyle::builder().font_size(16.0).build();
    let cached_segment_shaping: HashMap<TextRange, ShapingResult> = HashMap::new();
    let cached_font_resolutions: HashMap<TextRange, FontResolution> = HashMap::new();
    let cached_rollbacks: HashMap<TextRange, String> = HashMap::new();

    let latin = shape_paragraph(
        &backend,
        &DIRECT_SHAPE_HYPHENATOR,
        &input,
        &text,
        16.0,
        1.0,
        &[ResolvedClusterRange::new(range, FontRole::LatinText)],
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| true,
        &HashMap::new(),
        &cached_segment_shaping,
        &cached_font_resolutions,
        &cached_rollbacks,
    );
    assert!(!latin.shaping_results.is_empty());

    let cjk = shape_paragraph(
        &backend,
        &DIRECT_SHAPE_HYPHENATOR,
        &input,
        &text,
        16.0,
        40.0,
        &[ResolvedClusterRange::new(range, FontRole::CjkText)],
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| false,
        &HashMap::new(),
        &cached_segment_shaping,
        &cached_font_resolutions,
        &cached_rollbacks,
    );
    assert!(!cjk.shaping_results.is_empty());

    let space = Text::from(" ");
    let space_range = text_range(0, 1);
    let space_input = LayoutInput::builder(
        TiqianTextContent::new(space.clone()),
        LayoutConstraints::with_defaults(100.0),
    )
    .build();
    let space_result = shape_paragraph(
        &backend,
        &DIRECT_SHAPE_HYPHENATOR,
        &space_input,
        &space,
        16.0,
        100.0,
        &[ResolvedClusterRange::new(space_range, FontRole::LatinText)],
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| false,
        &HashMap::new(),
        &cached_segment_shaping,
        &cached_font_resolutions,
        &cached_rollbacks,
    );
    assert!(!space_result.shaping_results.is_empty());
}

struct ProgressivePriorityHyphenator;

impl Hyphenator for ProgressivePriorityHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        if word == "abcdef" {
            vec![2, 4]
        } else {
            vec![
                -1,
                0,
                1,
                2,
                word.scalar_len().value(),
                word.scalar_len().value() + 2,
            ]
        }
    }
}

static PROGRESSIVE_PRIORITY_HYPHENATOR: ProgressivePriorityHyphenator =
    ProgressivePriorityHyphenator;

#[test]
fn progressive_technical_tier_priority_and_false_branches() {
    let text = Text::from("abcdef/ghijkl");
    let span = text_range(0, 13);
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text.clone())
            .line_break_spans(vec![
                LineBreakSpan {
                    range: text_range(0, 2),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: span,
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: text_range(10, 13),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
            ])
            .build(),
        LayoutConstraints::with_defaults(10.0),
    )
    .build();
    let backend = DeterministicStubFontBackend::default();
    let ranges = [
        text_range(0, 7),
        text_range(2, 7),
        text_range(0, 0),
    ];
    let cached_segment_shaping: HashMap<TextRange, ShapingResult> = HashMap::new();
    let cached_font_resolutions: HashMap<TextRange, FontResolution> = HashMap::new();
    let cached_rollbacks: HashMap<TextRange, String> = HashMap::new();
    let result = shape_paragraph(
        &backend,
        &PROGRESSIVE_PRIORITY_HYPHENATOR,
        &input,
        &text,
        16.0,
        10.0,
        &[
            ResolvedClusterRange::new(ranges[0], FontRole::LatinText),
            ResolvedClusterRange::new(ranges[1], FontRole::LatinText),
            ResolvedClusterRange::new(ranges[2], FontRole::LatinText),
        ],
        &HashMap::new(),
        &ClreqPunctuationGlyphSubstitutor::default(),
        &|_| TextStyle::builder().font_size(16.0).build(),
        &|_| false,
        &HashMap::from([(
            span,
            HashSet::from([
                ProgressiveBreakTier::Structural,
                ProgressiveBreakTier::Syllable,
            ]),
        )]),
        &cached_segment_shaping,
        &cached_font_resolutions,
        &cached_rollbacks,
    );
    assert!(!result.shaping_results.is_empty());
}

struct HyphenatedWordHyphenator;

impl Hyphenator for HyphenatedWordHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        if word == "hyphenated" {
            vec![3, 6]
        } else {
            Vec::new()
        }
    }
}

static HYPHENATED_WORD_HYPHENATOR: HyphenatedWordHyphenator = HyphenatedWordHyphenator;

#[test]
fn latin_separator_cuts_exhaustive_branches() {
    let text = "12(3):45-67 12(3):45–67 12(3):45—67 12(3):45 12(3):. 12():45 12(3): :(3):45 12(3):- 12(3):45- 12(3):4a-65 12(3):45-6a 12(3):abc http://example.com/a/b/c https://test.org:8080/foo?bar=1&baz=2#frag%20~val+1*2|3;4,5.6-7_8 http:/test /a a/ a//b a/b ABC CamelCase aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa hyphenated-word clean/solidus hyphenated";
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .hyphenator(&HYPHENATED_WORD_HYPHENATOR)
        .build();
    for max_width in [500.0, 10.0] {
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(max_width),
            )
            .build(),
        );
        assert!(!result.lines.is_empty());
    }
}

struct TierRevisitHyphenator;

impl Hyphenator for TierRevisitHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        match word.as_str() {
            "abcdef" => vec![2, 4],
            "cdef" => vec![1],
            _ => Vec::new(),
        }
    }
}

static TIER_REVISIT_HYPHENATOR: TierRevisitHyphenator = TierRevisitHyphenator;

#[test]
fn progressive_tier_loop_revisits_offsets_with_lower_priority_tiers() {
    let text = Text::from("abcdef/");
    let span = text_range(0, 7);
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text.clone())
            .line_break_spans(vec![LineBreakSpan {
                range: span,
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .build(),
        LayoutConstraints::with_defaults(4.0),
    )
    .build();
    let ranges = [text_range(0, 7), text_range(2, 7)];
    let backend = DeterministicStubFontBackend::default();
    let cached_segment_shaping: HashMap<TextRange, ShapingResult> = HashMap::new();
    let cached_font_resolutions: HashMap<TextRange, FontResolution> = HashMap::new();
    let cached_rollbacks: HashMap<TextRange, String> = HashMap::new();
    let result = shape_paragraph(
        &backend,
        &TIER_REVISIT_HYPHENATOR,
        &input,
        &text,
        16.0,
        4.0,
        &[
            ResolvedClusterRange::new(ranges[0], FontRole::LatinText),
            ResolvedClusterRange::new(ranges[1], FontRole::LatinText),
        ],
        &HashMap::new(),
        &ClreqPunctuationGlyphSubstitutor::default(),
        &|_| TextStyle::builder().font_size(16.0).build(),
        &|_| false,
        &HashMap::new(),
        &cached_segment_shaping,
        &cached_font_resolutions,
        &cached_rollbacks,
    );
    assert!(!result.shaping_results.is_empty());
}

#[test]
fn latin_separator_tokens_cover_url_leading_slash_and_dash_locators() {
    for token in ["//example.com/a", "12(3):45–67", "12(3):45—67"] {
        let text = Text::from(token);
        let range = text_range(0, text.scalar_len().value());
        let input = LayoutInput::builder(
            TiqianTextContent::new(text.clone()),
            LayoutConstraints::with_defaults(500.0),
        )
        .build();
        let backend = DeterministicStubFontBackend::default();
        let cached_segment_shaping: HashMap<TextRange, ShapingResult> = HashMap::new();
        let cached_font_resolutions: HashMap<TextRange, FontResolution> = HashMap::new();
        let cached_rollbacks: HashMap<TextRange, String> = HashMap::new();
        for measure in [500.0, 8.0] {
            let result = shape_paragraph(
                &backend,
                &HYPHENATED_WORD_HYPHENATOR,
                &input,
                &text,
                16.0,
                measure,
                &[ResolvedClusterRange::new(range, FontRole::LatinText)],
                &HashMap::new(),
                &ClreqPunctuationGlyphSubstitutor::default(),
                &|_| TextStyle::builder().font_size(16.0).build(),
                &|_| false,
                &HashMap::new(),
                &cached_segment_shaping,
                &cached_font_resolutions,
                &cached_rollbacks,
            );
            assert!(!result.shaping_results.is_empty());
        }
    }
}