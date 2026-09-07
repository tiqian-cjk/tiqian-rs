use tiqian::api::*;
use tiqian::core::geometry::{LayoutConstraints, TextRange, scalar_offset};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, InlineBoxOuterSpacing, InlineObjectBoundaryAdjustment, LineBreakPolicy,
    LineBreakSpan, RichTextBackgroundPaint, RichTextLayer, RichTextLayerKind,
    RichTextLinePaint, RichTextPaint, RichTextSemantic, TextStyle, built_in_layout_profiles,
};

fn underline_layer() -> RichTextLayer {
    RichTextLayer {
        kind: RichTextLayerKind::Underline {
            line: RichTextLinePaint::default(),
        },
        paints: Vec::new(),
    }
}

fn background_layer(background: RichTextBackgroundPaint) -> RichTextLayer {
    RichTextLayer {
        kind: RichTextLayerKind::Background { background },
        paints: Vec::new(),
    }
}

#[test]
fn inline_configuration_values_preserve_core_defaults() {
    assert_eq!(InlineBoxStyle::new(), InlineBoxStyle::default());
    assert_eq!(
        InlineObjectBoundaryAdjustment::FIXED,
        InlineObjectMetrics::new(24.0, 16.0, 8.0).leading_boundary
    );
}

#[test]
fn explicit_text_style_scope_generates_a_complete_style_span() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.text_style(
        TextStyle::builder()
            .font_families(vec!["Source Han Sans SC".to_owned()])
            .font_size(15.0)
            .build(),
    );
    builder.push("正文");
    builder
        .push_text_style(
            TextStyleOverride::builder()
                .font_weight(700)
                .italic(true)
                .build(),
        )
        .unwrap();
    builder.push("强调");
    builder.pop().unwrap();
    builder.push("结尾");

    let output = builder.build().unwrap();
    assert_eq!("正文强调结尾", output.input.content.text);
    assert_eq!(1, output.input.content.spans.len());
    assert_eq!(
        TextRange::new(scalar_offset(2), scalar_offset(4)),
        output.input.content.spans[0].range
    );
    assert_eq!(700, output.input.content.spans[0].style.font_weight);
    assert!(output.input.content.spans[0].style.italic);
    assert_eq!(15.0, output.input.content.spans[0].style.font_size);
}

#[test]
fn closure_scope_detects_an_unclosed_child_scope() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_text_style(TextStyleOverride::default(), |builder| {
        builder
            .push_text_style(TextStyleOverride::default())
            .unwrap();
    });

    assert_eq!(
        Err(ParagraphBuildError::ClosureScopeBoundary {
            scope: ParagraphScopeKind::TextStyle,
        }),
        builder.build()
    );
}

#[test]
fn try_with_error_prevents_a_later_build() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    let error = builder
        .try_with_text_style(TextStyleOverride::default(), |builder| {
            builder.pop()?;
            builder.pop()
        })
        .unwrap_err();

    assert_eq!(ParagraphBuildError::EmptyScopeStack, error);
    assert_eq!(Err(error), builder.build());
}

#[test]
fn paragraph_configuration_panics_after_source_text_is_appended() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.push("正文");

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            builder.profile_id(built_in_layout_profiles::clreq_horizontal());
        }))
        .is_err()
    );
}

