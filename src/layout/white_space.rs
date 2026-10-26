use crate::common::HashMap;
use crate::core::geometry::{ScalarOffset, TextRange};
use crate::core::text::Text;
use crate::core::text_model::{InlineObjectSpan, WhiteSpaceMode};
use alloc::vec::Vec;

/// 单次扫描源文本，样式与源注解边界不截断空白序列。
/// 对象覆盖的源文本不参与解析，与 cluster 角色切分保持一致。
pub fn scan_white_space(
    text: &Text,
    mode: WhiteSpaceMode,
    objects: &HashMap<ScalarOffset, InlineObjectSpan>,
) -> Vec<WhiteSpaceUnit> {
    let mut units: Vec<WhiteSpaceUnit> = Vec::new();
    let mut offset = ScalarOffset::ZERO;
    let length = text.scalar_len();
    let collapse = matches!(
        mode,
        WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks
    );
    let mut previous_space = false;
    let mut after_break = false;
    while offset < length {
        if let Some(object) = objects
            .get(&offset)
            .filter(|object| object.range.end() > offset && object.range.end() <= length)
        {
            offset = object.range.end();
            previous_space = false;
            after_break = false;
            continue;
        }
        let start = offset;
        let character = text.char_at_or_none(offset).unwrap();
        offset += 1;
        let kind = match character {
            ' ' => WhiteSpaceKind::Space,
            '\t' => WhiteSpaceKind::Tab,
            '\r' => {
                if text.char_at_or_none(offset) == Some('\n') && !objects.contains_key(&offset) {
                    offset += 1;
                }
                WhiteSpaceKind::SegmentBreak
            }
            '\n' => WhiteSpaceKind::SegmentBreak,
            '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                WhiteSpaceKind::MandatoryBreak
            }
            _ => {
                previous_space = false;
                after_break = false;
                continue;
            }
        };
        let hard_break = kind == WhiteSpaceKind::MandatoryBreak
            || (kind == WhiteSpaceKind::SegmentBreak && mode != WhiteSpaceMode::Collapse);
        let action = if hard_break {
            if collapse {
                // 每条前置空白记录最多被此移除过程回扫一次。
                let mut boundary = start;
                for unit in units.iter_mut().rev() {
                    if unit.range.end() != boundary
                        || !matches!(unit.kind, WhiteSpaceKind::Space | WhiteSpaceKind::Tab)
                    {
                        break;
                    }
                    unit.action = WhiteSpaceAction::Remove;
                    boundary = unit.range.start();
                }
            }
            previous_space = false;
            after_break = true;
            WhiteSpaceAction::Preserve
        } else if !collapse {
            WhiteSpaceAction::Preserve
        } else if after_break {
            WhiteSpaceAction::Remove
        } else if previous_space {
            WhiteSpaceAction::Collapse
        } else {
            previous_space = true;
            if kind == WhiteSpaceKind::Space {
                WhiteSpaceAction::Preserve
            } else {
                WhiteSpaceAction::ToSpace
            }
        };
        units.push(WhiteSpaceUnit {
            range: TextRange::new(start, offset),
            kind,
            action,
        });
    }
    units
}

/// 与行宽无关的源字符分类，CRLF 占一条记录。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpaceKind {
    Space,
    Tab,
    SegmentBreak,
    MandatoryBreak,
}

/// shaping 前的显示转换，源文本保持不变。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpaceAction {
    /// 保留源类别，只有 Space 产生一个有效 U+0020。
    Preserve,
    /// 生成一个有效 U+0020，使用源范围起点的样式。
    ToSpace,
    /// 被前一个有效空格吸收，不产生显示内容。
    Collapse,
    /// 移除源换行两侧的空白，不产生显示内容。
    Remove,
}

/// 一个源空格、Tab 或换行序列，不依赖样式边界。
/// 被折叠的源偏移共用前一个有效空格的末侧位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WhiteSpaceUnit {
    pub range: TextRange,
    pub kind: WhiteSpaceKind,
    pub action: WhiteSpaceAction,
}

