use tiqian::common::HashSet;

use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{
    DecorationKind, DecorationSpan, LastLineAlignment, LayoutInput, ParagraphStyle,
    RichTextBackgroundPaint, RichTextLinePattern, RichTextPaint, RichTextRole, RichTextSpan,
    RubyKind, RubyLineHeightMode, RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use vello::peniko::color::AlphaColor;

use crate::renderer::DemoColorSpan;

#[derive(Clone)]
pub struct DemoDocument {
    pub input: LayoutInput,
    pub colors: Vec<DemoColorSpan>,
    pub rich_text: Vec<RichTextSpan>,
}

/// Private block model for the desktop demo sample.
pub struct DemoDocumentDemo {
    pub blocks: Vec<DemoDocumentDemoBlock>,
}

pub enum DemoDocumentDemoBlock {
    Paragraph(DemoDocument),
    NarrowParagraph {
        document: DemoDocument,
        max_width: f32,
    },
    ListItem {
        marker: DemoDocument,
        body: DemoDocument,
    },
    Section {
        height: f32,
    },
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
    let content = TiqianTextContent::builder(Text::from(text))
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
        RubySpan::new(ruby, Text::from("zhōng wén")),
        RubySpan::builder(bopomofo, Text::from("ㄓㄨˋ ㄧㄣ"))
            .kind(RubyKind::Bopomofo)
            .build(),
    ])
    .build();
    let colors = vec![DemoColorSpan {
        range: latin,
        color: AlphaColor::from_rgba8(37, 99, 235, 255),
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

/// Builds the formal feature-inspection sample displayed by the desktop demo.
pub fn build_document_demo(physical_content_width: f32, physical_scale: f32) -> DemoDocumentDemo {
    let body = TextStyle::builder()
        .font_families(vec!["Source Han Sans SC".to_owned()])
        .font_size(15.0 * physical_scale)
        .build();
    let flush = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .build();
    let indented = ParagraphStyle::builder()
        .line_height(Some(22.5 * physical_scale))
        .build();
    let block_quote = ParagraphStyle::builder()
        .block_indent(Ic { count: 2.0 })
        .first_line_indent(Some(Ic::ZERO))
        .build();
    let uniform_ruby = ParagraphStyle::builder()
        .ruby_line_height_mode(RubyLineHeightMode::UniformParagraph)
        .build();
    let title_paragraph = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .last_line_alignment(LastLineAlignment::Center)
        .build();
    let signature = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .last_line_alignment(LastLineAlignment::End)
        .build();
    let section_height = 22.5 * physical_scale;

    let proof = "「第三次校样」据编辑批注修订，日期为二〇二六年八月二十六日。";
    let title = "提椠中文正文排版样张";
    let overview = "汉字排版讲究的不只是字形端正，也包括行列疏密、标点位置与段落节奏。本页选取书刊校样中常见的文字形式，集中呈现简体中文横排、中西文混排、行间注文和传统标注。窗口宽度改变时，文字会依照新的版心重新成行；标题、列表与注文也随正文一同调整。";
    let punctuation = "编辑在批注中写道：“排版并非把文字摆下去，而是让每一行都获得清楚、安稳而从容的秩序。”括号（包括圆括号、方括号和书名号）应与正文相接，逗号、句号、问号和感叹号都在恰当的位置。遇到“真的如此吗？！”一类连续标点时，字面仍须紧凑，不宜留下突兀的空白。";
    let narrow_proof = "校样排印，宜留呼吸。";
    let mixed_text = "中文书刊经常夹用 Latin letters、OpenType 字体名称、Unicode 字符编号和 HTTP/2 协议名称。汉字与西文字母或数字相邻时，应留有细微而稳定的间隔；行首与行尾则不额外添空。较长的英文词如 internationalization 和 interoperability，可以在合适的音节处使用连字符转行，但不应任意拆开。";
    let narrow_hyphenation = "术语 internationalization 可按音节转行。";
    let list_intro = "校阅正文时，可依次观察以下项目：";
    let pinyin = "地名、术语或生僻字可以附加拼音。例如，“提椠”二字读作 tíqiàn；注文居于基字上方，既帮助读者辨音，也不打乱正文原有的行列。相邻注文较长时，字间距离可以适度调整，使注音清楚而不显拥挤。";
    let bopomofo = "为照顾使用注音符号的读者，本页另以“您好”为例：您字右侧标注 ㄋㄧㄣˊ，好字右侧标注 ㄏㄠˇ。声母、韵母与调号依字身排列，注文与正文之间保持清楚而稳定的对应关系。";
    let decorations = "讨论现代中文排版时，北京大学的研究者常会参阅《中文排版需求》以及相关字体排印著作。书名可用波浪线标示，专有名称则用直线区别。已故语言学家朱德熙先生对现代汉语研究贡献深远；在特定出版物中，其姓名可以示亡号标明。需要读者格外留意的词句，还可以加着重号。";
    let quote =
        "编校札记：版面宽阔时，正文宜从容舒展；\n栏宽收窄时，段首缩进与行间距离也应保持协调。";
    let rich_text = "校样状态分为：已核、待校、旁注与撤销。已核内容可以绿色标示；待校内容使用蓝色；旁注使用红色。新增词句加实线下划线，存疑内容加虚线下划线，补充说明加点线下划线，已经撤销的文字则保留删除线，以便追溯修改过程。";
    let file_note = "本次校样依据 editorial-notes.md 整理，参考版本为 Review 3。文件名采用等宽字体，版本名称加浅色背景；两者夹在中文正文中时，前后仍应保留舒适的阅读间隔。";
    let bullet_intro = "本页适合在以下情形中检查：";
    let closing = "好的中文排版不会抢在文字之前引人注意，却能让阅读更加连贯、安静而从容。字形、标点、注文和段落彼此协调，长篇正文才能在不同版面中保持稳定的节奏。";
    let signature_text = "——《提椠中文正文排版样张》";
    let boundary_appendix_title = "附录：窄栏断词与行尾标点";
    let appendix_title = "附录：Emoji 组合字形";
    let emoji_appendix = "本附录列出可用于核对的组合字形：👩🏽‍💻、👨‍👩‍👧‍👦、🇨🇳、1️⃣ 与 ✈️。每一项都应作为完整字形参与排版，在换行、选择与绘制时保持一致。";
    let ligature_appendix_title = "附录：连字字形";
    let other_languages_appendix_title = "附录：其他语言示例文本";
    let japanese = "このマークアップ構文は JSX と呼ばれます。React が普及させた JavaScript の構文拡張です。JSX マークアップは関連するレンダリングロジックのすぐそばに配置できるので、React コンポーネントは簡単に作成、保守、削除ができます。";
    let korean = "이 마크업 구문을 JSX라 부릅니다. 이것은 React에 의해서 대중화된 자바스크립트 구문의 확장입니다. JSX 마크업을 관련된 렌더링 로직과 가까이 두면, React 컴포넌트를 쉽게 만들고 관리하고 삭제할 수 있습니다.";
    let english = "Cras maximus rutrum magna in gravida. Suspendisse et varius lectus. Ut ac metus id est vehicula euismod ac a sapien. Curabitur pulvinar ornare neque. Proin mattis magna vel massa eleifend cursus. Donec elementum sollicitudin venenatis. Aenean imperdiet consectetur diam, nec mollis leo. ";
    let spanish = "Esta sintaxis de marcado se llama JSX. Es una extensión de la sintaxis de JavaScript popularizada por React. Al poner marcado JSX cerca de la lógica de renderizado relacionada hace que los componentes de React sean fáciles de crear, mantener y eliminar.";
    let russian = "Этот синтаксис разметки называется JSX. Это расширение синтаксиса JavaScript, которое стало популярным благодаря React. Размещение разметки JSX рядом с соответствующей логикой рендеринга упрощает создание, сопровождение и удаление компонентов React.";
    let ligature_symbols = "FiraCode: -> <= := != === //";
    let ligature_words = "EB Garamond: office affinity waffle";
    let fira_code = TextStyle::builder()
        .font_families(vec!["FiraCode Nerd Font".to_owned()])
        .font_size(15.0 * physical_scale)
        .build();
    let eb_garamond = TextStyle::builder()
        .font_families(vec!["EB Garamond".to_owned()])
        .font_size(17.0 * physical_scale)
        .build();

    let title_style = TextStyle::builder()
        .font_families(body.font_families.clone())
        .font_size(28.5 * physical_scale)
        .font_weight(700)
        .build();
    let emphasis = |text: &str, needle: &str| DecorationSpan {
        range: range_of(text, needle),
        kind: DecorationKind::Emphasis,
    };
    let mut blocks = vec![
        DemoDocumentDemoBlock::Paragraph(demo_document(
            proof,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![RichTextSpan::with_paint(
                range_of(proof, "「第三次校样」"),
                RichTextRole::Underline,
                RichTextPaint::default(),
            )],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            title,
            physical_content_width,
            body.clone(),
            title_paragraph,
            vec![TextSpan {
                range: range_of(title, title),
                style: title_style,
            }],
            vec![],
            vec![RubySpan::new(range_of(title, "提椠"), Text::from("tíqiàn"))],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            overview,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![
                emphasis(overview, "行列疏密"),
                emphasis(overview, "段落节奏"),
            ],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            punctuation,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            mixed_text,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![
                TextSpan {
                    range: range_of(mixed_text, "OpenType"),
                    style: TextStyle::builder()
                        .font_families(vec!["Inter".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of(mixed_text, "Unicode"),
                    style: TextStyle::builder()
                        .font_families(vec!["Inter".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of(mixed_text, "HTTP/2"),
                    style: TextStyle::builder()
                        .font_families(vec!["Inter".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
            ],
            vec![],
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
    ];
    blocks.extend([
        demo_list_item(
            "一、",
            "每个非末行在版心内保持齐整，末行则依段落用途自然收束。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "二、",
            "标点临近行首或行尾时，系统优先调整可用空隙，避免出现突兀的断行。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "三、",
            "中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。",
            physical_content_width,
            body.clone(),
            vec![],
            vec![
                TextSpan {
                    range: range_of("中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。", "黑体"),
                    style: TextStyle::builder()
                        .font_families(vec!["Source Han Sans SC".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of("中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。", "宋体"),
                    style: TextStyle::builder()
                        .font_families(vec!["serif".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of("中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。", "sans-serif"),
                    style: TextStyle::builder()
                        .font_families(vec!["Inter".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_occurrence("中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。", "serif", 1),
                    style: TextStyle::builder()
                        .font_families(vec!["serif".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
                TextSpan {
                    range: range_of("中文可使用黑体或宋体；英文可能为 sans-serif 或 serif，亦可能为 monospace （等宽字体）。混排时，仍须保持稳定的基线和行距。", "monospace"),
                    style: TextStyle::builder()
                        .font_families(vec!["monospace".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                },
            ],
            vec![],
            vec![],
        ),
    ]);
    blocks.extend([
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            pinyin,
            physical_content_width,
            body.clone(),
            uniform_ruby,
            vec![],
            vec![emphasis(pinyin, "注文")],
            vec![RubySpan::new(
                range_of(pinyin, "提椠"),
                Text::from("tíqiàn"),
            )],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            bopomofo,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![emphasis(bopomofo, "清楚而稳定")],
            vec![
                RubySpan::builder(range_of(bopomofo, "您"), Text::from("ㄋㄧㄣˊ"))
                    .kind(RubyKind::Bopomofo)
                    .build(),
                RubySpan::builder(range_of(bopomofo, "好"), Text::from("ㄏㄠˇ"))
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
            decorations,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![
                DecorationSpan {
                    range: range_of(decorations, "北京大学"),
                    kind: DecorationKind::ProperNoun,
                },
                DecorationSpan {
                    range: range_of(decorations, "《中文排版需求》"),
                    kind: DecorationKind::BookTitle,
                },
                DecorationSpan {
                    range: range_of(decorations, "朱德熙"),
                    kind: DecorationKind::Mourning,
                },
                emphasis(decorations, "格外留意"),
            ],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            quote,
            physical_content_width,
            body.clone(),
            block_quote,
            vec![TextSpan {
                range: range_of(quote, "从容舒展"),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(body.font_size)
                    .italic(true)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            rich_text,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![],
            vec![],
            vec![],
            vec![
                DemoColorSpan {
                    range: range_of(rich_text, "已核"),
                    color: AlphaColor::from_rgba8(26, 110, 60, 255),
                },
                DemoColorSpan {
                    range: range_of(rich_text, "待校"),
                    color: AlphaColor::from_rgba8(37, 99, 235, 255),
                },
                DemoColorSpan {
                    range: range_of(rich_text, "旁注"),
                    color: AlphaColor::from_rgba8(176, 0, 32, 255),
                },
            ],
            vec![
                RichTextSpan::with_paint(
                    range_of(rich_text, "新增词句"),
                    RichTextRole::Underline,
                    RichTextPaint::default(),
                ),
                RichTextSpan::with_paint(
                    range_of(rich_text, "存疑内容"),
                    RichTextRole::Underline,
                    RichTextPaint::builder()
                        .line_pattern(RichTextLinePattern::dashed(
                            physical_scale,
                            3.0 * physical_scale,
                            2.0 * physical_scale,
                        ))
                        .build(),
                ),
                RichTextSpan::with_paint(
                    range_of(rich_text, "补充说明"),
                    RichTextRole::Underline,
                    RichTextPaint::builder()
                        .line_pattern(RichTextLinePattern::dotted(
                            1.5 * physical_scale,
                            1.5 * physical_scale,
                        ))
                        .build(),
                ),
                RichTextSpan::with_paint(
                    range_of(rich_text, "已经撤销的文字"),
                    RichTextRole::LineThrough,
                    RichTextPaint::default(),
                ),
                RichTextSpan::with_paint(
                    range_of(rich_text, "校样状态"),
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
            ],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            file_note,
            physical_content_width,
            body.clone(),
            indented.clone(),
            vec![TextSpan {
                range: range_of(file_note, "editorial-notes.md"),
                style: TextStyle::builder()
                    .font_families(vec!["monospace".to_owned()])
                    .font_size(body.font_size)
                    .build(),
            }],
            vec![],
            vec![],
            vec![DemoColorSpan {
                range: range_of(file_note, "Review 3"),
                color: AlphaColor::from_rgba8(126, 34, 206, 255),
            }],
            vec![
                RichTextSpan::with_paint(
                    range_of(file_note, "Review 3"),
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
                    range_of(file_note, "editorial-notes.md"),
                    RichTextRole::InlineCode,
                    RichTextPaint::builder()
                        .argb(0xFFE5E7EB_u32 as i32)
                        .background(
                            RichTextBackgroundPaint::builder()
                                .horizontal_padding(2.0 * physical_scale)
                                .vertical_padding(1.0 * physical_scale)
                                .corner_radius(2.0 * physical_scale)
                                .build(),
                        )
                        .build(),
                ),
            ],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            bullet_intro,
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
            "调整窗口宽度，比较宽栏与窄栏中的断行、缩进和标点位置；",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "•",
            "改变系统缩放比例，检查正文、注文、线条与留白是否同步变化；",
            physical_content_width,
            body.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
        ),
        demo_list_item(
            "•",
            "对照标题、列表、引文和校样标记，确认不同层级仍保持清楚的视觉秩序。",
            physical_content_width,
            body.clone(),
            vec![],
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
            indented.clone(),
            vec![
                TextSpan {
                    range: range_of(closing, "连贯、安静而从容"),
                    style: TextStyle::builder()
                        .font_families(body.font_families.clone())
                        .font_size(body.font_size)
                        .italic(true)
                        .build(),
                },
                TextSpan {
                    range: range_of(closing, "连贯、安静而从容"),
                    style: TextStyle::builder()
                        .font_families(body.font_families.clone())
                        .font_size(19.5 * physical_scale)
                        .font_weight(700)
                        .build(),
                },
            ],
            vec![],
            vec![],
            vec![DemoColorSpan {
                range: range_of(closing, "连贯、安静而从容"),
                color: AlphaColor::from_rgba8(26, 110, 60, 255),
            }],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            signature_text,
            physical_content_width,
            body.clone(),
            signature,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            boundary_appendix_title,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: range_of(boundary_appendix_title, boundary_appendix_title),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(19.5 * physical_scale)
                    .font_weight(700)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::NarrowParagraph {
            document: demo_document(
                narrow_proof,
                4.0 * body.font_size,
                body.clone(),
                flush.clone(),
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            max_width: 4.0 * body.font_size,
        },
        DemoDocumentDemoBlock::NarrowParagraph {
            document: demo_document(
                narrow_hyphenation,
                8.0 * body.font_size,
                body.clone(),
                flush.clone(),
                vec![TextSpan {
                    range: range_of(narrow_hyphenation, "internationalization"),
                    style: TextStyle::builder()
                        .font_families(vec!["Inter".to_owned()])
                        .font_size(body.font_size)
                        .build(),
                }],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            max_width: 8.0 * body.font_size,
        },
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            appendix_title,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: range_of(appendix_title, appendix_title),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(19.5 * physical_scale)
                    .font_weight(700)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            emoji_appendix,
            physical_content_width,
            body.clone(),
            indented,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            ligature_appendix_title,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: range_of(ligature_appendix_title, ligature_appendix_title),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(19.5 * physical_scale)
                    .font_weight(700)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            ligature_words,
            physical_content_width,
            eb_garamond,
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Paragraph(demo_document(
            ligature_symbols,
            physical_content_width,
            fira_code,
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            other_languages_appendix_title,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![TextSpan {
                range: range_of(
                    other_languages_appendix_title,
                    other_languages_appendix_title,
                ),
                style: TextStyle::builder()
                    .font_families(body.font_families.clone())
                    .font_size(19.5 * physical_scale)
                    .font_weight(700)
                    .build(),
            }],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            japanese,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            korean,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            english,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            spanish,
            physical_content_width,
            body.clone(),
            flush.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        )),
        DemoDocumentDemoBlock::Section {
            height: section_height,
        },
        DemoDocumentDemoBlock::Paragraph(demo_document(
            russian,
            physical_content_width,
            body,
            flush,
            vec![],
            vec![],
            vec![],
            vec![],
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
    rich_text: Vec<RichTextSpan>,
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
            rich_text,
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
    let content = TiqianTextContent::builder(Text::from(text))
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
        .unwrap_or_else(|| {
            panic!("paragraph-demo sample is missing occurrence {occurrence} of {needle}")
        });
    let start = text[..start].encode_utf16().count() as i32;
    TextRange::new(start, start + needle.encode_utf16().count() as i32)
}
