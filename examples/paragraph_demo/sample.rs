use tiqian::api::{
    ParagraphBuilder, RubyAnnotation, TextStyleOverride,
};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text_model::{
    LastLineAlignment, ParagraphStyle, RichTextBackgroundPaint, RichTextLayer,
    RichTextLayerKind, RichTextLinePaint, RichTextLinePattern, RichTextPaint,
    RubyLineHeightMode, TextStyle,
};
use tiqian::core::units::Ic;

#[derive(Clone)]
pub struct DemoDocument {
    pub input: tiqian::core::text_model::LayoutInput,
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
    let body = TextStyle::builder()
        .font_families(vec!["Source Han Sans SC".to_owned()])
        .font_size(16.0 * physical_scale)
        .build();
    paragraph(
        physical_content_width,
        body,
        ParagraphStyle::default(),
        |builder| {
            builder.with_background(
                background_paint(physical_scale),
                &[RichTextPaint::Fill { argb: 0xFFFDE68A_u32 as i32 }],
                |builder| builder.push("中文"),
            );
            builder.push("，。……——");
            builder.with_text_style(inter_style(16.0 * physical_scale), |builder| {
                builder.with_color(0xFF2563EB_u32 as i32, |builder| {
                    builder.with_underline(
                        dashed_paint(physical_scale),
                        |builder| builder.push("English"),
                    );
                });
            });
            builder.push("\n");
            builder.with_ruby(RubyAnnotation::pinyin("zhōng wén"), |builder| {
                builder.push("中文");
            });
            builder.push("「");
            builder.emphasis("括号");
            builder.push("」，");
            builder.mourning("句号");
            builder.push("。");
            builder.with_ruby(RubyAnnotation::bopomofo("ㄓㄨˋ ㄧㄣ"), |builder| {
                builder.push("中文");
            });
            builder.push("；中文, ");
            builder.proper_noun("punctuation");
            builder.push(". English；中文—…“");
            builder.book_title("text");
            builder.push("”中文");
        },
    )
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
    let heading = TextStyleOverride::builder()
        .font_size(19.5 * physical_scale)
        .font_weight(700)
        .build();
    let title_style = TextStyleOverride::builder()
        .font_size(28.5 * physical_scale)
        .font_weight(700)
        .build();