#[test]
fn scopes_lower_to_existing_layout_and_presentation_fields_in_opening_order() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_color(0xFF2563EB_u32 as i32, |builder| {
        builder.with_rich_text(
            &[underline_layer()],
            |builder| {
                builder.with_technical(|builder| {
                    builder.with_inline_code(
                        TextStyleOverride::builder()
                            .font_families(vec!["monospace".to_owned()])
                            .build(),
                        RichTextBackgroundPaint::default(),
                        |builder| builder.push("code"),
                    );
                });
            },
        );
    });
    builder.with_link("https://tiqian.org".to_owned(), |builder| {
        builder.push("tiqian.org");
    });

    let output = builder.build().unwrap();
    let code_range = TextRange::new(scalar_offset(0), scalar_offset(4));
    let link_range = TextRange::new(scalar_offset(4), scalar_offset(14));

    assert_eq!("codetiqian.org", output.input.content.text);
    assert_eq!(code_range, output.rich_text[0].range);
    assert_eq!(
        vec![
            RichTextLayerKind::Text,
            RichTextLayerKind::Underline {
                line: RichTextLinePaint::default(),
            },
            RichTextLayerKind::Background {
                background: RichTextBackgroundPaint::default(),
            },
        ],
        output.rich_text[0]
            .layers
            .iter()
            .map(|layer| layer.kind.clone())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        vec![RichTextPaint::Fill {
            argb: 0xFF2563EB_u32 as i32,
        }],
        output.rich_text[0].layers[0].paints,
    );
    assert_eq!(
        vec![
            RichTextSemantic::TechnicalInline,
            RichTextSemantic::TechnicalInline,
        ],
        output.rich_text[0].semantics,
    );
    assert_eq!(
        vec![RichTextSemantic::Link {
                target: "https://tiqian.org".to_owned(),
            }],
        output.rich_text[1].semantics,
    );
    assert_eq!(
        vec![
            LineBreakSpan {
                range: code_range,
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
            LineBreakSpan {
                range: link_range,
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
        ],
        output.input.content.line_break_spans
    );
    assert_eq!(
        vec![code_range],
        output.input.content.auto_space_suppressed_ranges
    );
    assert!(
        output
            .input
            .content
            .source_boundaries
            .contains(&code_range.start())
    );
    assert!(
        output
            .input
            .content
            .source_boundaries
            .contains(&code_range.end())
    );
    assert!(
        output
            .input
            .content
            .source_boundaries
            .contains(&link_range.start())
    );
    assert!(
        output
            .input
            .content
            .source_boundaries
            .contains(&link_range.end())
    );
    assert_eq!(1, output.input.content.spans.len());
    assert_eq!(code_range, output.input.content.spans[0].range);
    assert_eq!(
        vec!["monospace".to_owned()],
        output.input.content.spans[0].style.font_families
    );
}

#[test]
fn inline_code_generates_line_break_and_auto_space_inputs() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_inline_code(
        TextStyleOverride::default(),
        RichTextBackgroundPaint::default(),
        |builder| builder.push("code"),
    );

    let output = builder.build().unwrap();
    let range = TextRange::new(scalar_offset(0), scalar_offset(4));

    assert_eq!(
        vec![LineBreakSpan {
            range,
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        output.input.content.line_break_spans
    );
    assert_eq!(vec![range], output.input.content.auto_space_suppressed_ranges);
}

#[test]
fn padded_background_and_inline_code_generate_narrow_inline_boxes() {
    let background = RichTextBackgroundPaint::builder().horizontal_padding(4.0).build();
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_rich_text(&[background_layer(background.clone())], |builder| {
        builder.push("背景");
    });
    builder.with_inline_code(TextStyleOverride::default(), background, |builder| {
        builder.push("code");
    });

    let output = builder.build().unwrap();
    assert_eq!(
        vec![
            TextRange::new(scalar_offset(0), scalar_offset(2)),
            TextRange::new(scalar_offset(2), scalar_offset(6)),
        ],
        output
            .input
            .inline_boxes
            .iter()
            .map(|span| span.range)
            .collect::<Vec<_>>()
    );
    assert!(output.input.inline_boxes.iter().all(|span| {
        span.inline_start == 4.0
            && span.inline_end == 4.0
            && span.outer_spacing == InlineBoxOuterSpacing::Narrow
    }));
}

#[test]
fn zero_padding_and_non_background_rich_text_do_not_generate_inline_boxes() {
    let padded_underline = RichTextLayer {
        kind: RichTextLayerKind::Underline {
            line: RichTextLinePaint::default(),
        },
        paints: vec![RichTextPaint::Fill { argb: 0xFF000000_u32 as i32 }],
    };
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_rich_text(&[background_layer(RichTextBackgroundPaint::default())], |builder| {
        builder.push("零");
    });
    builder.with_rich_text(&[padded_underline], |builder| {
        builder.push("线");
    });

    let output = builder.build().unwrap();
    assert!(output.input.inline_boxes.is_empty());
}

#[test]
fn decoration_ruby_and_inline_box_lower_to_their_core_spans() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.with_inline_box(
        InlineBoxStyle::with_all(2.0, 3.0, InlineBoxOuterSpacing::Source),
        |builder| {
            builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
                builder.emphasis("提椠");
            });
        },
    );

    let output = builder.build().unwrap();
    let range = TextRange::new(scalar_offset(0), scalar_offset(2));
    assert_eq!(range, output.input.inline_boxes[0].range);
    assert_eq!(2.0, output.input.inline_boxes[0].inline_start);
    assert_eq!(3.0, output.input.inline_boxes[0].inline_end);
    assert_eq!(
        InlineBoxOuterSpacing::Source,
        output.input.inline_boxes[0].outer_spacing
    );
    assert_eq!(range, output.input.ruby_spans[0].base_range);
    assert_eq!(Text::from("tíqiàn"), output.input.ruby_spans[0].text);
    assert_eq!(DecorationKind::Emphasis, output.input.decorations[0].kind);
    assert_eq!(range, output.input.decorations[0].range);
}