/// 仅属于当前行的调整，不回写宽度无关的空白单元。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineWhiteSpaceAction {
    RemoveStart,
    RemoveEnd,
    ResolveTab,
    HangEnd,
    ConditionallyHangEnd,
    /// 仅移除行边自动间距，独立于原始空白与 repair 分配。
    RemoveAutoSpaceStart,
    RemoveAutoSpaceEnd,
}

/// 候选行测量与最终几何共用的已解析宽度。
/// 悬挂保留占位 advance，只从测量中排除悬挂部分；条件悬挂可仅排除部分宽度。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineWhiteSpaceAdjustment {
    pub cluster_index: i32,
    pub action: LineWhiteSpaceAction,
    pub occupied_advance: f32,
    pub hanging_advance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::text_range;
    use alloc::{vec, vec::Vec};

    #[test]
    fn collapse_converts_segment_breaks_and_tabs_without_changing_source_ranges() {
        let text = Text::from("中 \t\r\n \r\nA");
        let units = scan_white_space(&text, WhiteSpaceMode::Collapse, &HashMap::new());
        assert_eq!(
            units.iter().map(|unit| unit.range).collect::<Vec<_>>(),
            vec![
                text_range(1, 2),
                text_range(2, 3),
                text_range(3, 5),
                text_range(5, 6),
                text_range(6, 8),
            ]
        );
        assert_eq!(units[0].action, WhiteSpaceAction::Preserve);
        assert!(
            units[1..]
                .iter()
                .all(|unit| unit.action == WhiteSpaceAction::Collapse)
        );
        let units = scan_white_space(
            &Text::from("中\r\nA\tB"),
            WhiteSpaceMode::Collapse,
            &HashMap::new(),
        );
        assert!(
            units
                .iter()
                .all(|unit| unit.action == WhiteSpaceAction::ToSpace)
        );
    }

    #[test]
    fn preserve_breaks_removes_surrounding_spaces_and_keeps_crlf_atomic() {
        let units = scan_white_space(
            &Text::from("A \t\r\n \tB"),
            WhiteSpaceMode::PreserveBreaks,
            &HashMap::new(),
        );
        assert_eq!(
            units[2],
            WhiteSpaceUnit {
                range: text_range(3, 5),
                kind: WhiteSpaceKind::SegmentBreak,
                action: WhiteSpaceAction::Preserve,
            }
        );
        assert!(
            units
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != 2)
                .all(|(_, unit)| unit.action == WhiteSpaceAction::Remove)
        );
    }

    #[test]
    fn preservation_keeps_each_space_and_tab_and_all_break_kinds() {
        let text = Text::from("A  \t\r\n\r\n\u{000B}\u{000C}\u{0085}\u{2028}\u{2029}");
        for mode in [WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
            let units = scan_white_space(&text, mode, &HashMap::new());
            assert_eq!(10, units.len());
            assert!(
                units
                    .iter()
                    .all(|unit| unit.action == WhiteSpaceAction::Preserve)
            );
            assert_eq!(
                5,
                units
                    .iter()
                    .filter(|unit| unit.kind == WhiteSpaceKind::MandatoryBreak)
                    .count()
            );
        }
    }

    #[test]
    fn fixed_breaks_stay_mandatory_in_collapse_and_other_spaces_interrupt_runs() {
        let text = Text::from(" \u{00A0} \u{3000} \u{2028} ");
        let units = scan_white_space(&text, WhiteSpaceMode::Collapse, &HashMap::new());
        assert_eq!(5, units.len());
        assert_eq!(WhiteSpaceAction::Preserve, units[0].action);
        assert_eq!(WhiteSpaceAction::Preserve, units[1].action);
        assert_eq!(WhiteSpaceAction::Remove, units[2].action);
        assert_eq!(WhiteSpaceKind::MandatoryBreak, units[3].kind);
        assert_eq!(WhiteSpaceAction::Preserve, units[3].action);
        assert_eq!(WhiteSpaceAction::Remove, units[4].action);
    }
}
