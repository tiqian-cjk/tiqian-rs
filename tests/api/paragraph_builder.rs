use tiqian::api::*;
use tiqian::core::geometry::{LayoutConstraints, TextRange, scalar_offset};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, InlineBoxOuterSpacing, InlineObjectBoundaryAdjustment, LineBreakPolicy,
    LineBreakSpan, ParagraphStyle, RichTextBackgroundPaint, RichTextLayer, RichTextLayerKind,
    RichTextLinePaint, RichTextPaint, RichTextSemantic, TextStyle, LayoutProfileId,
    built_in_layout_profiles,
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
    assert_eq!("正文强调结尾", output.content.text);
    assert_eq!(1, output.content.spans.len());
    assert_eq!(
        TextRange::new(scalar_offset(2), scalar_offset(4)),
        output.content.spans[0].range
    );
    assert_eq!(700, output.content.spans[0].style.font_weight);
    assert!(output.content.spans[0].style.italic);
    assert_eq!(15.0, output.content.spans[0].style.font_size);
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
fn paragraph_configuration_updates_after_source_text_are_ignored() {
    let text_style = TextStyle::builder().font_size(18.0).build();
    let paragraph_style = ParagraphStyle::builder().line_height(Some(30.0)).build();
    let profile_id = LayoutProfileId {
        value: "kept-profile".to_owned(),
    };
    let paint = RichTextPaint::Fill {
        argb: 0xFF2563EB_u32 as i32,
    };
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.text_style(text_style.clone());
    builder.paragraph_style(paragraph_style.clone());
    builder.profile_id(profile_id.clone());
    builder.paints(&[paint.clone()]);
    builder.push("正文");
    builder.text_style(TextStyle::builder().font_size(24.0).build());
    builder.paragraph_style(ParagraphStyle::default());
    builder.profile_id(built_in_layout_profiles::clreq_horizontal());
    builder.paints(&[RichTextPaint::Fill {
        argb: 0xFFDC2626_u32 as i32,
    }]);
    builder.push("后续");

    let output = builder.build().unwrap();
    assert_eq!(text_style, output.text_style);
    assert_eq!(paragraph_style, output.paragraph_style);
    assert_eq!(profile_id, output.profile_id);
    assert!(output.rich_text.iter().all(|span| span.layers == vec![RichTextLayer {
        kind: RichTextLayerKind::Text,
        paints: vec![paint.clone()],
    }]));
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

    assert_eq!("codetiqian.org", output.content.text);
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
        output.content.line_break_spans
    );
    assert_eq!(
        vec![code_range],
        output.content.auto_space_suppressed_ranges
    );
    assert!(
        output.content.source_boundaries.contains(&code_range.start())
    );
    assert!(
        output.content.source_boundaries.contains(&code_range.end())
    );
    assert!(
        output.content.source_boundaries.contains(&link_range.start())
    );
    assert!(
        output.content.source_boundaries.contains(&link_range.end())
    );
    assert_eq!(1, output.content.spans.len());
    assert_eq!(code_range, output.content.spans[0].range);
    assert_eq!(
        vec!["monospace".to_owned()],
        output.content.spans[0].style.font_families
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
        output.content.line_break_spans
    );
    assert_eq!(vec![range], output.content.auto_space_suppressed_ranges);
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
            .inline_boxes
            .iter()
            .map(|span| span.range)
            .collect::<Vec<_>>()
    );
    assert!(output.inline_boxes.iter().all(|span| {
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
    assert!(output.inline_boxes.is_empty());
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
    assert_eq!(range, output.inline_boxes[0].range);
    assert_eq!(2.0, output.inline_boxes[0].inline_start);
    assert_eq!(3.0, output.inline_boxes[0].inline_end);
    assert_eq!(
        InlineBoxOuterSpacing::Source,
        output.inline_boxes[0].outer_spacing
    );
    assert_eq!(range, output.ruby_spans[0].base_range);
    assert_eq!(Text::from("tíqiàn"), output.ruby_spans[0].text);
    assert_eq!(DecorationKind::Emphasis, output.decorations[0].kind);
    assert_eq!(range, output.decorations[0].range);
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
    assert_eq!("甲\nobject", output.content.text);
    assert_eq!(
        TextRange::new(scalar_offset(2), scalar_offset(8)),
        output.inline_objects[0].range
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

#[test]
fn convenience_scopes_preserve_their_own_ranges_and_restore_outer_state() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.styled(
        TextStyleOverride::builder().font_weight(700).build(),
        "粗",
    );
    builder.ruby(RubyAnnotation::pinyin("xì"), "细").unwrap();
    builder.mourning("亡");
    builder.proper_noun("专");
    builder.book_title("书");
    builder.color(0xFF2563EB_u32 as i32, "色");
    builder.background(
        RichTextBackgroundPaint::builder().horizontal_padding(2.0).build(),
        &[RichTextPaint::Fill {
            argb: 0xFFFF0000_u32 as i32,
        }],
        "底",
    );
    builder.underline(RichTextLinePaint::default(), "线");
    builder.line_through(RichTextLinePaint::default(), "删");
    builder.link("https://tiqian.org".to_owned(), "提椠");
    builder.technical("code");
    builder.auto_space_suppressed("紧");

    let output = builder.build().unwrap();
    assert_eq!("粗细亡专书色底线删提椠code紧", output.content.text);
    assert_eq!(vec![700], output.content.spans.iter().map(|span| span.style.font_weight).collect::<Vec<_>>());
    assert_eq!(TextRange::new(scalar_offset(1), scalar_offset(2)), output.ruby_spans[0].base_range);
    assert_eq!(
        vec![DecorationKind::Mourning, DecorationKind::ProperNoun, DecorationKind::BookTitle],
        output.decorations.iter().map(|span| span.kind).collect::<Vec<_>>(),
    );
    assert!(output.rich_text.iter().any(|span| span.layers.iter().any(|layer| matches!(layer.kind, RichTextLayerKind::Background { .. }))));
    assert!(output.rich_text.iter().any(|span| span.layers.iter().any(|layer| matches!(layer.kind, RichTextLayerKind::Underline { .. }))));
    assert!(output.rich_text.iter().any(|span| span.layers.iter().any(|layer| matches!(layer.kind, RichTextLayerKind::LineThrough { .. }))));
    assert!(output.rich_text.iter().any(|span| span.semantics == vec![RichTextSemantic::Link { target: "https://tiqian.org".to_owned() }]));
    assert_eq!(
        vec![
            LineBreakSpan {
                range: TextRange::new(scalar_offset(11), scalar_offset(15)),
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
        ],
        output.content.line_break_spans,
    );
    assert_eq!(
        vec![
            TextRange::new(scalar_offset(11), scalar_offset(15)),
            TextRange::new(scalar_offset(15), scalar_offset(16)),
        ],
        output.content.auto_space_suppressed_ranges,
    );
    assert_eq!(1, output.inline_boxes.len());
    assert_eq!(TextRange::new(scalar_offset(6), scalar_offset(7)), output.inline_boxes[0].range);
}

#[test]
fn links_only_use_technical_breaking_when_text_displays_the_target() {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.link("https://tiqian.org".to_owned(), "https://tiqian.org");
    builder.link("https://tiqian.org".to_owned(), "提椠");

    let output = builder.build().unwrap();
    assert_eq!(1, output.content.line_break_spans.len());
    assert_eq!(TextRange::new(scalar_offset(0), scalar_offset(18)), output.content.line_break_spans[0].range);
    assert_eq!(2, output.rich_text.len());
    assert!(output.rich_text.iter().all(|span| matches!(span.semantics.as_slice(), [RichTextSemantic::Link { .. }])));
}

#[test]
fn styles_and_errors_cover_explicit_overrides_and_position_insertion_rules() {
    let base = TextStyle::builder()
        .font_families(vec!["Base".to_owned()])
        .font_size(12.0)
        .locale("zh-Hans".to_owned())
        .font_weight(400)
        .italic(false)
        .baseline_shift(0.0)
        .build();
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.text_style(base);
    builder.styled(
        TextStyleOverride::builder()
            .font_families(Vec::new())
            .font_size(16.0)
            .locale("ja-JP".to_owned())
            .font_weight(500)
            .italic(true)
            .baseline_shift(2.0)
            .build(),
        "字",
    );
    let output = builder.build().unwrap();
    assert_eq!(Vec::<String>::new(), output.content.spans[0].style.font_families);
    assert_eq!(16.0, output.content.spans[0].style.font_size);
    assert_eq!("ja-JP", output.content.spans[0].style.locale);
    assert_eq!(500, output.content.spans[0].style.font_weight);
    assert!(output.content.spans[0].style.italic);
    assert_eq!(2.0, output.content.spans[0].style.baseline_shift);

    let mut ruby_builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    ruby_builder.push_ruby(RubyAnnotation::pinyin("jiǎ")).unwrap();
    let error = ruby_builder
        .inline_object("图", InlineObjectMetrics::new(16.0, 12.0, 4.0))
        .unwrap_err();
    assert_eq!(
        ParagraphBuildError::ForbiddenPositionInsertion {
            scope: ParagraphScopeKind::Ruby,
            insertion: ParagraphPositionInsertionKind::InlineObject,
        },
        error
    );
    assert_eq!("cannot insert InlineObject inside Ruby", error.to_string());
    assert_eq!(
        "inline object requires non-empty replacement text",
        ParagraphBuildError::EmptyInlineObjectReplacementText.to_string()
    );
}

#[test]
fn manual_and_fallible_scopes_lower_each_supported_presentation_contract() {
    let paint = RichTextPaint::Fill { argb: 0xFF334155_u32 as i32 };
    let line = RichTextLinePaint::default();
    let background = RichTextBackgroundPaint::builder().horizontal_padding(3.0).build();
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.paints(&[paint.clone()]);

    builder.push_decoration(DecorationKind::Emphasis).unwrap();
    builder.push("饰");
    builder.pop().unwrap();
    builder.push_color(0xFF2563EB_u32 as i32).unwrap();
    builder.push("色");
    builder.pop().unwrap();
    builder.push_paints(&[paint.clone()]).unwrap();
    builder.push("画");
    builder.pop().unwrap();
    builder.push_rich_text(&[underline_layer()]).unwrap();
    builder.push("层");
    builder.pop().unwrap();
    builder.push_background(background.clone(), &[paint.clone()]).unwrap();
    builder.push("背");
    builder.pop().unwrap();
    builder.push_underline(line.clone()).unwrap();
    builder.push("下");
    builder.pop().unwrap();
    builder.push_line_through(line.clone()).unwrap();
    builder.push("删");
    builder.pop().unwrap();
    builder.push_link("https://tiqian.org".to_owned()).unwrap();
    builder.push("提椠");
    builder.pop().unwrap();
    builder.push_technical().unwrap();
    builder.push("tech");
    builder.pop().unwrap();
    builder.push_inline_code(TextStyleOverride::default(), background).unwrap();
    builder.push("code");
    builder.pop().unwrap();
    builder.push_auto_space_suppressed().unwrap();
    builder.push("紧");
    builder.pop().unwrap();

    builder.try_with_ruby(RubyAnnotation::pinyin("x"), |builder| { builder.push("注"); Ok(()) }).unwrap();
    builder.try_with_decoration(DecorationKind::Mourning, |builder| { builder.push("悼"); Ok(()) }).unwrap();
    builder.try_with_inline_box(InlineBoxStyle::with_edges(1.0, 2.0), |builder| { builder.push("盒"); Ok(()) }).unwrap();
    builder.try_with_color(0xFF16A34A_u32 as i32, |builder| { builder.push("绿"); Ok(()) }).unwrap();
    builder.try_with_paints(&[paint.clone()], |builder| { builder.push("笔"); Ok(()) }).unwrap();
    builder.try_with_rich_text(&[underline_layer()], |builder| { builder.push("富"); Ok(()) }).unwrap();
    builder.try_with_background(RichTextBackgroundPaint::default(), &[paint.clone()], |builder| { builder.push("景"); Ok(()) }).unwrap();
    builder.try_with_underline(line.clone(), |builder| { builder.push("线"); Ok(()) }).unwrap();
    builder.try_with_line_through(line, |builder| { builder.push("杠"); Ok(()) }).unwrap();
    builder.try_with_link("https://tiqian.org".to_owned(), |builder| { builder.push("链接"); Ok(()) }).unwrap();
    builder.try_with_technical(|builder| { builder.push("T"); Ok(()) }).unwrap();
    builder.try_with_inline_code(TextStyleOverride::default(), RichTextBackgroundPaint::default(), |builder| { builder.push("C"); Ok(()) }).unwrap();
    builder.try_with_auto_space_suppressed(|builder| { builder.push("抑"); Ok(()) }).unwrap();

    let output = builder.build().unwrap();
    assert_eq!("饰色画层背下删提椠techcode紧注悼盒绿笔富景线杠链接TC抑", output.content.text);
    assert_eq!(1, output.ruby_spans.len());
    assert!(output.decorations.iter().any(|span| span.kind == DecorationKind::Emphasis));
    assert!(output.decorations.iter().any(|span| span.kind == DecorationKind::Mourning));
    assert!(output.rich_text.iter().any(|span| span.semantics.iter().any(|semantic| matches!(semantic, RichTextSemantic::Link { .. }))));
    assert!(output.content.line_break_spans.len() >= 4);
    assert!(output.content.auto_space_suppressed_ranges.len() >= 5);
    assert!(output.inline_boxes.iter().any(|span| span.inline_start == 3.0));
}

#[test]
fn style_builders_and_build_errors_preserve_explicit_public_contracts() {
    let attachment = tiqian::core::text_model::InlineAttachment::Previous;
    let leading = InlineObjectBoundaryAdjustment::builder()
        .participates_in_uniform_stretch(true)
        .shrink_capacity(2.0)
        .line_end_discardable_advance(1.0)
        .prevents_line_break(true)
        .build();
    let trailing = InlineObjectBoundaryAdjustment::builder().shrink_capacity(3.0).build();
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(320.0));
    builder.styled(
        TextStyleOverride::builder().inline_attachment(attachment).build(),
        "附",
    );
    builder.ruby(
        RubyAnnotation::builder("ㄈㄨˋ")
            .font_families(vec!["Bopomofo".to_owned()])
            .kind(tiqian::core::text_model::RubyKind::Bopomofo)
            .locale(Some("zh-Hant".to_owned()))
            .build(),
        "注",
    ).unwrap();
    builder.inline_box(
        InlineBoxStyle::builder().inline_start(1.0).inline_end(2.0).outer_spacing(InlineBoxOuterSpacing::Source).build(),
        "盒",
    ).unwrap();
    builder.inline_object(
        "物",
        InlineObjectMetrics::builder(24.0, 16.0, 8.0)
            .leading_boundary(leading.clone())
            .trailing_boundary(trailing.clone())
            .build(),
    ).unwrap();
    let output = builder.build().unwrap();
    assert_eq!(attachment, output.content.spans[0].style.inline_attachment);
    assert_eq!(Text::from("ㄈㄨˋ"), output.ruby_spans[0].text);
    assert_eq!(vec!["Bopomofo"], output.ruby_spans[0].font_families);
    assert_eq!(Some("zh-Hant".to_owned()), output.ruby_spans[0].locale);
    assert_eq!(InlineBoxOuterSpacing::Source, output.inline_boxes[0].outer_spacing);
    assert_eq!(leading, output.inline_objects[0].leading_boundary);
    assert_eq!(trailing, output.inline_objects[0].trailing_boundary);

    assert_eq!("cannot close an empty paragraph scope stack", ParagraphBuildError::EmptyScopeStack.to_string());
    assert_eq!("paragraph builder contains unclosed scopes", ParagraphBuildError::UnclosedScopes { scopes: vec![ParagraphScopeKind::Link] }.to_string());
    assert_eq!("paragraph builder scope stack crossed a closure boundary", ParagraphBuildError::ClosureScopeBoundary { scope: ParagraphScopeKind::Color }.to_string());
    assert_eq!("InlineBox scope requires non-empty text", ParagraphBuildError::EmptyScope { scope: ParagraphScopeKind::InlineBox }.to_string());
}