#[test]
fn position_insertions_preserve_source_and_reject_invalid_ruby_content() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.push("甲");
    builder.hard_break().unwrap();
    builder
        .inline_object("object", InlineObjectMetrics::new(24.0, 16.0, 8.0))
        .unwrap();

    let output = builder.build().unwrap();
    assert_eq!("甲\nobject", output.input.content.text);
    assert_eq!(
        TextRange::new(scalar_offset(2), scalar_offset(8)),
        output.input.inline_objects[0].range
    );

    let mut ruby_builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    ruby_builder
        .push_ruby(RubyAnnotation::pinyin("jiǎ"))
        .unwrap();
    assert_eq!(
        Err(ParagraphBuildError::ForbiddenPositionInsertion {
            scope: ParagraphScopeKind::Ruby,
            insertion: ParagraphPositionInsertionKind::HardBreak,
        }),
        ruby_builder.hard_break()
    );
    assert_eq!(
        Err(ParagraphBuildError::ForbiddenPositionInsertion {
            scope: ParagraphScopeKind::Ruby,
            insertion: ParagraphPositionInsertionKind::HardBreak,
        }),
        ruby_builder.build()
    );
}

#[test]
fn empty_ruby_inline_box_and_inline_object_are_build_errors() {
    let mut ruby_builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    assert_eq!(
        ParagraphBuildError::EmptyScope {
            scope: ParagraphScopeKind::Ruby,
        },
        match ruby_builder.ruby(RubyAnnotation::pinyin("tíqiàn"), "") {
            Ok(_) => panic!("empty ruby must return an error"),
            Err(error) => error,
        }
    );
    assert_eq!(
        Err(ParagraphBuildError::EmptyScope {
            scope: ParagraphScopeKind::Ruby,
        }),
        ruby_builder.build()
    );

    let mut box_builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    assert_eq!(
        ParagraphBuildError::EmptyScope {
            scope: ParagraphScopeKind::InlineBox,
        },
        match box_builder.inline_box(InlineBoxStyle::new(), "") {
            Ok(_) => panic!("empty inline box must return an error"),
            Err(error) => error,
        }
    );
    assert_eq!(
        Err(ParagraphBuildError::EmptyScope {
            scope: ParagraphScopeKind::InlineBox,
        }),
        box_builder.build()
    );

    let mut object_builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    assert_eq!(
        Err(ParagraphBuildError::EmptyInlineObjectReplacementText),
        object_builder.inline_object("", InlineObjectMetrics::new(24.0, 16.0, 8.0))
    );
    assert_eq!(
        Err(ParagraphBuildError::EmptyInlineObjectReplacementText),
        object_builder.build()
    );
}
