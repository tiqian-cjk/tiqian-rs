// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/UnicodePunctuationBoundaryResolver.kt

use super::super::core::EastAsianSpacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use super::super::core::LayoutModel::{Cluster, ContextualKinsokuDecisionInfo};
use super::super::core::TextIndex::utf16_offset_to_utf8_byte_index;
use super::super::font::FontPolicy::FontRole;
use super::super::linebreak::LineBreak::{
    is_mandatory_break_code_point, is_zero_width_space_code_point,
};
use super::super::linebreak::UnicodePunctuationLineBreak::{
    UnicodePunctuationLineBreakClass, unicode_punctuation_line_break,
};
use super::QuotePairAnalyzer::QuotePair;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct UnicodePunctuationBoundaries {
    pub forbidden_line_start_clusters: HashSet<i32>,
    pub forbidden_line_end_clusters: HashSet<i32>,
    pub unbreakable_ranges: Vec<(i32, i32)>,
    pub decisions: Vec<ContextualKinsokuDecisionInfo>,
}

/** `WesternBracketCjkInterChar`：Western bracket 保留 Latin face/proportional advance，但直接接触 CJK body 时仍是 CLREQ tier-3 普通字距位置。 */
pub fn resolve_western_bracket_cjk_inter_char_boundaries(
    text: &str,
    clusters: &[Cluster],
    cluster_roles: &[FontRole],
) -> HashSet<i32> {
    (0..clusters.len().saturating_sub(1))
        .filter(|left| {
            is_western_bracket_cjk_inter_char_boundary(
                text,
                clusters,
                cluster_roles,
                *left,
                left + 1,
            )
        })
        .map(|left| left as i32)
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedInlineVirtualBoundary {
    pub previous_cluster_index: i32,
    pub attached_cluster_range: (i32, i32),
    pub next_cluster_index: Option<i32>,
}

/** `AttachedInlineVirtualAdjacency`：attached run 仅在决定其两侧 prose boundary spacing 时被忽略；source order、shaping 和 glyph geometry 不变，边界物理归属 run end。 */
pub fn resolve_attached_inline_virtual_boundaries(
    inline_attachments: &[super::super::core::TextModel::InlineAttachment],
) -> Vec<AttachedInlineVirtualBoundary> {
    use super::super::core::TextModel::InlineAttachment;
    let mut out = Vec::new();
    let mut index = 0;
    while index < inline_attachments.len() {
        if inline_attachments[index] != InlineAttachment::Previous {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = start;
        while end + 1 < inline_attachments.len()
            && inline_attachments[end + 1] == InlineAttachment::Previous
        {
            end += 1;
        }
        if start > 0 {
            out.push(AttachedInlineVirtualBoundary {
                previous_cluster_index: start as i32 - 1,
                attached_cluster_range: (start as i32, end as i32),
                next_cluster_index: (end + 1 < inline_attachments.len()).then_some(end as i32 + 1),
            });
        }
        index = end + 1;
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttachedInlineInterCharBoundaries {
    pub ordinary_western_boundary_after_clusters: HashSet<i32>,
    pub suppressed_physical_boundary_after_clusters: HashSet<i32>,
    pub virtual_boundary_after_clusters: HashMap<i32, i32>,
    pub virtual_sino_western_boundary_after_clusters: HashSet<i32>,
}

pub fn resolve_attached_inline_inter_char_boundaries(
    text: &str,
    clusters: &[Cluster],
    cluster_roles: &[FontRole],
    edges: &[EastAsianSpacingEdges],
    western: &HashSet<i32>,
    attachments: &[super::super::core::TextModel::InlineAttachment],
) -> AttachedInlineInterCharBoundaries {
    assert!(
        clusters.len() == cluster_roles.len() && clusters.len() == edges.len(),
        "Clusters, roles and East_Asian_Spacing edges must align."
    );
    assert!(
        clusters.len() == attachments.len(),
        "Inline attachments must align with clusters."
    );
    let virtuals = resolve_attached_inline_virtual_boundaries(attachments);
    let mut suppressed = HashSet::new();
    for b in &virtuals {
        suppressed.insert(b.previous_cluster_index);
        if let Some(n) = b.next_cluster_index {
            suppressed.insert(b.attached_cluster_range.1);
            let _ = n;
        }
    }
    let ordinary = western
        .iter()
        .copied()
        .filter(|i| !suppressed.contains(i))
        .collect();
    let mut virtual_map = HashMap::new();
    let mut sino = HashSet::new();
    for b in virtuals {
        let Some(next) = b.next_cluster_index else {
            continue;
        };
        let prev = b.previous_cluster_index as usize;
        let nextu = next as usize;
        let both = cluster_roles[prev].is_cjk_like() && cluster_roles[nextu].is_cjk_like();
        let punctuation_western = (cluster_roles[prev] == FontRole::CjkPunctuation
            && edges[nextu].leading == EastAsianSpacingValue::Narrow)
            || (edges[prev].trailing == EastAsianSpacingValue::Narrow
                && cluster_roles[nextu] == FontRole::CjkPunctuation);
        let sw = wide_narrow(edges[prev].trailing, edges[nextu].leading);
        let bracket =
            is_western_bracket_cjk_inter_char_boundary(text, clusters, cluster_roles, prev, nextu);
        if both || punctuation_western || sw || bracket {
            virtual_map.insert(b.attached_cluster_range.1, b.previous_cluster_index);
        }
        if sw {
            sino.insert(b.attached_cluster_range.1);
        }
    }
    AttachedInlineInterCharBoundaries {
        ordinary_western_boundary_after_clusters: ordinary,
        suppressed_physical_boundary_after_clusters: suppressed,
        virtual_boundary_after_clusters: virtual_map,
        virtual_sino_western_boundary_after_clusters: sino,
    }
}

impl FontRole {
    fn is_cjk_like(self) -> bool {
        self == Self::CjkText || self == Self::CjkPunctuation
    }
}
fn wide_narrow(left: EastAsianSpacingValue, right: EastAsianSpacingValue) -> bool {
    matches!(
        (left, right),
        (EastAsianSpacingValue::Wide, EastAsianSpacingValue::Narrow)
            | (EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Wide)
    )
}
fn is_western_bracket_cjk_inter_char_boundary(
    text: &str,
    clusters: &[Cluster],
    roles: &[FontRole],
    left: usize,
    right: usize,
) -> bool {
    let left_bracket = roles.get(left) != Some(&FontRole::CjkPunctuation)
        && cluster_text(text, &clusters[left])
            .and_then(last_significant)
            .is_some_and(|(_, c)| is_western_bracket(c));
    let right_bracket = roles.get(right) != Some(&FontRole::CjkPunctuation)
        && cluster_text(text, &clusters[right])
            .and_then(first_significant)
            .is_some_and(|(_, c)| is_western_bracket(c));
    (left_bracket && roles.get(right) == Some(&FontRole::CjkText))
        || (roles.get(left) == Some(&FontRole::CjkText) && right_bracket)
}
fn is_western_bracket(cp: i32) -> bool {
    matches!(
        unicode_punctuation_line_break::class_of(cp),
        UnicodePunctuationLineBreakClass::OpenPunctuation
            | UnicodePunctuationLineBreakClass::ClosePunctuation
            | UnicodePunctuationLineBreakClass::CloseParenthesis
    )
}

/** `Uax14WesternPunctuationBoundary`：仅补足可裁剪 UAX #14 标点规则；数字、word、combining mark 和 script 规则继续由既有 pipeline 处理。 */
pub fn resolve_unicode_punctuation_boundaries(
    text: &str,
    clusters: &[Cluster],
    roles: &[FontRole],
    quote_pairs: &[QuotePair],
) -> UnicodePunctuationBoundaries {
    let opens: HashSet<i32> = quote_pairs.iter().map(|p| p.open_index).collect();
    let closes: HashSet<i32> = quote_pairs.iter().map(|p| p.close_index).collect();
    let mut starts = HashSet::new();
    let mut ends = HashSet::new();
    let mut ranges = Vec::new();
    let mut decisions = Vec::new();
    for (index, cluster) in clusters.iter().enumerate() {
        if roles.get(index) == Some(&FontRole::CjkPunctuation) || cluster.range.is_empty() {
            continue;
        }
        let Some(source) = cluster_text(text, cluster) else {
            continue;
        };
        let Some((fo, fc)) = first_significant(source) else {
            continue;
        };
        let Some((lo, lc)) = last_significant(source) else {
            continue;
        };
        let first_offset = cluster.range.start() + fo;
        let last_offset = cluster.range.start() + lo;
        let first_class = unicode_punctuation_line_break::class_of(fc);
        let last_class = unicode_punctuation_line_break::class_of(lc);
        let first_dir = quote_direction(text, first_offset, fc, first_class);
        let last_dir = quote_direction(text, last_offset, lc, last_class);
        let paired_close = closes.contains(&first_offset);
        let authored = follows_authored_boundary(text, first_offset);
        let decimal = first_class == UnicodePunctuationLineBreakClass::InfixNumericSeparator
            && decimal_mark_after_space(index, clusters, text);
        let forbid_start = !authored
            && (paired_close
                || matches!(
                    first_dir,
                    QuoteDirection::Final | QuoteDirection::Unresolved
                )
                || (!decimal
                    && matches!(
                        first_class,
                        UnicodePunctuationLineBreakClass::ClosePunctuation
                            | UnicodePunctuationLineBreakClass::CloseParenthesis
                            | UnicodePunctuationLineBreakClass::Exclamation
                            | UnicodePunctuationLineBreakClass::InfixNumericSeparator
                    )));
        if forbid_start {
            starts.insert(index as i32);
            if let Some(prev) = previous_content_cluster(index, clusters, text) {
                ranges.push((prev, index as i32));
            }
            decisions.push(ContextualKinsokuDecisionInfo {
                range: cluster.range,
                source_text: source.to_owned(),
                cluster_index: index as i32,
                forbidden_position: "LineStart".to_owned(),
                reason: if paired_close {
                    "Uax14WesternPunctuationBoundary:PairedClosingQuote".to_owned()
                } else if matches!(
                    first_dir,
                    QuoteDirection::Final | QuoteDirection::Unresolved
                ) {
                    "Uax14WesternPunctuationBoundary:LB19".to_owned()
                } else {
                    format!(
                        "Uax14WesternPunctuationBoundary:{}",
                        rule_for_start(first_class)
                    )
                },
                impossible_measure_fallback: None,
            });
        }
        let paired_open = opens.contains(&last_offset);
        let forbid_end = paired_open
            || matches!(
                last_dir,
                QuoteDirection::Initial | QuoteDirection::Unresolved
            )
            || last_class == UnicodePunctuationLineBreakClass::OpenPunctuation;
        if forbid_end {
            ends.insert(index as i32);
            if let Some(next) = next_content_cluster(index, clusters, text) {
                ranges.push((index as i32, next));
            }
            decisions.push(ContextualKinsokuDecisionInfo {
                range: cluster.range,
                source_text: source.to_owned(),
                cluster_index: index as i32,
                forbidden_position: "LineEnd".to_owned(),
                reason: if paired_open {
                    "Uax14WesternPunctuationBoundary:PairedOpeningQuote".to_owned()
                } else if matches!(
                    last_dir,
                    QuoteDirection::Initial | QuoteDirection::Unresolved
                ) {
                    "Uax14WesternPunctuationBoundary:LB19".to_owned()
                } else {
                    "Uax14WesternPunctuationBoundary:LB14".to_owned()
                },
                impossible_measure_fallback: None,
            });
        }
    }
    ranges.dedup();
    UnicodePunctuationBoundaries {
        forbidden_line_start_clusters: starts,
        forbidden_line_end_clusters: ends,
        unbreakable_ranges: ranges,
        decisions,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuoteDirection {
    Initial,
    Final,
    Unresolved,
    WordApostrophe,
    None,
}
fn quote_direction(
    text: &str,
    offset: i32,
    cp: i32,
    cls: UnicodePunctuationLineBreakClass,
) -> QuoteDirection {
    if cls != UnicodePunctuationLineBreakClass::Quotation {
        return QuoteDirection::None;
    }
    if cp == 0x2019 {
        let l = code_point_before(text, offset).is_some_and(latin_word);
        let r = code_point_at(text, offset + 1).is_some_and(latin_word);
        return if l && r {
            QuoteDirection::WordApostrophe
        } else if !l && r {
            QuoteDirection::Initial
        } else {
            QuoteDirection::Final
        };
    }
    if matches!(cp, 0x00ab | 0x2018 | 0x201b | 0x201c | 0x201f | 0x2039) {
        QuoteDirection::Initial
    } else if matches!(cp, 0x00bb | 0x2019 | 0x201d | 0x203a) {
        QuoteDirection::Final
    } else {
        QuoteDirection::Unresolved
    }
}
fn latin_word(cp: i32) -> bool {
    (0x41..=0x5a).contains(&cp)
        || (0x61..=0x7a).contains(&cp)
        || (0x30..=0x39).contains(&cp)
        || (0xc0..=0x24f).contains(&cp)
}
fn rule_for_start(c: UnicodePunctuationLineBreakClass) -> &'static str {
    match c {
        UnicodePunctuationLineBreakClass::InfixNumericSeparator => "LB15d",
        _ => "LB13",
    }
}
fn cluster_text<'a>(text: &'a str, c: &Cluster) -> Option<&'a str> {
    Some(
        &text[utf16_offset_to_utf8_byte_index(text, c.range.start())?
            ..utf16_offset_to_utf8_byte_index(text, c.range.end())?],
    )
}
fn first_significant(s: &str) -> Option<(i32, i32)> {
    let mut off = 0;
    for c in s.chars() {
        let cp = c as i32;
        if !c.is_whitespace() {
            return Some((off, cp));
        }
        off += c.len_utf16() as i32;
    }
    None
}
fn last_significant(s: &str) -> Option<(i32, i32)> {
    let mut v = Vec::new();
    let mut off = 0;
    for c in s.chars() {
        v.push((off, c as i32, c.is_whitespace()));
        off += c.len_utf16() as i32;
    }
    v.into_iter().rev().find(|x| !x.2).map(|(o, c, _)| (o, c))
}
fn code_point_at(text: &str, off: i32) -> Option<i32> {
    let b = utf16_offset_to_utf8_byte_index(text, off)?;
    text[b..].chars().next().map(|c| c as i32)
}
fn code_point_before(text: &str, off: i32) -> Option<i32> {
    let b = utf16_offset_to_utf8_byte_index(text, off)?;
    text[..b].chars().next_back().map(|c| c as i32)
}
fn follows_authored_boundary(text: &str, offset: i32) -> bool {
    let mut c = offset;
    while c > 0 {
        let Some(p) = code_point_before(text, c) else {
            return true;
        };
        if is_mandatory_break_code_point(p) || is_zero_width_space_code_point(p) {
            return true;
        }
        if !(p <= 0xffff && char::from_u32(p as u32).is_some_and(char::is_whitespace)) {
            return false;
        }
        c -= if p > 0xffff { 2 } else { 1 };
    }
    true
}
fn has_authored_break(s: &str) -> bool {
    s.chars().any(|c| {
        is_mandatory_break_code_point(c as i32) || is_zero_width_space_code_point(c as i32)
    })
}
fn previous_content_cluster(index: usize, clusters: &[Cluster], text: &str) -> Option<i32> {
    for i in (0..index).rev() {
        let s = cluster_text(text, &clusters[i])?;
        if has_authored_break(s) {
            return None;
        }
        if first_significant(s).is_some() {
            return Some(i as i32);
        }
    }
    None
}
fn next_content_cluster(index: usize, clusters: &[Cluster], text: &str) -> Option<i32> {
    for (i, cluster) in clusters.iter().enumerate().skip(index + 1) {
        let s = cluster_text(text, cluster)?;
        if has_authored_break(s) {
            return None;
        }
        if first_significant(s).is_some() {
            return Some(i as i32);
        }
    }
    None
}
fn decimal_mark_after_space(index: usize, clusters: &[Cluster], text: &str) -> bool {
    if index == 0 {
        return false;
    }
    let Some(prev) = cluster_text(text, &clusters[index - 1]) else {
        return false;
    };
    if prev.is_empty() || prev.chars().any(|c| !c.is_whitespace()) {
        return false;
    }
    let current = cluster_text(text, &clusters[index]).unwrap_or("");
    let following = current.chars().nth(1).map(|c| c as i32).or_else(|| {
        cluster_text(text, clusters.get(index + 1)?)
            .and_then(|s| s.chars().next().map(|c| c as i32))
    });
    following.is_some_and(|c| (0x30..=0x39).contains(&c))
}