    let proof = paragraph(physical_content_width, body.clone(), indented.clone(), |builder| {
        builder.with_underline(RichTextLinePaint::default(), |builder| {
            builder.push("「第三次校样」");
        });
        builder.push("据编辑批注修订，日期为二〇二六年八月二十六日。");
    });
    let title = paragraph(physical_content_width, body.clone(), title_paragraph, |builder| {
        builder.with_text_style(title_style, |builder| {
            builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
                builder.push("提椠");
            });
            builder.push("中文正文排版样张");
        });
    });
    let overview = paragraph(physical_content_width, body.clone(), indented.clone(), |builder| {
        builder.push("汉字排版讲究的不只是字形端正，也包括");
        builder.emphasis("行列疏密");
        builder.push("、标点位置与");
        builder.emphasis("段落节奏");
        builder.push("。本页选取书刊校样中常见的文字形式，集中呈现简体中文横排、中西文混排、行间注文和传统标注。窗口宽度改变时，文字会依照新的版心重新成行；标题、列表与注文也随正文一同调整。");
    });
    let punctuation = paragraph(physical_content_width, body.clone(), indented.clone(), |builder| {
        builder.push("编辑在批注中写道：“排版并非把文字摆下去，而是让每一行都获得清楚、安稳而从容的秩序。”括号（包括圆括号、方括号和书名号）应与正文相接，逗号、句号、问号和感叹号都在恰当的位置。遇到“真的如此吗？！”一类连续标点时，字面仍须紧凑，不宜留下突兀的空白。");
    });
    let mixed = paragraph(physical_content_width, body.clone(), indented.clone(), |builder| {
        builder.push("中文书刊经常夹用 Latin letters、");
        builder.styled(inter_style(body.font_size), "OpenType");
        builder.push(" 字体名称、");
        builder.styled(inter_style(body.font_size), "Unicode");
        builder.push(" 字符编号和 ");
        builder.styled(inter_style(body.font_size), "HTTP/2");
        builder.push(" 协议名称。汉字与西文字母或数字相邻时，应留有细微而稳定的间隔；行首与行尾则不额外添空。较长的英文词如 internationalization 和 interoperability，可以在合适的音节处使用连字符转行，但不应任意拆开。");
    });
    let list_intro = plain_paragraph(
        physical_content_width,
        body.clone(),
        flush.clone(),
        "校阅正文时，可依次观察以下项目：",
    );

    let mut blocks = vec![
        DemoDocumentDemoBlock::Paragraph(proof),
        DemoDocumentDemoBlock::Paragraph(title),
        DemoDocumentDemoBlock::Paragraph(overview),
        DemoDocumentDemoBlock::Paragraph(punctuation),
        DemoDocumentDemoBlock::Paragraph(mixed),
        DemoDocumentDemoBlock::Paragraph(list_intro),
        list_item(
            "一、",
            physical_content_width,
            body.clone(),
            |builder| builder.push("每个非末行在版心内保持齐整，末行则依段落用途自然收束。"),
        ),
        list_item(
            "二、",
            physical_content_width,
            body.clone(),
            |builder| builder.push("标点临近行首或行尾时，系统优先调整可用空隙，避免出现突兀的断行。"),
        ),
        list_item(
            "三、",
            physical_content_width,
            body.clone(),
            |builder| {
                builder.push("中文可使用");
                builder.styled(
                    TextStyleOverride::builder()
                        .font_families(vec!["Source Han Sans SC".to_owned()])
                        .build(),
                    "黑体",
                );
                builder.push("或");
                builder.styled(
                    TextStyleOverride::builder()
                        .font_families(vec!["serif".to_owned()])
                        .build(),
                    "宋体",
                );
                builder.push("；英文可能为 ");
                builder.styled(inter_style(body.font_size), "sans-serif");
                builder.push(" 或 ");
                builder.styled(
                    TextStyleOverride::builder()
                        .font_families(vec!["serif".to_owned()])
                        .build(),
                    "serif",
                );
                builder.push("，亦可能为 ");
                builder.styled(
                    TextStyleOverride::builder()
                        .font_families(vec!["monospace".to_owned()])
                        .build(),
                    "monospace",
                );
                builder.push(" （等宽字体）。混排时，仍须保持稳定的基线和行距。");
            },
        ),
        DemoDocumentDemoBlock::Section { height: section_height },
    ];

    blocks.extend([
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            uniform_ruby,
            |builder| {
                builder.push("地名、术语或生僻字可以附加拼音。例如，“");
                builder.with_ruby(RubyAnnotation::pinyin("tíqiàn"), |builder| {
                    builder.push("提椠");
                });
                builder.push("”二字读作 tíqiàn；");
                builder.emphasis("注文");
                builder.push("居于基字上方，既帮助读者辨音，也不打乱正文原有的行列。相邻注文较长时，字间距离可以适度调整，使注音清楚而不显拥挤。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            |builder| {
                builder.push("为照顾使用注音符号的读者，本页另以“");
                builder.with_ruby(RubyAnnotation::bopomofo("ㄋㄧㄣˊ"), |builder| builder.push("您"));
                builder.with_ruby(RubyAnnotation::bopomofo("ㄏㄠˇ"), |builder| builder.push("好"));
                builder.push("”为例：您字右侧标注 ㄋㄧㄣˊ，好字右侧标注 ㄏㄠˇ。声母、韵母与调号依字身排列，注文与正文之间保持");
                builder.emphasis("清楚而稳定");
                builder.push("的对应关系。");
            },
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            |builder| {
                builder.push("讨论现代中文排版时，");
                builder.proper_noun("北京大学");
                builder.push("的研究者常会参阅");
                builder.book_title("《中文排版需求》");
                builder.push("以及相关字体排印著作。书名可用波浪线标示，专有名称则用直线区别。已故语言学家");
                builder.mourning("朱德熙");
                builder.push("先生对现代汉语研究贡献深远；在特定出版物中，其姓名可以示亡号标明。需要读者");
                builder.emphasis("格外留意");
                builder.push("的词句，还可以加着重号。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            block_quote,
            |builder| {
                builder.push("编校札记：版面宽阔时，正文宜");
                builder.styled(TextStyleOverride::builder().italic(true).build(), "从容舒展");
                builder.push("；\n栏宽收窄时，段首缩进与行间距离也应保持协调。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            |builder| {
                builder.with_background(
                    background_paint(physical_scale),
                    &[RichTextPaint::Fill { argb: 0xFFFDE68A_u32 as i32 }],
                    |builder| builder.push("校样状态"),
                );
                builder.push("分为：");
                builder.color(0xFF1A6E3C_u32 as i32, "已核");
                builder.push("、");
                builder.color(0xFF2563EB_u32 as i32, "待校");
                builder.push("、");
                builder.color(0xFFB00020_u32 as i32, "旁注");
                builder.push("与撤销。已核内容可以绿色标示；待校内容使用蓝色；旁注使用红色。");
                builder.underline(RichTextLinePaint::default(), "新增词句");
                builder.push("加实线下划线，");
                builder.underline(dashed_paint(physical_scale), "存疑内容");
                builder.push("加虚线下划线，");
                builder.underline(dotted_paint(physical_scale), "补充说明");
                builder.push("加点线下划线，");
                builder.line_through(RichTextLinePaint::default(), "已经撤销的文字");
                builder.push("则保留删除线，以便追溯修改过程。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            |builder| {
                builder.push("本次校样依据 ");
                builder.with_paints(&[RichTextPaint::Fill { argb: 0xFFE5E7EB_u32 as i32 }], |builder| {
                    builder.with_rich_text(
                        &[RichTextLayer {
                            kind: RichTextLayerKind::Text,
                            paints: vec![RichTextPaint::default()],
                        }],
                        |builder| {
                            builder.inline_code(
                                TextStyleOverride::builder()
                                    .font_families(vec!["monospace".to_owned()])
                                    .build(),
                                code_paint(physical_scale),
                                "editorial-notes.md",
                            );
                        },
                    );
                });
                builder.push(" 整理，参考版本为 ");
                builder.with_color(0xFF7E22CE_u32 as i32, |builder| {
                    builder.with_background(
                        background_paint(physical_scale),
                        &[RichTextPaint::Fill { argb: 0xFFFDE68A_u32 as i32 }],
                        |builder| builder.push("Review 3"),
                    );
                });
                builder.push("。文件名采用等宽字体，版本名称加浅色背景；两者夹在中文正文中时，前后仍应保留舒适的阅读间隔。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            "本页适合在以下情形中检查：",
        )),
        list_item(
            "•",
            physical_content_width,
            body.clone(),
            |builder| builder.push("调整窗口宽度，比较宽栏与窄栏中的断行、缩进和标点位置；"),
        ),
        list_item(
            "•",
            physical_content_width,
            body.clone(),
            |builder| builder.push("改变系统缩放比例，检查正文、注文、线条与留白是否同步变化；"),
        ),
        list_item(
            "•",
            physical_content_width,
            body.clone(),
            |builder| builder.push("对照标题、列表、引文和校样标记，确认不同层级仍保持清楚的视觉秩序。"),
        ),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            |builder| {
                builder.push("好的中文排版不会抢在文字之前引人注意，却能让阅读更加");
                builder.with_color(0xFF1A6E3C_u32 as i32, |builder| {
                    builder.with_text_style(TextStyleOverride::builder().italic(true).build(), |builder| {
                        builder.with_text_style(
                            TextStyleOverride::builder()
                                .font_size(19.5 * physical_scale)
                                .font_weight(700)
                                .build(),
                            |builder| builder.push("连贯、安静而从容"),
                        );
                    });
                });
                builder.push("。字形、标点、注文和段落彼此协调，长篇正文才能在不同版面中保持稳定的节奏。");
            },
        )),
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            signature,
            "——《提椠中文正文排版样张》",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(styled_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            heading.clone(),
            "附录：窄栏断词与行尾标点",
        )),
        DemoDocumentDemoBlock::NarrowParagraph {
            document: plain_paragraph(
                4.0 * body.font_size,
                body.clone(),
                flush.clone(),
                "校样排印，宜留呼吸。",
            ),
            max_width: 4.0 * body.font_size,
        },
        DemoDocumentDemoBlock::NarrowParagraph {
            document: paragraph(
                8.0 * body.font_size,
                body.clone(),
                flush.clone(),
                |builder| {
                    builder.push("术语 ");
                    builder.styled(inter_style(body.font_size), "internationalization");
                    builder.push(" 可按音节转行。");
                },
            ),
            max_width: 8.0 * body.font_size,
        },
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(styled_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            heading.clone(),
            "附录：Emoji 组合字形",
        )),
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            indented.clone(),
            "本附录列出可用于核对的组合字形：👩🏽‍💻、👨‍👩‍👧‍👦、🇨🇳、1️⃣ 与 ✈️。每一项都应作为完整字形参与排版，在换行、选择与绘制时保持一致。",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(styled_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            heading.clone(),
            "附录：连字字形",
        )),
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            TextStyle::builder()
                .font_families(vec!["EB Garamond".to_owned()])
                .font_size(17.0 * physical_scale)
                .build(),
            flush.clone(),
            "EB Garamond: office affinity waffle",
        )),
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            TextStyle::builder()
                .font_families(vec!["FiraCode Nerd Font".to_owned()])
                .font_size(15.0 * physical_scale)
                .build(),
            flush.clone(),
            "FiraCode: -> <= := != === //",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(styled_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            heading,
            "附录：其他语言示例文本",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            "このマークアップ構文は JSX と呼ばれます。React が普及させた JavaScript の構文拡張です。JSX マークアップは関連するレンダリングロジックのすぐそばに配置できるので、React コンポーネントは簡単に作成、保守、削除ができます。",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            "이 마크업 구문을 JSX라 부릅니다. 이것은 React에 의해서 대중화된 자바스크립트 구문의 확장입니다. JSX 마크업을 관련된 렌더링 로직과 가까이 두면, React 컴포넌트를 쉽게 만들고 관리하고 삭제할 수 있습니다.",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            "Cras maximus rutrum magna in gravida. Suspendisse et varius lectus. Ut ac metus id est vehicula euismod ac a sapien. Curabitur pulvinar ornare neque. Proin mattis magna vel massa eleifend cursus. Donec elementum sollicitudin venenatis. Aenean imperdiet consectetur diam, nec mollis leo. ",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body.clone(),
            flush.clone(),
            "Esta sintaxis de marcado se llama JSX. Es una extensión de la sintaxis de JavaScript popularizada por React. Al poner marcado JSX cerca de la lógica de renderizado relacionada hace que los componentes de React sean fáciles de crear, mantener y eliminar.",
        )),
        DemoDocumentDemoBlock::Section { height: section_height },
        DemoDocumentDemoBlock::Paragraph(plain_paragraph(
            physical_content_width,
            body,
            flush,
            "Этот синтаксис разметки называется JSX. Это расширение синтаксиса JavaScript, которое стало популярным благодаря React. Размещение разметки JSX рядом с соответствующей логикой рендеринга упрощает создание, сопровождение и удаление компонентов React.",
        )),
    ]);

    DemoDocumentDemo { blocks }
}

