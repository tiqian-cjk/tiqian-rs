// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/PunctuationGeometryStage.kt

use super::super::clreq::ClreqProfile::{
    AutoSpaceMode, AutoSpacePolicy, KinsokuLevel, PunctuationGluePlacement, PunctuationWidthPolicy,
    clreq_punctuation_policies,
};
use super::super::core::EastAsianSpacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use super::super::core::Geometry::{Rect, TextRange};
use super::super::core::LayoutModel::{
    AutoSpaceDecisionInfo, Cluster, ContextualKinsokuDecisionInfo, Glyph, InlineBoxDecisionInfo,
};
use super::super::core::TextModel::{InlineAttachment, InlineBoxSpan};
use super::super::font::FontPolicy::FontRole;
use super::KinsokuRule::KinsokuRule;
use super::PunctuationGeometryLedger::cluster_index_range_for;
use super::PunctuationModel::{PunctuationAtom, PunctuationAtomBuilder, PunctuationInkInput};
use super::UnicodePunctuationBoundaryResolver::resolve_attached_inline_virtual_boundaries;
use crate::common::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct ContextualKinsoku {
    pub forbidden_line_start_clusters: HashSet<i32>,
    pub unbreakable_ranges: Vec<(i32, i32)>,
    pub impossible_measure_hang_eligible_clusters: HashSet<i32>,
    pub extendable_hang_ranges: Vec<(i32, i32)>,
    pub decisions: Vec<ContextualKinsokuDecisionInfo>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineObjectAttachedMark {
    pub object_cluster_index: i32,
    pub separator_cluster_indices: Vec<i32>,
    pub mark_cluster_index: i32,
}
/** `InlineObjectPunctuationSeparatorSpaceCollapse`：识别 inline object 与后续 prose punctuation 间的作者 ASCII separator，使 layout 折叠其视觉 advance，并将 kinsoku 施加于第一可见 mark。 */
pub fn inline_object_attached_marks(
    clusters: &[Cluster],
    roles: &[FontRole],
    level: KinsokuLevel,
    rule: &dyn KinsokuRule,
) -> Vec<InlineObjectAttachedMark> {
    if level == KinsokuLevel::None {
        return Vec::new();
    }
    let mut out = Vec::new();
    for mark in 1..clusters.len() {
        let c = &clusters[mark];
        let cjk =
            roles.get(mark) == Some(&FontRole::CjkPunctuation) && rule.forbidden_at_line_start(c);
        let ascii = roles.get(mark) == Some(&FontRole::LatinText)
            && c.text
                .chars()
                .next()
                .is_some_and(clreq_punctuation_policies::is_ascii_point_mark);
        if !cjk && !ascii {
            continue;
        }
        let mut previous = mark as i32 - 1;
        let mut separators = Vec::new();
        while previous >= 0
            && is_space_run(&clusters[previous as usize])
            && clusters[previous as usize].range.end()
                == clusters[previous as usize + 1].range.start()
        {
            separators.push(previous);
            previous -= 1;
        }
        if previous < 0
            || !is_inline_object_cluster(&clusters[previous as usize])
            || clusters[previous as usize].range.end()
                != clusters[previous as usize + 1].range.start()
        {
            continue;
        }
        separators.reverse();
        out.push(InlineObjectAttachedMark {
            object_cluster_index: previous,
            separator_cluster_indices: separators,
            mark_cluster_index: mark as i32,
        });
    }
    out
}
/** `InlineObjectAttachedKinsoku`：inline object 无 glyph display text，仍是 attached mark 前的可见 base；pair 若能容纳则不可拆，若自身超 measure 则仅允许 hang 作最后回退。 */
pub fn inline_object_attached_kinsoku(
    clusters: &[Cluster],
    attachments: &[InlineObjectAttachedMark],
    line_break: &[Cluster],
    level: KinsokuLevel,
    body: f32,
    first: f32,
) -> ContextualKinsoku {
    if level == KinsokuLevel::None {
        return empty_contextual();
    }
    assert!(
        clusters.len() == line_break.len(),
        "Inline-object kinsoku requires cluster-for-cluster line-break geometry"
    );
    let mut starts = HashSet::new();
    let mut ranges = Vec::new();
    let mut hang = HashSet::new();
    let mut extend = Vec::new();
    let mut decisions = Vec::new();
    for a in attachments {
        let prev = a.object_cluster_index;
        let index = a.mark_cluster_index;
        let mark = &clusters[index as usize];
        let ascii = mark
            .text
            .chars()
            .next()
            .is_some_and(clreq_punctuation_policies::is_ascii_point_mark);
        for x in &a.separator_cluster_indices {
            starts.insert(*x);
        }
        starts.insert(index);
        let width: f32 = (prev..=index).map(|i| line_break[i as usize].advance).sum();
        let available = if prev == 0 { first } else { body };
        if width <= available {
            ranges.push((prev, index));
        } else {
            let may = mark.display_text.chars().count() == 1
                && matches!(mark.display_text.chars().next(), Some('、' | '，' | '。'))
                || ascii;
            if may {
                for x in &a.separator_cluster_indices {
                    hang.insert(*x);
                }
                hang.insert(index);
                extend.push((prev, index));
            }
        }
        decisions.push(ContextualKinsokuDecisionInfo::new(
            mark.range,
            mark.text.clone(),
            index,
            "LineStart".to_owned(),
            if a.separator_cluster_indices.is_empty() {
                "InlineObjectAttachedKinsoku"
            } else {
                "InlineObjectAttachedKinsokuAcrossCollapsedSeparatorSpace"
            }
            .to_owned(),
        ));
    }
    ContextualKinsoku {
        forbidden_line_start_clusters: starts,
        unbreakable_ranges: ranges,
        impossible_measure_hang_eligible_clusters: hang,
        extendable_hang_ranges: extend,
        decisions,
    }
}
/** `AttachedAsciiPointMarkKinsoku`：直接接触的 ASCII `, . : ; ! ?` 保持 Latin proportional glyph，但不得开始自动换行；连续 point cluster 为同一 protected run。 */
pub fn attached_ascii_point_mark_kinsoku(
    clusters: &[Cluster],
    roles: &[FontRole],
    line_break: &[Cluster],
    level: KinsokuLevel,
    body: f32,
    first: f32,
) -> ContextualKinsoku {
    if level == KinsokuLevel::None {
        return empty_contextual();
    }
    assert!(
        clusters.len() == line_break.len(),
        "Contextual kinsoku requires cluster-for-cluster line-break geometry"
    );
    let (mut starts, mut ranges, mut hang, mut extend, mut decisions) = (
        HashSet::new(),
        Vec::new(),
        HashSet::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut index = 1;
    while index < clusters.len() {
        let c = &clusters[index];
        let p = &clusters[index - 1];
        let begins = roles.get(index) == Some(&FontRole::LatinText)
            && c.text
                .chars()
                .next()
                .is_some_and(clreq_punctuation_policies::is_ascii_point_mark)
            && !p.display_text.is_empty()
            && !p.text.chars().next_back().is_some_and(char::is_whitespace)
            && p.range.end() == c.range.start();
        if !begins {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index;
        while end + 1 < clusters.len()
            && roles.get(end + 1) == Some(&FontRole::LatinText)
            && clusters[end + 1]
                .text
                .chars()
                .next()
                .is_some_and(clreq_punctuation_policies::is_ascii_point_mark)
            && clusters[end].range.end() == clusters[end + 1].range.start()
        {
            end += 1;
        }
        for (i, cluster) in clusters.iter().enumerate().take(end + 1).skip(start) {
            starts.insert(i as i32);
            decisions.push(ContextualKinsokuDecisionInfo::new(
                cluster.range,
                cluster.text.clone(),
                i as i32,
                "LineStart".to_owned(),
                "AttachedAsciiPointMarkKinsoku".to_owned(),
            ));
        }
        ranges.push((start as i32 - 1, end as i32));
        let width: f32 = (start - 1..=end).map(|i| line_break[i].advance).sum();
        if width > if start - 1 == 0 { first } else { body } {
            for i in start..=end {
                hang.insert(i as i32);
            }
            extend.push((start as i32 - 1, end as i32));
        }
        index = end + 1;
    }
    ContextualKinsoku {
        forbidden_line_start_clusters: starts,
        unbreakable_ranges: ranges,
        impossible_measure_hang_eligible_clusters: hang,
        extendable_hang_ranges: extend,
        decisions,
    }
}
pub fn punctuation_atoms(
    cluster: &Cluster,
    em: f32,
    builder: &PunctuationAtomBuilder,
    glyphs: &[Glyph],
    glue: PunctuationGluePlacement,
    width: PunctuationWidthPolicy,
) -> Vec<PunctuationAtom> {
    if cluster.display_text.is_empty() {
        return Vec::new();
    }
    cluster
        .display_text
        .chars()
        .enumerate()
        .filter_map(|(i, c)| {
            builder.build(
                c,
                display_char_source_range(cluster, i),
                em,
                punctuation_ink_input_for(cluster, i, glyphs),
                glue,
                width,
            )
        })
        .collect()
}
/** `MissingInkBoundsFallback` 的记录侧：仅完全没有 shaping 信息时返回 None；有 shaping 但无法归属 ink 时返回带 reason 的 input。 */
fn punctuation_ink_input_for(
    c: &Cluster,
    index: usize,
    glyphs: &[Glyph],
) -> Option<PunctuationInkInput> {
    if glyphs.is_empty() {
        return None;
    }
    let glyph = if glyphs.len() == c.display_text.chars().count() {
        let mut g = glyphs.get(index)?.clone();
        g.x -= glyphs[..index].iter().map(|x| x.advance).sum::<f32>();
        Some(g)
    } else if c.display_text.chars().count() == 1 {
        union_as_single_glyph(glyphs)
    } else {
        None
    };
    let Some(g) = glyph else {
        return Some(
            PunctuationInkInput::builder(0.)
                .ink_bounds(None)
                .bounds_fallback_reason(Some("glyph-cluster-mapping-ambiguous".to_owned()))
                .build(),
        );
    };
    let bounds = g.bounds.map(|b| Rect {
        left: b.left + g.x,
        top: b.top + g.y,
        right: b.right + g.x,
        bottom: b.bottom + g.y,
    });
    Some(
        PunctuationInkInput::builder(g.advance)
            .ink_bounds(bounds)
            .bounds_fallback_reason(
                g.bounds
                    .is_none()
                    .then(|| "shaper-no-ink-bounds".to_owned()),
            )
            .halt_advance(g.halt_advance)
            .halt_placement_x(g.halt_placement_x)
            .build(),
    )
}
fn union_as_single_glyph(glyphs: &[Glyph]) -> Option<Glyph> {
    let mut first = glyphs.first()?.clone();
    let b: Vec<Rect> = glyphs
        .iter()
        .filter_map(|g| {
            g.bounds.map(|x| Rect {
                left: x.left + g.x,
                top: x.top + g.y,
                right: x.right + g.x,
                bottom: x.bottom + g.y,
            })
        })
        .collect();
    if b.is_empty() {
        return Some(first);
    }
    first.advance = glyphs.iter().map(|g| g.advance).sum();
    first.x = 0.;
    first.y = 0.;
    first.halt_advance = None;
    first.halt_placement_x = None;
    first.bounds = Some(Rect {
        left: b.iter().map(|x| x.left).fold(f32::INFINITY, f32::min),
        top: b.iter().map(|x| x.top).fold(f32::INFINITY, f32::min),
        right: b.iter().map(|x| x.right).fold(f32::NEG_INFINITY, f32::max),
        bottom: b.iter().map(|x| x.bottom).fold(f32::NEG_INFINITY, f32::max),
    });
    Some(first)
}
pub fn apply_auto_space_policy(
    clusters: &[Cluster],
    edges: &[EastAsianSpacingEdges],
    attachments: &[InlineAttachment],
    policy: AutoSpacePolicy,
    font_size: f32,
    narrow_leading: &HashSet<i32>,
    narrow_trailing: &HashSet<i32>,
) -> AutoSpaceApplicationResult {
    if clusters.is_empty() {
        return AutoSpaceApplicationResult {
            clusters: Vec::new(),
            decisions: Vec::new(),
        };
    }
    assert!(
        edges.len() == clusters.len(),
        "East_Asian_Spacing values must align with natural clusters."
    );
    assert!(
        attachments.len() == clusters.len(),
        "Inline attachments must align with natural clusters."
    );
    let gap = policy.gap_em * font_size;
    let mut decisions = Vec::new();
    let virtuals = resolve_attached_inline_virtual_boundaries(attachments);
    let mut suppressed = HashSet::new();
    let mut virtual_gap = vec![false; clusters.len()];
    for b in virtuals {
        suppressed.insert(b.previous_cluster_index);
        if let Some(next) = b.next_cluster_index {
            suppressed.insert(b.attached_cluster_range.1);
            let p = b.previous_cluster_index as usize;
            let n = next as usize;
            if !is_space_run(&clusters[n]) && !is_mandatory_break_cluster(&clusters[n]) {
                let narrow = if edges[p].trailing == EastAsianSpacingValue::Wide
                    && edges[n].leading == EastAsianSpacingValue::Narrow
                {
                    clusters[n].text.chars().next()
                } else if edges[p].trailing == EastAsianSpacingValue::Narrow
                    && edges[n].leading == EastAsianSpacingValue::Wide
                {
                    clusters[p].text.chars().next_back()
                } else {
                    None
                };
                virtual_gap[b.attached_cluster_range.1 as usize] =
                    mode_for_narrow(narrow, policy) == Some(AutoSpaceMode::Insert);
            }
        }
    }
    let updated = clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let prev = i.checked_sub(1).map(|x| edges[x].trailing);
            let next = edges.get(i + 1).map(|x| x.leading);
            if is_space_run(c) {
                let narrow = if prev == Some(EastAsianSpacingValue::Wide)
                    && next == Some(EastAsianSpacingValue::Narrow)
                {
                    clusters.get(i + 1).and_then(|x| x.text.chars().next())
                } else if prev == Some(EastAsianSpacingValue::Narrow)
                    && next == Some(EastAsianSpacingValue::Wide)
                {
                    clusters.get(i - 1).and_then(|x| x.text.chars().next_back())
                } else {
                    None
                };
                if mode_for_narrow(narrow, policy).is_none_or(|m| m == AutoSpaceMode::Disabled)
                    || c.advance == gap
                {
                    return c.clone();
                }
                let reduction = c.advance - gap;
                decisions.push(AutoSpaceDecisionInfo {
                    cluster_range: c.range,
                    side: "gap".to_owned(),
                    boundary_role: "EastAsianSpacing.Wide".to_owned(),
                    mode: "Replace".to_owned(),
                    characters_affected: c.text.utf16_len(),
                    reduction_per_char: reduction / c.text.utf16_len() as f32,
                    total_reduction: reduction,
                    reason: "TextAutoSpaceReplace:east-asian-spacing-W-space-N".to_owned(),
                });
                let mut x = c.clone();
                x.advance = gap;
                x
            } else {
                let mut add = 0.;
                let leading = prev == Some(EastAsianSpacingValue::Wide)
                    && edges[i].leading == EastAsianSpacingValue::Narrow
                    && mode_for_narrow(c.text.chars().next(), policy)
                        == Some(AutoSpaceMode::Insert)
                    && !suppressed.contains(&(i as i32 - 1));
                if leading {
                    add += gap;
                    decisions.push(auto_decision(
                        c,
                        "leading",
                        if narrow_leading.contains(&(i as i32)) {
                            "InlineBox.Narrow"
                        } else {
                            "EastAsianSpacing.Wide"
                        },
                        if narrow_leading.contains(&(i as i32)) {
                            "InlineBoxOuterAutoSpace:leading-W-N"
                        } else {
                            "TextAutoSpaceInsert:east-asian-spacing-W-N"
                        },
                        -gap,
                    ));
                }
                let normal = next == Some(EastAsianSpacingValue::Wide)
                    && edges[i].trailing == EastAsianSpacingValue::Narrow
                    && mode_for_narrow(c.text.chars().next_back(), policy)
                        == Some(AutoSpaceMode::Insert)
                    && !suppressed.contains(&(i as i32));
                if normal || virtual_gap[i] {
                    add += gap;
                    decisions.push(auto_decision(
                        c,
                        "trailing",
                        if narrow_trailing.contains(&(i as i32)) {
                            "InlineBox.Narrow"
                        } else if virtual_gap[i] {
                            "InlineAttachment.Previous"
                        } else {
                            "EastAsianSpacing.Wide"
                        },
                        if narrow_trailing.contains(&(i as i32)) {
                            "InlineBoxOuterAutoSpace:trailing-N-W"
                        } else if virtual_gap[i] {
                            "AttachedInlineVirtualAutoSpace:east-asian-spacing-W-N"
                        } else {
                            "TextAutoSpaceInsert:east-asian-spacing-W-N"
                        },
                        -gap,
                    ));
                }
                if add == 0. {
                    c.clone()
                } else {
                    let mut x = c.clone();
                    x.advance += add;
                    x
                }
            }
        })
        .collect();
    AutoSpaceApplicationResult {
        clusters: updated,
        decisions,
    }
}
pub fn is_east_asian_spacing_boundary_at(
    right: usize,
    clusters: &[Cluster],
    edges: &[EastAsianSpacingEdges],
) -> bool {
    let left = right - 1;
    if wide_narrow(edges[left].trailing, edges[right].leading) {
        return true;
    }
    (is_space_run(&clusters[right])
        && edges[left].trailing == EastAsianSpacingValue::Wide
        && edges
            .get(right + 1)
            .is_some_and(|x| x.leading == EastAsianSpacingValue::Narrow))
        || (is_space_run(&clusters[left])
            && edges[right].leading == EastAsianSpacingValue::Wide
            && left > 0
            && edges[left - 1].trailing == EastAsianSpacingValue::Narrow)
}
/** `AttachedAsciiPointMarkOverridesConditionalEastAsianSpacing`：直接附着 ASCII point mark 已受 Chinese kinsoku 约束，不能再次按 C→N 插入 gap；独立 `%`、`#` 等仍遵循 Unicode Conditional。 */
pub fn is_attached_ascii_point_mark_at(clusters: &[Cluster], index: usize) -> bool {
    index > 0
        && clusters[index]
            .text
            .chars()
            .next()
            .is_some_and(clreq_punctuation_policies::is_ascii_point_mark)
        && !clusters[index - 1].display_text.is_empty()
        && !clusters[index - 1]
            .text
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
        && clusters[index - 1].range.end() == clusters[index].range.start()
}
#[derive(Clone, Debug, PartialEq)]
pub struct AutoSpaceApplicationResult {
    pub clusters: Vec<Cluster>,
    pub decisions: Vec<AutoSpaceDecisionInfo>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct InlineBoxApplicationResult {
    pub clusters: Vec<Cluster>,
    pub advance_by_cluster: HashMap<i32, f32>,
    pub decisions: Vec<InlineBoxDecisionInfo>,
}
pub fn apply_inline_box_spans(
    clusters: &[Cluster],
    spans: &[InlineBoxSpan],
) -> InlineBoxApplicationResult {
    if clusters.is_empty() || spans.is_empty() {
        return InlineBoxApplicationResult {
            clusters: clusters.to_vec(),
            advance_by_cluster: HashMap::new(),
            decisions: Vec::new(),
        };
    }
    let (mut lead, mut trail, mut decisions) = (HashMap::new(), HashMap::new(), Vec::new());
    for s in spans {
        if s.range.start() >= s.range.end() {
            continue;
        }
        let Some((f, l)) = cluster_index_range_for(clusters, s.range) else {
            continue;
        };
        if s.inline_start != 0. {
            *lead.entry(f).or_insert(0.) += s.inline_start;
        }
        if s.inline_end != 0. {
            *trail.entry(l).or_insert(0.) += s.inline_end;
        }
        decisions.push(InlineBoxDecisionInfo {
            range: s.range,
            inline_start: s.inline_start,
            inline_end: s.inline_end,
            outer_spacing: format!("{:?}", s.outer_spacing),
            first_cluster_index: f,
            last_cluster_index: l,
            reason: "InlineBoxBoundaryAdvance".to_owned(),
        });
    }
    let mut advances = HashMap::new();
    let resolved = clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let a = lead.get(&(i as i32)).copied().unwrap_or(0.);
            let b = trail.get(&(i as i32)).copied().unwrap_or(0.);
            let structural = a + b;
            if structural != 0. {
                advances.insert(i as i32, structural);
            }
            if structural == 0. && a == 0. {
                c.clone()
            } else {
                let mut x = c.clone();
                x.advance = (x.advance + structural).max(0.);
                x.leading_layout_advance += a;
                x
            }
        })
        .collect();
    InlineBoxApplicationResult {
        clusters: resolved,
        advance_by_cluster: advances,
        decisions,
    }
}
fn empty_contextual() -> ContextualKinsoku {
    ContextualKinsoku {
        forbidden_line_start_clusters: HashSet::new(),
        unbreakable_ranges: Vec::new(),
        impossible_measure_hang_eligible_clusters: HashSet::new(),
        extendable_hang_ranges: Vec::new(),
        decisions: Vec::new(),
    }
}
fn is_space_run(c: &Cluster) -> bool {
    !c.text.is_empty() && c.text.chars().all(|x| x == ' ')
}
fn is_inline_object_cluster(c: &Cluster) -> bool {
    c.font_key == "inline-object"
}
fn is_mandatory_break_cluster(c: &Cluster) -> bool {
    c.font_key == "mandatory-break" && c.display_text.is_empty()
}
fn display_char_source_range(c: &Cluster, i: usize) -> TextRange {
    if c.display_text.utf16_len() == c.text.utf16_len() {
        let start: i32 = c
            .display_text
            .chars()
            .take(i)
            .map(|character| character.len_utf16() as i32)
            .sum();
        let end = start
            + c.display_text
                .chars()
                .nth(i)
                .map_or(0, |character| character.len_utf16() as i32);
        TextRange::new(c.range.start() + start, c.range.start() + end)
    } else {
        c.range
    }
}
fn mode_for_narrow(c: Option<char>, p: AutoSpacePolicy) -> Option<AutoSpaceMode> {
    c.map(|x| {
        if x.is_ascii_digit() {
            p.cjk_digit
        } else {
            p.cjk_latin
        }
    })
}
fn wide_narrow(a: EastAsianSpacingValue, b: EastAsianSpacingValue) -> bool {
    matches!(
        (a, b),
        (EastAsianSpacingValue::Wide, EastAsianSpacingValue::Narrow)
            | (EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Wide)
    )
}
fn auto_decision(
    c: &Cluster,
    side: &str,
    role: &str,
    reason: &str,
    total: f32,
) -> AutoSpaceDecisionInfo {
    AutoSpaceDecisionInfo {
        cluster_range: c.range,
        side: side.to_owned(),
        boundary_role: role.to_owned(),
        mode: "Insert".to_owned(),
        characters_affected: 0,
        reduction_per_char: 0.,
        total_reduction: total,
        reason: reason.to_owned(),
    }
}
