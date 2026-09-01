use tiqian::common::HashSet;
use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    built_in_layout_profiles, link_address_display, ColorSpan, DecorationKind, DecorationSpan,
    InlineAttachment, InlineBoxOuterSpacing, InlineBoxSpan, InlineObjectBoundaryAdjustment,
    InlineObjectPreferredStretch, InlineObjectPreferredStretchKind, InlineObjectSpan,
    LastLineAlignment, LayoutInput, LayoutProfileId, LineBreakPolicy, LineBreakSpan,
    LineLengthGrid, MeasureAdaptiveFirstLineIndent, ParagraphStyle, RichTextBackgroundDrawStyle,
    RichTextBackgroundMetricPolicy, RichTextBackgroundPaint, RichTextLinePattern, RichTextPaint,
    RichTextRole, RichTextSpan, RubyKind, RubyLineHeightMode, RubySpan, TextSpan, TextStyle,
    TiqianTextContent, WritingMode, DEFAULT_EMPHASIS_DOT_GAP_EM,
    DEFAULT_INLINE_OBJECT_MINIMUM_CLEARANCE_EM, INLINE_OBJECT_REPLACEMENT_CHAR,
};

#[test]
fn test_tiqian_text_content_and_link_address_display() {
    let content = TiqianTextContent::builder(Text::from("Hello Tiqian"))
        .spans(vec![TextSpan { range: TextRange::new(0, 5), style: TextStyle::default() }])
        .source_boundaries(HashSet::from([0, 5, 12]))
        .line_break_spans(vec![LineBreakSpan {
            range: TextRange::new(0, 5), policy: LineBreakPolicy::ProgressiveTechnical,
        }])
        .auto_space_suppressed_ranges(vec![TextRange::new(6, 12)])
        .build();
    assert_eq!("Hello Tiqian", content.text.as_str());
    assert_eq!(1, content.spans.len());
    assert_eq!(3, content.source_boundaries.len());
    assert_eq!(1, content.line_break_spans.len());
    assert_eq!(1, content.auto_space_suppressed_ranges.len());
    for (display, target, expected) in [
        ("", "", false), ("tiqian.org", "", false), ("", "https://tiqian.org", false),
        ("tiqian.org", "tiqian.org", true), ("tiqian.org", "https://tiqian.org", true),
        ("tiqian.org", "http://tiqian.org", true), ("dev@tiqian.org", "mailto:dev@tiqian.org", true),
        ("tiqian.org", "https://other.org", false), ("tiqian.org", "ftp://tiqian.org", false),
    ] {
        assert_eq!(expected, link_address_display::displays_address(&Text::from(display), target));
    }
}

#[test]
fn test_spans_and_inline_box() {
    let line_break = LineBreakSpan { range: TextRange::new(0, 4), policy: LineBreakPolicy::ProgressiveTechnical };
    assert_eq!(TextRange::new(0, 4), line_break.range);
    assert_eq!(LineBreakPolicy::ProgressiveTechnical, line_break.policy);
    let inline_box = InlineBoxSpan::builder(TextRange::new(1, 3))
        .inline_start(2.0).inline_end(3.0).outer_spacing(InlineBoxOuterSpacing::Source).build();
    assert_eq!(TextRange::new(1, 3), inline_box.range);
    assert_eq!(2.0, inline_box.inline_start);
    assert_eq!(3.0, inline_box.inline_end);
    assert_eq!(InlineBoxOuterSpacing::Source, inline_box.outer_spacing);
    assert_eq!('\u{FFFC}', INLINE_OBJECT_REPLACEMENT_CHAR);
}

#[test]
fn test_inline_object_preferred_stretch_and_adjustment() {
    let stretch = InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 10.0, 15.0);
    assert_eq!(InlineObjectPreferredStretchKind::Relation, stretch.kind);
    assert_eq!(5.0, stretch.capacity());
    for (natural, target) in [(-1.0, 10.0), (f32::NAN, 10.0), (f32::INFINITY, 10.0), (10.0, 10.0), (10.0, 8.0), (10.0, f32::NAN), (10.0, f32::INFINITY)] {
        assert!(std::panic::catch_unwind(|| InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::PunctuationTrailing, natural, target)).is_err());
    }
    let fixed = InlineObjectBoundaryAdjustment::FIXED;
    assert!(!fixed.participates_in_uniform_stretch);
    assert_eq!(None, fixed.preferred_stretch);
    let adjustment = InlineObjectBoundaryAdjustment::builder().participates_in_uniform_stretch(true).preferred_stretch(stretch).shrink_capacity(2.0).line_end_discardable_advance(1.0).prevents_line_break(true).build();
    assert!(adjustment.participates_in_uniform_stretch);
    assert_eq!(Some(stretch), adjustment.preferred_stretch);
    assert_eq!(2.0, adjustment.shrink_capacity);
    assert_eq!(1.0, adjustment.line_end_discardable_advance);
    assert!(adjustment.prevents_line_break);
    assert!(std::panic::catch_unwind(|| InlineObjectBoundaryAdjustment::builder().shrink_capacity(-0.5).build()).is_err());
    let object = InlineObjectSpan::new(TextRange::new(0, 1), 16.0, 12.0, 4.0, fixed, adjustment);
    assert_eq!(16.0, object.advance);
    assert_eq!(12.0, object.ascent);
}