fn list_item(
    marker: &str,
    physical_content_width: f32,
    text_style: TextStyle,
    content: impl FnOnce(&mut ParagraphBuilder),
) -> DemoDocumentDemoBlock {
    let paragraph_style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .build();
    DemoDocumentDemoBlock::ListItem {
        marker: plain_paragraph(
            physical_content_width,
            text_style.clone(),
            paragraph_style.clone(),
            marker,
        ),
        body: paragraph(physical_content_width, text_style, paragraph_style, content),
    }
}

fn plain_paragraph(
    physical_content_width: f32,
    text_style: TextStyle,
    paragraph_style: ParagraphStyle,
    text: &str,
) -> DemoDocument {
    paragraph(physical_content_width, text_style, paragraph_style, |builder| {
        builder.push(text);
    })
}

fn styled_paragraph(
    physical_content_width: f32,
    text_style: TextStyle,
    paragraph_style: ParagraphStyle,
    style: TextStyleOverride,
    text: &str,
) -> DemoDocument {
    paragraph(physical_content_width, text_style, paragraph_style, |builder| {
        builder.styled(style, text);
    })
}

fn paragraph(
    physical_content_width: f32,
    text_style: TextStyle,
    paragraph_style: ParagraphStyle,
    content: impl FnOnce(&mut ParagraphBuilder),
) -> DemoDocument {
    let mut builder = ParagraphBuilder::new(LayoutConstraints::with_defaults(
        physical_content_width.max(1.0),
    ));
    builder
        .text_style(text_style)
        .paragraph_style(paragraph_style);
    content(&mut builder);
    let input = builder
        .build()
        .expect("paragraph demo builder input must be valid");
    DemoDocument { input }
}

