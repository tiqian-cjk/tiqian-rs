use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use tiqian::common::{HashMap, HashSet};
use tiqian::clreq::clreq_profile::ClreqPunctuationGlyphSubstitutor;
use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineBreakPolicy, LineBreakSpan, TextStyle, TiqianTextContent,
};
use tiqian::font::font_policy::{FontCandidate, FontDecision, FontRole};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::paragraph_shaping_stage::{
    is_inline_object_cluster, is_mandatory_break_cluster, is_zero_width_soft_break_cluster,
    map_to_cluster_range, shape_paragraph,
};
use tiqian::layout::cluster_role_resolution::ResolvedClusterRange;
use tiqian::layout::progressive_break_decisions::ProgressiveBreakTier;
use tiqian::layout::width_independent_annotation_cache::{
    build_paragraph_layout_prep, prepare_width_independent_annotation,
};
use tiqian::linebreak::hyphenation::Hyphenator;
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
    UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE,
};

#[test]
fn map_to_cluster_range_with_zero_and_positive_advance() {
    let cluster = Cluster::with_display_text(
        TextRange::new(0, 4),
        Text::from("test"),
        Text::from("test"),
        "k".to_owned(),
        20.0,
    );

    let mapped_zero = map_to_cluster_range(
        &[
            Glyph::builder(1, TextRange::new(0, 2), 0.0).build(),
            Glyph::builder(2, TextRange::new(2, 4), 0.0).build(),
        ],
        &cluster,
    );
    assert_eq!(2, mapped_zero.len());
    assert_eq!(10.0, mapped_zero[0].advance);
    assert_eq!(10.0, mapped_zero[1].advance);
    assert_eq!(TextRange::new(0, 4), mapped_zero[0].cluster_range);

    let mapped_normal = map_to_cluster_range(
        &[
            Glyph::builder(1, TextRange::new(0, 2), 8.0).build(),
            Glyph::builder(2, TextRange::new(2, 4), 12.0).x(8.0).build(),
        ],
        &cluster,
    );
    assert_eq!(2, mapped_normal.len());
    assert_eq!(8.0, mapped_normal[0].advance);
    assert_eq!(12.0, mapped_normal[1].advance);
}