#[test]
fn test_text_style_and_decorations() {
    let style = TextStyle::builder().font_families(vec!["Noto Serif CJK SC".to_owned()]).font_size(18.0).locale("zh-CN".to_owned()).font_weight(700).italic(true).baseline_shift(-2.0).inline_attachment(InlineAttachment::Previous).build();
    assert_eq!(vec!["Noto Serif CJK SC"], style.font_families);
    assert_eq!(18.0, style.font_size);
    assert_eq!("zh-CN", style.locale);
    assert_eq!(700, style.font_weight);
    assert!(style.italic);
    assert_eq!(-2.0, style.baseline_shift);
    assert_eq!(InlineAttachment::Previous, style.inline_attachment);
    let decoration = DecorationSpan { range: TextRange::new(2, 4), kind: DecorationKind::Emphasis };
    assert_eq!(TextRange::new(2, 4), decoration.range);
    assert_eq!(DecorationKind::Emphasis, decoration.kind);
    assert_eq!(ColorSpan { start: 1, end: 5, argb: 0xFF112233_u32 as i32 }, ColorSpan { start: 1, end: 5, argb: 0xFF112233_u32 as i32 });
}

#[test]
fn test_rich_text_spans_and_patterns() {
    let background = RichTextBackgroundPaint::builder().horizontal_padding(2.0).vertical_padding(3.0).corner_radius(4.0).continuation_corner_radius(1.0).metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).draw_style(RichTextBackgroundDrawStyle::border(1.5)).build();
    let paint = RichTextPaint::builder().argb(0xFF000000_u32 as i32).line_pattern(RichTextLinePattern::Solid).background(background.clone()).adjacent_same_style_clearance(1.5).build();
    assert_eq!(Some(0xFF000000_u32 as i32), paint.argb);
    assert_eq!(1.5, paint.adjacent_same_style_clearance);
    assert_eq!(2.0, background.horizontal_padding);
    assert!(std::panic::catch_unwind(|| RichTextPaint::builder().adjacent_same_style_clearance(-0.1).build()).is_err());
    assert!(std::panic::catch_unwind(|| RichTextBackgroundPaint::builder().horizontal_padding(-1.0).build()).is_err());
    assert!(std::panic::catch_unwind(|| RichTextBackgroundDrawStyle::border(0.0)).is_err());
    assert!(std::panic::catch_unwind(|| RichTextLinePattern::dashed(0.0, 4.0, 2.0)).is_err());
    assert!(std::panic::catch_unwind(|| RichTextLinePattern::dotted(2.0, 0.0)).is_err());
    let link = RichTextRole::Link { target: "https://tiqian.org".to_owned() };
    for role in [RichTextRole::Background, RichTextRole::Underline, RichTextRole::LineThrough, link, RichTextRole::TechnicalInline, RichTextRole::InlineCode] {
        assert_eq!(role, RichTextSpan::with_paint(TextRange::new(0, 2), role.clone(), paint.clone()).role);
    }
}

#[test]
fn test_ruby_and_paragraph_models() {
    let pinyin = RubySpan::builder(TextRange::new(0, 1), Text::from("hàn")).font_families(vec!["CustomFont".to_owned()]).kind(RubyKind::Pinyin).build();
    assert_eq!(RubyKind::Pinyin, pinyin.kind);
    assert_eq!(None, pinyin.locale);
    let bopomofo = RubySpan::with_kind(TextRange::new(0, 1), Text::from("ㄏㄢˋ"), RubyKind::Bopomofo);
    assert_eq!(Some("zh-TW".to_owned()), bopomofo.locale);
    assert_eq!(0.1, DEFAULT_EMPHASIS_DOT_GAP_EM);
    assert_eq!(0.1, DEFAULT_INLINE_OBJECT_MINIMUM_CLEARANCE_EM);
    let indent = MeasureAdaptiveFirstLineIndent::new(14.0, 1.0, 2.0);
    assert_eq!(1.0, indent.resolve_em(10.0));
    assert_eq!(2.0, indent.resolve_em(14.0));
    let style = ParagraphStyle::builder().last_line_alignment(LastLineAlignment::End).writing_mode(WritingMode::VerticalRl).line_height(Some(32.0)).first_line_indent_policy(indent).line_length_grid(LineLengthGrid::new(true, Some(LastLineAlignment::Center))).ruby_line_height_mode(RubyLineHeightMode::UniformParagraph).inline_object_minimum_clearance_em(0.2).emphasis_dot_gap_em(0.15).build();
    assert_eq!(LastLineAlignment::End, style.last_line_alignment);
    assert_eq!(WritingMode::VerticalRl, style.writing_mode);
    assert_eq!(Some(32.0), style.line_height);
    let profile = LayoutProfileId { value: "custom-profile".to_owned() };
    assert_eq!("custom-profile", profile.value);
    assert_eq!("clreq-horizontal", built_in_layout_profiles::clreq_horizontal().value);
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("Test")), LayoutConstraints::with_defaults(300.0)).paragraph_style(style).profile_id(profile).decorations(vec![DecorationSpan { range: TextRange::new(0, 2), kind: DecorationKind::Emphasis }]).ruby_spans(vec![pinyin]).inline_boxes(vec![InlineBoxSpan::new(TextRange::new(0, 1))]).inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(0, 1), 10.0, 8.0, 2.0)]).build();
    assert_eq!("custom-profile", input.profile_id.value);
    assert_eq!(1, input.inline_objects.len());
}