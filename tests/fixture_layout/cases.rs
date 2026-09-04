use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid,
    ParagraphStyle, RubyKind, RubySpan, TiqianTextContent,
};
use tiqian::core::units::Ic;

pub struct Fixture {
    pub id: &'static str,
    pub input: LayoutInput,
    pub use_english_hyphenation: bool,
    pub pin_basic_no_hang: bool,
}

pub fn fixtures() -> Vec<Fixture> {
    vec![
        fixture("basic-pause-stop", "中文，中文。", 160.0),
        fixture("ellipsis-and-dash", "中文……English——中文。", 220.0),
        fixture("nested-quotes", "他说：“你好，世界。”", 180.0),
        fixture("adjacent-punctuation-spacing", "他说：“你好，世界。”！！", 220.0),
        fixture("contextual-curly-quotes", "中‘that’s’中’，‘", 192.0),
        fixture("mixed-script-quote-paragraph-language", "“Json是谁？”", 192.0),
        fixture("adjacent-curly-quote-list-context", "中文“对A”“波霸”；中文“欧派”“double”“double may”呢", 320.0),
        fixture("mi10s-adjacent-curly-quote-wrap", "所以这个和 “骑ji” “说shui”“斜xiá”不一样，港台是从众的，大陆读音大多数源自韵书。", 160.0),
        without_grid("mi10s-western-bracket-citation-wrap", "史力军,姚晨,杨国玉,等.常见有机化合物中文词汇的读音详解[J].化学教育(中英文), 2023, 44(10):21-38.", 272.0),
        without_grid("bibliographic-numeric-locator-break", "中文中文中文44(10):21-38.", 224.0),
        fixture("unmatched-curly-quotes", "’90s James’； “truncated；中文“未闭", 240.0),
        fixture("fallback-roles", "提椠……Hello——世界。", 240.0),
        fixture("greedy-multi-line", "咖啡馆比咖啡更早地改变了城里人的作息与谈吐。", 144.0),
        pinned("kinsoku-carry-previous", "提椠中文中文中文。", 64.0),
        pinned("kinsoku-push-in", "中文中。", 60.0),
        pinned("lookahead-future-push-in", "中文中文中文。", 60.0),
        pinned("lookahead-avoids-repair", "中文中文中文。", 48.0),
        fixture("justify-cjk-paragraph", "中文中文中文中文中文中文", 100.0),
        fixture("justify-mixed-paragraph", "中文Hello中文，世界。", 144.0),
        without_grid("justify-unbreakable-number-symbol", "中文50℃中文中文中文Example", 128.0),
        fixture("ascii-brackets-in-cjk", "中文段落(English)和[mixed]说明。", 240.0),
        pinned("ascii-point-mark-in-cjk", "中文中文,中文", 64.0),
        pinned("ascii-point-mark-impossible-measure", "中,文", 15.0),
        first_line_indent("real-paragraph-1", "咖啡（coffee）在十七世纪经威尼斯传入欧洲。最初它被当作药物出售，价格高得吓人，真正让它流行起来的是随后遍地开花的咖啡馆——读报、辩论、下棋、写作——城市生活忽然多出一个公共客厅。意大利人做出了 espresso，维也纳人往杯里加奶油，土耳其人坚持连渣同煮……每座城市都相信自己手里那一杯才是正统。有人说：「先有咖啡馆，后有启蒙运动」。这话说得夸张，但也不算太离谱。", 320.0, Some(2.0)),
        fixture("latin-word-wrap", "他引用了一句话：The quick brown fox jumps over the lazy dog，然后继续讲。", 240.0),
        emphasis_marks(),
        ruby_line_height(),
        bopomofo_tone_em_box(),
        first_line_indent("first-line-indent", "咖啡的风味因产地而各异，烘焙的深浅同样会改变口感与香气。", 200.0, Some(2.0)),
        fixture("latin-camelcase", "用PowerPoint做", 128.0),
        fixture("latin-existing-hyphen", "out-of-the-way", 128.0),
        fixture("latin-hard-break", "中Network", 64.0),
        fixture("latin-opaque-url-token", "链接 https://example.com/path/to/abc123def456ghi789", 160.0),
        fixture("zero-width-space-soft-break", "A.\u{200B}.\u{200B}.Complete？AaFont？", 96.0),
        hyphenated("western-hyphenation", "请运行 internationalization 命令", 160.0),
        progressive_technical_inline(),
        progressive_technical_hash_fill(),
        progressive_technical_alpha_numeric(),
        progressive_technical_current_line_emergency(),
        first_line_indent("adaptive-short-line-indent", "提椠是一个面向中文正文的排版引擎", 160.0, None),
        fixture("mandatory-single-newline", "第一行\n第二行", 160.0),
        fixture("mandatory-blank-lines", "甲\n\n乙\n", 160.0),
        fixture("mandatory-leading-trailing-newline", "\n开头和结尾\n", 160.0),
        fixture("mandatory-crlf", "甲\r\n乙", 160.0),
        fixture("mandatory-wraps-long-line", "中文中文中文中文中文\n尾行", 64.0),
        first_line_indent("indent-opening-quote", "“好咖啡要趁热喝。”他说完便把杯子推了过来，让大家依次尝一口。", 192.0, Some(2.0)),
        pinned("line-end-kinsoku", "中文中文（中文）中文", 80.0),
        interlinear_lines(),
        mourning_frame(),
        fixture("contextual-dash-ellipsis", "中文—下句；等…真。 English — next; ellipsis… / slash. A——B; Wait……what? 中文—English\n——中文\n……", 1024.0),
        fixture("parenthetical-dash-pairs", "他彻夜想Jessica——Jessica是他的前女友——睡不着觉。地点——北京，时间——明天。", 1024.0),
        fixture("quote-digit-boundaries", "中文 le“t”ters 中1“1”2文；中Ａ“Ｂ”Ｃ文。尾号是“1‘2’3”，用时1’30”。", 1024.0),
    ]
}