#[test]
fn cluster_predicates_and_curly_quote_features() {
    let mandatory = Cluster::with_display_text(
        TextRange::new(0, 1),
        Text::from("\n"),
        Text::new(),
        "mandatory-break".to_owned(),
        0.0,
    );
    assert!(is_mandatory_break_cluster(&mandatory));
    assert!(!is_zero_width_soft_break_cluster(&mandatory));
    assert!(!is_inline_object_cluster(&mandatory));

    let zero_width = Cluster::with_display_text(
        TextRange::new(0, 1),
        Text::from("\u{200B}"),
        Text::new(),
        "zero-width-space".to_owned(),
        0.0,
    );
    assert!(is_zero_width_soft_break_cluster(&zero_width));
    assert!(!is_mandatory_break_cluster(&zero_width));

    let inline_object = Cluster::with_display_text(
        TextRange::new(0, 1),
        Text::from("x"),
        Text::new(),
        "inline-object".to_owned(),
        20.0,
    );
    assert!(is_inline_object_cluster(&inline_object));
    assert!(!is_mandatory_break_cluster(&inline_object));

    let normal = Cluster::with_display_text(
        TextRange::new(0, 1),
        Text::from("中"),
        Text::from("中"),
        "font".to_owned(),
        16.0,
    );
    assert!(!is_mandatory_break_cluster(&normal));
    assert!(!is_zero_width_soft_break_cluster(&normal));
    assert!(!is_inline_object_cluster(&normal));

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
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
    struct NoClusterHyphenShaper;

    impl TextShaper for NoClusterHyphenShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            if input.text.slice_text(input.range) == "-" || input.display_text == "-" {
                return ShapingResult::new(Vec::new(), Vec::new());
            }
            ExplainableStubTextShaper.shape(input)
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(NoClusterHyphenShaper);
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
    struct InkCoverageShaper {
        ink_right: f32,
    }

    impl TextShaper for InkCoverageShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let cluster = Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                "test".to_owned(),
                32.0,
            );
            let glyph = Glyph::builder(1, input.range, 32.0)
                .bounds(Some(Rect {
                    left: 0.0,
                    top: 0.0,
                    right: self.ink_right,
                    bottom: 10.0,
                }))
                .build();
            ShapingResult::new(
                vec![cluster],
                vec![GlyphRun::new(
                    input.range,
                    "test".to_owned(),
                    vec![glyph],
                    32.0,
                )],
            )
        }
    }

    struct RollbackShaper {
        calls: AtomicI32,
    }

    impl TextShaper for RollbackShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            let source = input.text.slice_text(input.range);
            let cluster = Cluster::with_display_text(
                input.range,
                source.clone(),
                input.display_text.clone(),
                "test".to_owned(),
                16.0,
            );
            let decision = ShapingDecisionInfo::builder(
                input.range,
                source,
                input.display_text.clone(),
                "test".to_owned(),
                1,
                16.0,
                "Test".to_owned(),
                "test".to_owned(),
            )
            .capability_issue((call == 1)
                .then(|| UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE.to_owned()))
            .missing_glyphs(if call == 2 { 1 } else { 0 })
            .build();
            ShapingResult::with_decisions(
                vec![cluster],
                vec![GlyphRun::new(
                    input.range,
                    "test".to_owned(),
                    Vec::new(),
                    16.0,
                )],
                vec![decision],
            )
        }
    }

    struct MultiAndNullGlyphShaper {
        calls: AtomicI32,
    }

    impl TextShaper for MultiAndNullGlyphShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let call = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            let glyphs = match call % 3 {
                0 => Vec::new(),
                1 => vec![
                    Glyph::builder(1, input.range, 16.0).build(),
                    Glyph::builder(2, input.range, 16.0).x(16.0).build(),
                ],
                _ => vec![Glyph::builder(1, input.range, 32.0).build()],
            };
            ShapingResult::new(
                vec![Cluster::with_display_text(
                    input.range,
                    input.text.slice_text(input.range),
                    input.display_text.clone(),
                    "test".to_owned(),
                    32.0,
                )],
                vec![GlyphRun::new(
                    input.range,
                    "test".to_owned(),
                    glyphs,
                    32.0,
                )],
            )
        }
    }

    let layout = |text: &str, text_shaper: Box<dyn TextShaper>| {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.text_shaper = text_shaper;
        engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(300.0),
            )
            .build(),
        )
    };

    assert!(!layout("——", Box::new(InkCoverageShaper { ink_right: 20.0 }))
        .lines
        .is_empty());
    assert!(!layout("——", Box::new(InkCoverageShaper { ink_right: 30.0 }))
        .lines
        .is_empty());
    assert!(!layout(
        "……",
        Box::new(RollbackShaper {
            calls: AtomicI32::new(0),
        }),
    )
    .lines
    .is_empty());

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(MultiAndNullGlyphShaper {
        calls: AtomicI32::new(0),
    });
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
        if word.contains("hyphen") {
            vec![2, 4]
        } else {
            Vec::new()
        }
    }
}

static HYPHEN_WORD_HYPHENATOR: HyphenWordHyphenator = HyphenWordHyphenator;

#[test]
fn latin_segmentation_and_cuts_branches() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &HYPHEN_WORD_HYPHENATOR;

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
        if word.contains("Machine") {
            vec![-1, 0, 3, word.utf16_len(), word.utf16_len() + 1]
        } else {
            vec![2]
        }
    }
}

static MACHINE_HYPHENATOR: MachineHyphenator = MachineHyphenator;

