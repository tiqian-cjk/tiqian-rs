use std::collections::HashSet;

use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::TextModel::{
    DecorationKind, DecorationSpan, LayoutInput, RichTextBackgroundPaint,
    ParagraphStyle, RichTextLinePattern, RichTextPaint, RichTextRole, RichTextSpan, RubyKind,
    RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;

use crate::renderer::DemoColorSpan;

#[derive(Clone)]
pub struct DemoDocument {
    pub input: LayoutInput,
    pub colors: Vec<DemoColorSpan>,
    pub rich_text: Vec<RichTextSpan>,
}

/// Private document model matching the block order rendered by Kotlin's TiqianDemoScreen.
pub struct DemoDocumentDemo {
    pub blocks: Vec<DemoDocumentDemoBlock>,
}

pub enum DemoDocumentDemoBlock {
    TextField(DemoDocument),
    Paragraph(DemoDocument),
    ListItem { marker: DemoDocument, body: DemoDocument },
    Section { height: f32 },
}

#[allow(dead_code)]
pub fn build_document(physical_content_width: f32, physical_scale: f32) -> DemoDocument {
    let text = "中文，。……——English\n中文「括号」，句号。中文；中文, punctuation. English；中文—…“text”中文";
    let title = range_occurrence(text, "中文", 0);
    let latin = range_occurrence(text, "English", 0);
    let ruby = range_occurrence(text, "中文", 1);
    let bopomofo = range_occurrence(text, "中文", 2);
    let emphasis = range_of(text, "括号");
    let mourning = range_of(text, "句号");
    let proper_noun = range_of(text, "punctuation");
    let book_title = range_of(text, "text");
    let content = TiqianTextContent::builder(text.to_owned())
        .source_boundaries(HashSet::from([
            title.start(),
            title.end(),
            latin.start(),
            latin.end(),
            ruby.start(),
            ruby.end(),
            bopomofo.start(),
            bopomofo.end(),
            emphasis.start(),
            emphasis.end(),
            mourning.start(),
            mourning.end(),
            proper_noun.start(),
            proper_noun.end(),
            book_title.start(),
            book_title.end(),
        ]))
        .spans(vec![TextSpan {
            range: latin,
            style: TextStyle::builder()
                .font_families(vec!["Inter".to_owned()])
                .font_size(16.0 * physical_scale)
                .build(),
        }])
        .build();
    let input = LayoutInput::builder(
        content,
        LayoutConstraints::with_defaults(physical_content_width.max(1.0)),
    )
    .text_style(
        TextStyle::builder()
            .font_families(vec!["Source Han Sans SC".to_owned()])
            .font_size(16.0 * physical_scale)
            .build(),
    )
    .decorations(vec![
        DecorationSpan {
            range: emphasis,
            kind: DecorationKind::Emphasis,
        },
        DecorationSpan {
            range: mourning,
            kind: DecorationKind::Mourning,
        },
        DecorationSpan {
            range: proper_noun,
            kind: DecorationKind::ProperNoun,
        },
        DecorationSpan {
            range: book_title,
            kind: DecorationKind::BookTitle,
        },
    ])
    .ruby_spans(vec![
        RubySpan::new(ruby, "zhōng wén".to_owned()),
        RubySpan::builder(bopomofo, "ㄓㄨˋ ㄧㄣ".to_owned())
            .kind(RubyKind::Bopomofo)
            .build(),
    ])
    .build();
    let colors = vec![DemoColorSpan {
        range: latin,
        color: tiny_skia::Color::from_rgba8(37, 99, 235, 255),
    }];
    let rich_text = vec![
        RichTextSpan::with_paint(
            title,
            RichTextRole::Background,
            RichTextPaint::builder()
                .argb(0xFFFDE68A_u32 as i32)
                .background(
                    RichTextBackgroundPaint::builder()
                        .horizontal_padding(2.0 * physical_scale)
                        .vertical_padding(1.0 * physical_scale)
                        .corner_radius(3.0 * physical_scale)
                        .build(),
                )
                .build(),
        ),
        RichTextSpan::with_paint(
            latin,
            RichTextRole::Underline,
            RichTextPaint::builder()
                .argb(0xFF2563EB_u32 as i32)
                .line_pattern(RichTextLinePattern::dashed(
                    physical_scale,
                    3.0 * physical_scale,
                    2.0 * physical_scale,
                ))
                .build(),
        ),
        RichTextSpan::with_paint(
            proper_noun,
            RichTextRole::LineThrough,
            RichTextPaint::builder()
                .argb(0xFFDC2626_u32 as i32)
                .line_pattern(RichTextLinePattern::dotted(
                    1.5 * physical_scale,
                    1.5 * physical_scale,
                ))
                .build(),
        ),
    ];
    DemoDocument {
        input,
        colors,
        rich_text,
    }
}

/// Builds the complete block sequence and source text displayed by Kotlin's runComposeDemo.
pub fn build_document_demo(physical_content_width: f32, physical_scale: f32) -> DemoDocumentDemo {
    let body = TextStyle::builder()
        .font_families(vec!["Source Han Sans SC".to_owned()])
        .font_size(15.0 * physical_scale)
        .build();
    let flush = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .build();
    let block = ParagraphStyle::builder()
        .block_indent(Ic { count: 2.0 })
        .first_line_indent(Some(Ic::ZERO))
        .build();
    let section_height = 22.5 * physical_scale;
    let draft = "在这里打字，看我实时重排；也可以拖选、双击并复制。";
    let underline = "“开标点与句末标点。” 下划线只画字身，不吃首尾标点 glue。";
    let opening = "诸位好。我叫提椠，一台对中文正文斤斤计较的排版引擎。别家把 espresso 和汉字一锅乱炖，我偏要在中西之间留出四分之一个字的体面距离——你瞧，连这句里的 OpenType，我都没让它贴脸。";
    let rules = "我的家规不多，列在下面：";
    let mourning = "上周我还痛失一员旧部：双面印刷。它本为纸张正反透印而生，奈何屏幕没有背面，只好请它先走一步。纸终究比屏幕厚道，这话我只敢斜着说。";
    let bopomofo = "台湾来的朋友也照顾周到——您好，请坐：ㄅㄆㄇ 竖在字旁，平上去入标得分毫不差。";
    let reference = "我奉CLREQ——也就是《中文排版需求》——为圭臬，闲来也翻翻Unicode的家底。";
    let list_intro = "顺带一提，这些我也顺手包办：";
    let closing = "有人嫌我龟毛，我只当是褒奖。毕竟，好看的中文，是一个字一个字抠出来的。";

    let title_style = TextStyle::builder()
        .font_families(body.font_families.clone())
        .font_size(28.5 * physical_scale)
        .font_weight(700)
        .build();
    let mut blocks = vec![
        DemoDocumentDemoBlock::TextField(demo_document(
            draft,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            draft,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            underline,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![RichTextSpan::with_paint(
                range_of(underline, "“开标点与句末标点。”"),
                RichTextRole::Underline,
                RichTextPaint::default(),
            )],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            "一台排版引擎的自述",
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: TextRange::new(0, 9),
                style: title_style,
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            opening,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![],
            vec![
                DecorationSpan {
                    range: range_of(opening, "斤斤计较"),
                    kind: DecorationKind::Emphasis,
                },
                DecorationSpan {
                    range: range_of(opening, "四分之一个字"),
                    kind: DecorationKind::Emphasis,
                },
            ],
            vec![RubySpan::new(range_of(opening, "提椠"), "tíqiàn".to_owned())],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            rules,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: range_of(rules, "家规"),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(body.font_size)
                    .font_weight(700)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
    ];
    blocks.extend([
        demo_list_item(
            "一、",
            "标点不许在行首撒野：逗号句号一律避头尾，该挤就挤，该悬就悬。",
            physical_content_width,
            body.clone(),
            vec![DecorationSpan {
                range: TextRange::new(16, 19),
                kind: DecorationKind::Emphasis,
            }],
            vec![],
            vec![],
        ),
        demo_list_item(
            "二、",
            "字体随你挑——宋体的雅、等宽的拙，按角色各取所需。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![
                TextSpan {
                    range: range_of("字体随你挑——宋体的雅、等宽的拙，按角色各取所需。", "宋体的雅"),
                    style: TextStyle::builder()
                        .font_families(vec!["serif".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of("字体随你挑——宋体的雅、等宽的拙，按角色各取所需。", "等宽的拙"),
                    style: TextStyle::builder()
                        .font_families(vec!["monospace".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
            ],
            vec![],
        ),
        demo_list_item(
            "三、",
            "注音拼音都伺候，连生僻字也给你标得明明白白。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![RubySpan::new(
                range_of("注音拼音都伺候，连生僻字也给你标得明明白白。", "生僻字"),
                "shēngpì zì".to_owned(),
            )],
        ),
    ]);
    blocks.extend([
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            mourning,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![TextSpan {
                range: range_of(mourning, "纸终究比屏幕厚道，这话我只敢斜着说。"),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(body.font_size)
                    .italic(true)
                    .build(),
            }],
            vec![
                DecorationSpan {
                    range: range_of(mourning, "双面印刷"),
                    kind: DecorationKind::Mourning,
                },
                DecorationSpan {
                    range: range_of(mourning, "先走一步"),
                    kind: DecorationKind::Emphasis,
                },
            ],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            bopomofo,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![],
            vec![DecorationSpan {
                range: range_of(bopomofo, "分毫不差"),
                kind: DecorationKind::Emphasis,
            }],
            vec![
                RubySpan::builder(range_occurrence(bopomofo, "您", 0), "ㄋㄧㄣˊ".to_owned())
                    .kind(RubyKind::Bopomofo)
                    .build(),
                RubySpan::builder(range_occurrence(bopomofo, "好", 0), "ㄏㄠˇ".to_owned())
                    .kind(RubyKind::Bopomofo)
                    .build(),
                RubySpan::builder(range_occurrence(bopomofo, "请", 0), "ㄑㄧㄥˇ".to_owned())
                    .kind(RubyKind::Bopomofo)
                    .build(),
            ],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            reference,
            physical_content_width,
            body.clone(),
            block,
            vec![],
            vec![
                DecorationSpan {
                    range: range_of(reference, "CLREQ"),
                    kind: DecorationKind::ProperNoun,
                },
                DecorationSpan {
                    range: range_of(reference, "《中文排版需求》"),
                    kind: DecorationKind::BookTitle,
                },
                DecorationSpan {
                    range: range_of(reference, "Unicode"),
                    kind: DecorationKind::ProperNoun,
                },
            ],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            list_intro,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
    ]);
    blocks.extend([
        demo_list_item(
            "•",
            "整数字格行长，正文严丝合缝落在格子上；",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "•",
            "行尾标点悬挂、中西自动间距，统统全自动；",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "•",
            "挤一挤放得下的，绝不硬把一整行拉稀。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
        ),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            closing,
            physical_content_width,
            body.clone(),
            ParagraphStyle::default(),
            vec![
                TextSpan {
                    range: range_of(closing, "龟毛"),
                    style: TextStyle::builder()
                        .font_families(body.font_families.clone())
                        .font_size(body.font_size)
                        .italic(true)
                        .build(),
                },
                TextSpan {
                    range: range_of(closing, "一个字一个字"),
                    style: TextStyle::builder()
                        .font_families(body.font_families.clone())
                        .font_size(19.5 * physical_scale)
                        .font_weight(700)
                        .build(),
                },
            ],
            vec![],
            vec![],
            vec![
                DemoColorSpan {
                    range: range_of(closing, "褒奖"),
                    color: tiny_skia::Color::from_rgba8(176, 0, 32, 255),
                },
                DemoColorSpan {
                    range: range_of(closing, "一个字一个字"),
                    color: tiny_skia::Color::from_rgba8(26, 110, 60, 255),
                },
            ],
            vec![],
        )),
    ]);
    DemoDocumentDemo { blocks }
}

fn demo_list_item(
    marker: &str,
    body: &str,
    physical_content_width: f32,
    text_style: TextStyle,
    decorations: Vec<DecorationSpan>,
    spans: Vec<TextSpan>,
    ruby_spans: Vec<RubySpan>,
) -> DemoDocumentDemoBlock {
    DemoDocumentDemoBlock::ListItem {
        marker: demo_document(
            marker,
            physical_content_width,
            text_style.clone(),
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        body: demo_document(
            body,
            physical_content_width,
            text_style,
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
            spans,
            decorations,
            ruby_spans,
            vec![],
            vec![],
        ),
    }
}

fn demo_document(
    text: &str,
    physical_content_width: f32,
    text_style: TextStyle,
    paragraph_style: ParagraphStyle,
    spans: Vec<TextSpan>,
    decorations: Vec<DecorationSpan>,
    ruby_spans: Vec<RubySpan>,
    colors: Vec<DemoColorSpan>,
    rich_text: Vec<RichTextSpan>,
) -> DemoDocument {
    let mut source_boundaries = HashSet::new();
    for range in spans
        .iter()
        .map(|span| span.range)
        .chain(decorations.iter().map(|span| span.range))
        .chain(ruby_spans.iter().map(|span| span.base_range))
        .chain(colors.iter().map(|span| span.range))
        .chain(rich_text.iter().map(|span| span.range))
    {
        source_boundaries.insert(range.start());
        source_boundaries.insert(range.end());
    }
    let content = TiqianTextContent::builder(text.to_owned())
        .source_boundaries(source_boundaries)
        .spans(spans)
        .build();
    DemoDocument {
        input: LayoutInput::builder(
            content,
            LayoutConstraints::with_defaults(physical_content_width.max(1.0)),
        )
        .text_style(text_style)
        .paragraph_style(paragraph_style)
        .decorations(decorations)
        .ruby_spans(ruby_spans)
        .build(),
        colors,
        rich_text,
    }
}

fn range_of(text: &str, needle: &str) -> TextRange {
    range_occurrence(text, needle, 0)
}

fn range_occurrence(text: &str, needle: &str, occurrence: usize) -> TextRange {
    let start = text
        .match_indices(needle)
        .nth(occurrence)
        .map(|(start, _)| start)
        .unwrap_or_else(|| panic!("paragraph-demo sample is missing occurrence {occurrence} of {needle}"));
    let start = text[..start].encode_utf16().count() as i32;
    TextRange::new(start, start + needle.encode_utf16().count() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_document_demo_preserves_run_compose_demo_text_per_block() {
        let document = build_document_demo(640.0, 1.0);
        let actual: Vec<_> = document
            .blocks
            .iter()
            .map(|block| match block {
                DemoDocumentDemoBlock::TextField(document) => {
                    format!("TextField:{}", document.input.content.text)
                }
                DemoDocumentDemoBlock::Paragraph(document) => {
                    format!("Paragraph:{}", document.input.content.text)
                }
                DemoDocumentDemoBlock::ListItem { marker, body } => format!(
                    "List:{}|{}",
                    marker.input.content.text, body.input.content.text
                ),
                DemoDocumentDemoBlock::Section { height } => format!("Section:{height}"),
            })
            .collect();
        let expected = vec![
            "TextField:在这里打字，看我实时重排；也可以拖选、双击并复制。",
            "Paragraph:在这里打字，看我实时重排；也可以拖选、双击并复制。",
            "Paragraph:“开标点与句末标点。” 下划线只画字身，不吃首尾标点 glue。",
            "Paragraph:一台排版引擎的自述",
            "Paragraph:诸位好。我叫提椠，一台对中文正文斤斤计较的排版引擎。别家把 espresso 和汉字一锅乱炖，我偏要在中西之间留出四分之一个字的体面距离——你瞧，连这句里的 OpenType，我都没让它贴脸。",
            "Section:22.5",
            "Paragraph:我的家规不多，列在下面：",
            "List:一、|标点不许在行首撒野：逗号句号一律避头尾，该挤就挤，该悬就悬。",
            "List:二、|字体随你挑——宋体的雅、等宽的拙，按角色各取所需。",
            "List:三、|注音拼音都伺候，连生僻字也给你标得明明白白。",
            "Section:22.5",
            "Paragraph:上周我还痛失一员旧部：双面印刷。它本为纸张正反透印而生，奈何屏幕没有背面，只好请它先走一步。纸终究比屏幕厚道，这话我只敢斜着说。",
            "Paragraph:台湾来的朋友也照顾周到——您好，请坐：ㄅㄆㄇ 竖在字旁，平上去入标得分毫不差。",
            "Section:22.5",
            "Paragraph:我奉CLREQ——也就是《中文排版需求》——为圭臬，闲来也翻翻Unicode的家底。",
            "Paragraph:顺带一提，这些我也顺手包办：",
            "List:•|整数字格行长，正文严丝合缝落在格子上；",
            "List:•|行尾标点悬挂、中西自动间距，统统全自动；",
            "List:•|挤一挤放得下的，绝不硬把一整行拉稀。",
            "Section:22.5",
            "Paragraph:有人嫌我龟毛，我只当是褒奖。毕竟，好看的中文，是一个字一个字抠出来的。",
        ];
        assert_eq!(actual, expected);
    }
}