fn fixture(id: &'static str, text: &str, width: f32) -> Fixture {
    Fixture {
        id,
        input: input(text, width),
        use_english_hyphenation: false,
        pin_basic_no_hang: false,
    }
}

fn input(text: &str, width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(width),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

fn pinned(id: &'static str, text: &str, width: f32) -> Fixture {
    let mut fixture = fixture(id, text, width);
    fixture.pin_basic_no_hang = true;
    fixture
}

fn hyphenated(id: &'static str, text: &str, width: f32) -> Fixture {
    let mut fixture = fixture(id, text, width);
    fixture.use_english_hyphenation = true;
    fixture
}

fn without_grid(id: &'static str, text: &str, width: f32) -> Fixture {
    let mut fixture = fixture(id, text, width);
    fixture.input.paragraph_style.line_length_grid = LineLengthGrid::with_enabled(false);
    fixture
}

fn first_line_indent(id: &'static str, text: &str, width: f32, indent: Option<f32>) -> Fixture {
    let mut fixture = fixture(id, text, width);
    fixture.input.paragraph_style.first_line_indent = indent.map(|count| Ic { count });
    fixture
}

fn emphasis_marks() -> Fixture {
    let mut fixture = fixture("emphasis-marks", "他强调：豆子新鲜最要紧，烘焙其次。", 128.0);
    fixture.input.paragraph_style.line_height = Some(25.6);
    fixture.input.decorations = vec![DecorationSpan {
        range: text_range(4, 16),
        kind: DecorationKind::Emphasis,
    }];
    fixture
}

fn ruby_line_height() -> Fixture {
    let mut fixture = fixture("ruby-line-height", "甲乙丙丁戊己庚辛壬癸子丑", 64.0);
    fixture.input.paragraph_style.line_height = Some(18.0);
    fixture.input.ruby_spans = vec![RubySpan::new(text_range(4, 5), Text::from("wù"))];
    fixture
}

fn bopomofo_tone_em_box() -> Fixture {
    let mut fixture = fixture("bopomofo-tone-em-box", "好", 64.0);
    fixture.input.ruby_spans = vec![RubySpan::with_kind(
        text_range(0, 1),
        Text::from("ㄏㄠˇ"),
        RubyKind::Bopomofo,
    )];
    fixture
}

fn progressive_technical_inline() -> Fixture {
    let mut fixture = hyphenated("progressive-technical-inline", "中文 internationalization 命令", 160.0);
    fixture.input.content.line_break_spans = vec![LineBreakSpan {
        range: text_range(3, 23),
        policy: LineBreakPolicy::ProgressiveTechnical,
    }];
    fixture
}

fn progressive_technical_hash_fill() -> Fixture {
    let mut fixture = without_grid("progressive-technical-hash-fill", "deadbeefcafebabefeedfaceabcdefabcdef", 173.0);
    fixture.use_english_hyphenation = true;
    fixture.input.content.line_break_spans = vec![LineBreakSpan {
        range: text_range(0, 36),
        policy: LineBreakPolicy::ProgressiveTechnical,
    }];
    fixture
}

fn progressive_technical_alpha_numeric() -> Fixture {
    let mut fixture = without_grid("progressive-technical-alpha-numeric", "Machine2Machine", 76.0);
    fixture.use_english_hyphenation = true;
    fixture.input.content.line_break_spans = vec![LineBreakSpan {
        range: text_range(0, 15),
        policy: LineBreakPolicy::ProgressiveTechnical,
    }];
    fixture
}

fn progressive_technical_current_line_emergency() -> Fixture {
    let mut fixture = without_grid("progressive-technical-current-line-emergency", "Swift 这边是我最有体感的。JSONDecoder 慢是个老问题，SR-6252[36] 那个 issue 里挖出的根因是底层走 NSJSONSerialization 再桥接回 Objective-C，swift_dynamicCast 吃掉大量时间。", 579.0);
    fixture.input.content.line_break_spans = vec![
        LineBreakSpan { range: text_range(16, 27), policy: LineBreakPolicy::ProgressiveTechnical },
        LineBreakSpan { range: text_range(67, 86), policy: LineBreakPolicy::ProgressiveTechnical },
        LineBreakSpan { range: text_range(104, 121), policy: LineBreakPolicy::ProgressiveTechnical },
    ];
    fixture
}

fn interlinear_lines() -> Fixture {
    let mut fixture = fixture("interlinear-lines", "屈原写下离骚，顾炎武王夫之并称。", 224.0);
    fixture.input.decorations = vec![
        DecorationSpan { range: text_range(0, 2), kind: DecorationKind::ProperNoun },
        DecorationSpan { range: text_range(4, 6), kind: DecorationKind::BookTitle },
        DecorationSpan { range: text_range(7, 10), kind: DecorationKind::ProperNoun },
        DecorationSpan { range: text_range(10, 13), kind: DecorationKind::ProperNoun },
    ];
    fixture
}

fn mourning_frame() -> Fixture {
    let mut fixture = fixture("mourning-frame", "悼念：王小明同志、张大同同志。", 72.0);
    fixture.input.decorations = vec![
        DecorationSpan { range: text_range(3, 6), kind: DecorationKind::Mourning },
        DecorationSpan { range: text_range(9, 12), kind: DecorationKind::Mourning },
    ];
    fixture
}