fn inter_style(font_size: f32) -> TextStyleOverride {
    TextStyleOverride::builder()
        .font_families(vec!["Inter".to_owned()])
        .font_size(font_size)
        .build()
}

/// 构造普通文本背景的几何参数；数值按 demo 的物理缩放统一换算。
fn background_paint(physical_scale: f32) -> RichTextBackgroundPaint {
    RichTextBackgroundPaint::builder()
        .horizontal_padding(2.0 * physical_scale)
        .vertical_padding(physical_scale)
        .corner_radius(3.0 * physical_scale)
        .build()
}

/// 构造 inline-code 背景的几何参数。
fn code_paint(physical_scale: f32) -> RichTextBackgroundPaint {
    RichTextBackgroundPaint::builder()
        .horizontal_padding(2.0 * physical_scale)
        .vertical_padding(physical_scale)
        .corner_radius(2.0 * physical_scale)
        .build()
}

/// 构造 demo 使用的虚线参数。
fn dashed_paint(physical_scale: f32) -> RichTextLinePaint {
    RichTextLinePaint {
        thickness: physical_scale,
        pattern: RichTextLinePattern::Dashed {
            dash_length: 3.0 * physical_scale,
            gap_length: 2.0 * physical_scale,
        },
        adjacent_same_style_clearance: 0.0,
    }
}

/// 构造 demo 使用的点线参数。
fn dotted_paint(physical_scale: f32) -> RichTextLinePaint {
    RichTextLinePaint {
        thickness: 1.5 * physical_scale,
        pattern: RichTextLinePattern::Dotted {
            gap_length: 1.5 * physical_scale,
        },
        adjacent_same_style_clearance: 0.0,
    }
}