#[test]
fn progressive_technical_span_breaks_and_tiers() {
    let text = "Machine2Machine /v2.0_alpha=beta&gamma supercalifragilisticexpialidocious short";
    let span = TextRange::new(0, text.len() as i32);
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from(text))
            .line_break_spans(vec![
                LineBreakSpan {
                    range: span,
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: TextRange::new(5, 10),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
            ])
            .build(),
        LayoutConstraints::with_defaults(80.0),
    )
    .build();
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &MACHINE_HYPHENATOR;

    for tier in [
        ProgressiveBreakTier::Structural,
        ProgressiveBreakTier::Syllable,
        ProgressiveBreakTier::Emergency,
    ] {
        let rejected = HashMap::from([(span, HashSet::from([tier]))]);
        let annotation = prepare_width_independent_annotation(
            &input,
            &rejected,
            engine.clreq_profile_resolver.as_ref(),
            engine.font_role_classifier.as_ref(),
            engine.fallback_resolver.as_ref(),
            engine.font_metrics_resolver.as_ref(),
            &engine.quote_pair_analyzer,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
        );
        let prep = build_paragraph_layout_prep(
            &input,
            &annotation,
            &rejected,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
            &engine.punctuation_atom_builder,
            &engine.punctuation_spacing_compressor,
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
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
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
    struct MultiClusterShaper {
        split: AtomicBool,
    }

    impl TextShaper for MultiClusterShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let result = ExplainableStubTextShaper.shape(input);
            if input.range.length() <= 1 || self.split.fetch_xor(true, Ordering::Relaxed) {
                return result;
            }
            let mid = (input.range.start() + input.range.end()) / 2;
            ShapingResult::new(
                vec![
                    Cluster::with_display_text(
                        TextRange::new(input.range.start(), mid),
                        Text::from("a"),
                        Text::from("a"),
                        "k".to_owned(),
                        100.0,
                    ),
                    Cluster::with_display_text(
                        TextRange::new(mid, input.range.end()),
                        Text::from("b"),
                        Text::from("b"),
                        "k".to_owned(),
                        100.0,
                    ),
                ],
                result.glyph_runs,
            )
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(MultiClusterShaper {
        split: AtomicBool::new(false),
    });
    engine.hyphenator = &ONE_TWO_THREE_HYPHENATOR;
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
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
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
    struct WordShaper;

    impl TextShaper for WordShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let result = ExplainableStubTextShaper.shape(input);
            if input.range.length() != 2 || input.text.slice_text(input.range) != "em" {
                return result;
            }
            ShapingResult {
                clusters: vec![
                    Cluster::with_display_text(
                        TextRange::new(input.range.start(), input.range.start() + 1),
                        Text::from("e"),
                        Text::from("e"),
                        "k".to_owned(),
                        10.0,
                    ),
                    Cluster::with_display_text(
                        TextRange::new(input.range.start() + 1, input.range.end()),
                        Text::from("m"),
                        Text::from("m"),
                        "k".to_owned(),
                        10.0,
                    ),
                ],
                ..result
            }
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &LATIN_WORD_CUTS_HYPHENATOR;
    engine.text_shaper = Box::new(WordShaper);
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
            "Machine" => vec![-1, 0, 2, word.utf16_len(), word.utf16_len() + 2],
            _ => vec![2],
        }
    }
}

static DIRECT_SHAPE_HYPHENATOR: DirectShapeHyphenator = DirectShapeHyphenator;

#[test]
fn direct_shape_paragraph_edge_cases() {
    struct EmptyClusterShaper;

    impl TextShaper for EmptyClusterShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let result = ExplainableStubTextShaper.shape(input);
            if input.text.slice_text(input.range) == "singlecluster"
                || input.display_text == "singlecluster"
            {
                return ShapingResult {
                    clusters: Vec::new(),
                    ..result
                };
            }
            result
        }
    }

    let text = Text::from(
        "abcdef abcdeg antidisestablishmentarianism singlecluster Machine2Machine /a/b/c 12(3):. 12a(3):45 12(3a):45 12(3):-45 12(3):45- 12(3):45-6a 12(3):4a-65 12(3):abc aaaaaa111111 a1b2c3d4e5f6 http://example.com/foo https://example.com/foo?a=1&b=2#x%20~y abc.d abc.12 abc.de abc.de12 --.com foo.-bar /start end/ a/b a//b",
    );
    let range = TextRange::new(0, text.utf16_len());
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text.clone())
            .line_break_spans(vec![LineBreakSpan {
                range: TextRange::new(0, 10),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .build(),
        LayoutConstraints::with_defaults(1.0),
    )
    .build();
    let latin_candidate = FontCandidate {
        key: "k".to_owned(),
        family: "f".to_owned(),
        role: FontRole::LatinText,
    };
    let latin_decision = FontDecision {
        range,
        candidate: latin_candidate,
        role: FontRole::LatinText,
        reason: "r".to_owned(),
    };
    let cjk_decision = FontDecision {
        range,
        candidate: FontCandidate {
            key: "k".to_owned(),
            family: "f".to_owned(),
            role: FontRole::CjkText,
        },
        role: FontRole::CjkText,
        reason: "r".to_owned(),
    };
    let shaper = EmptyClusterShaper;
    let substitutor = ClreqPunctuationGlyphSubstitutor::default();
    let style = |_| TextStyle::builder().font_size(16.0).build();

    let latin = shape_paragraph(
        &shaper,
        &DIRECT_SHAPE_HYPHENATOR,
        &input,
        &text,
        16.0,
        1.0,
        &[ResolvedClusterRange::new(range, FontRole::LatinText)],
        &HashMap::from([(range, latin_decision)]),
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| true,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(!latin.shaping_results.is_empty());

    let cjk = shape_paragraph(
        &shaper,
        &DIRECT_SHAPE_HYPHENATOR,
        &input,
        &text,
        16.0,
        40.0,
        &[ResolvedClusterRange::new(range, FontRole::CjkText)],
        &HashMap::from([(range, cjk_decision)]),
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(!cjk.shaping_results.is_empty());

    let space = Text::from(" ");
    let space_range = TextRange::new(0, 1);
    let space_input = LayoutInput::builder(
        TiqianTextContent::new(space.clone()),
        LayoutConstraints::with_defaults(100.0),
    )
    .build();
    let space_decision = FontDecision {
        range: space_range,
        candidate: FontCandidate {
            key: "k".to_owned(),
            family: "f".to_owned(),
            role: FontRole::LatinText,
        },
        role: FontRole::LatinText,
        reason: "r".to_owned(),
    };
    let space_result = shape_paragraph(
        &shaper,
        &DIRECT_SHAPE_HYPHENATOR,
        &space_input,
        &space,
        16.0,
        100.0,
        &[ResolvedClusterRange::new(space_range, FontRole::LatinText)],
        &HashMap::from([(space_range, space_decision)]),
        &HashMap::new(),
        &substitutor,
        &style,
        &|_| false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(!space_result.shaping_results.is_empty());
}

struct ProgressivePriorityHyphenator;

impl Hyphenator for ProgressivePriorityHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        if word == "abcdef" {
            vec![2, 4]
        } else {
            vec![-1, 0, 1, 2, word.utf16_len(), word.utf16_len() + 2]
        }
    }
}

static PROGRESSIVE_PRIORITY_HYPHENATOR: ProgressivePriorityHyphenator =
    ProgressivePriorityHyphenator;

#[test]
fn progressive_technical_tier_priority_and_false_branches() {
    let text = Text::from("abcdef/ghijkl");
    let span = TextRange::new(0, 13);
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text.clone())
            .line_break_spans(vec![
                LineBreakSpan {
                    range: TextRange::new(0, 2),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: span,
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
                LineBreakSpan {
                    range: TextRange::new(10, 13),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                },
            ])
            .build(),
        LayoutConstraints::with_defaults(10.0),
    )
    .build();
    let candidate = FontCandidate {
        key: "k".to_owned(),
        family: "f".to_owned(),
        role: FontRole::LatinText,
    };
    let ranges = [
        TextRange::new(0, 7),
        TextRange::new(2, 7),
        TextRange::new(0, 0),
    ];
    let decisions = HashMap::from([
        (
            ranges[0],
            FontDecision {
                range: ranges[0],
                candidate: candidate.clone(),
                role: FontRole::LatinText,
                reason: "r".to_owned(),
            },
        ),
        (
            ranges[1],
            FontDecision {
                range: ranges[1],
                candidate: candidate.clone(),
                role: FontRole::LatinText,
                reason: "r".to_owned(),
            },
        ),
        (
            ranges[2],
            FontDecision {
                range: ranges[2],
                candidate,
                role: FontRole::LatinText,
                reason: "r".to_owned(),
            },
        ),
    ]);
    let result = shape_paragraph(
        &ExplainableStubTextShaper,
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
        &decisions,
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
        &HashMap::new(),
        &HashMap::new(),
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
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &HYPHENATED_WORD_HYPHENATOR;
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
    let span = TextRange::new(0, 7);
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
    let ranges = [TextRange::new(0, 7), TextRange::new(2, 7)];
    let candidate = FontCandidate {
        key: "k".to_owned(),
        family: "f".to_owned(),
        role: FontRole::LatinText,
    };
    let decisions = HashMap::from([
        (
            ranges[0],
            FontDecision {
                range: ranges[0],
                candidate: candidate.clone(),
                role: FontRole::LatinText,
                reason: "r".to_owned(),
            },
        ),
        (
            ranges[1],
            FontDecision {
                range: ranges[1],
                candidate,
                role: FontRole::LatinText,
                reason: "r".to_owned(),
            },
        ),
    ]);
    let result = shape_paragraph(
        &ExplainableStubTextShaper,
        &TIER_REVISIT_HYPHENATOR,
        &input,
        &text,
        16.0,
        4.0,
        &[
            ResolvedClusterRange::new(ranges[0], FontRole::LatinText),
            ResolvedClusterRange::new(ranges[1], FontRole::LatinText),
        ],
        &decisions,
        &HashMap::new(),
        &ClreqPunctuationGlyphSubstitutor::default(),
        &|_| TextStyle::builder().font_size(16.0).build(),
        &|_| false,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    );
    assert!(!result.shaping_results.is_empty());
}

#[test]
fn latin_separator_tokens_cover_url_leading_slash_and_dash_locators() {
    for token in ["//example.com/a", "12(3):45–67", "12(3):45—67"] {
        let text = Text::from(token);
        let range = TextRange::new(0, text.utf16_len());
        let input = LayoutInput::builder(
            TiqianTextContent::new(text.clone()),
            LayoutConstraints::with_defaults(500.0),
        )
        .build();
        let decision = FontDecision {
            range,
            candidate: FontCandidate {
                key: "k".to_owned(),
                family: "f".to_owned(),
                role: FontRole::LatinText,
            },
            role: FontRole::LatinText,
            reason: "r".to_owned(),
        };
        for measure in [500.0, 8.0] {
            let result = shape_paragraph(
                &ExplainableStubTextShaper,
                &HYPHENATED_WORD_HYPHENATOR,
                &input,
                &text,
                16.0,
                measure,
                &[ResolvedClusterRange::new(range, FontRole::LatinText)],
                &HashMap::from([(range, decision.clone())]),
                &HashMap::new(),
                &ClreqPunctuationGlyphSubstitutor::default(),
                &|_| TextStyle::builder().font_size(16.0).build(),
                &|_| false,
                &HashMap::new(),
                &HashMap::new(),
                &HashMap::new(),
            );
            assert!(!result.shaping_results.is_empty());
        }
    }
}