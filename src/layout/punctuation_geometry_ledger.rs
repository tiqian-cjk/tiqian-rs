// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/PunctuationGeometryLedger.kt

use super::super::clreq::clreq_profile::PunctuationClass;
use super::super::core::geometry::TextRange;
use super::super::core::layout_model::{
    Cluster, ClusterGeometryDecisionInfo, LineEdgeTrimDecisionInfo, SpacingDecisionInfo,
};
use super::super::core::text_model::InlineAttachment;
use super::punctuation_model::{
    PunctuationAnchor, PunctuationAtom, PunctuationSpacingAdjustment,
    PunctuationSpacingCompressionResult,
};
use super::unicode_punctuation_boundary_resolver::resolve_attached_inline_virtual_boundaries;
use crate::common::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationGeometryLedger {
    natural_clusters: Vec<Cluster>,
    geometries: HashMap<i32, PunctuationClusterGeometry>,
    budgets: HashMap<i32, GlueBudget>,
    justification_delta_by_cluster: HashMap<i32, f32>,
    raw_edge_trim_by_cluster: HashMap<i32, f32>,
    ruby_spread_by_cluster: HashMap<i32, f32>,
    inline_box_advance_by_cluster: HashMap<i32, f32>,
    attached_inline_trailing_glue_by_cluster: HashMap<i32, f32>,
}
impl PunctuationGeometryLedger {
    pub fn from(
        natural_clusters: Vec<Cluster>,
        punctuation_atoms: &[PunctuationAtom],
        spacing_plan: &PunctuationSpacingCompressionResult,
    ) -> Self {
        let geometries = build_geometries(&natural_clusters, punctuation_atoms);
        let budgets = geometries
            .iter()
            .map(|(i, g)| {
                (
                    *i,
                    GlueBudget {
                        leading_natural: g.leading_glue_natural,
                        leading_consumed: g.leading_glue_initially_consumed,
                        trailing_natural: g.trailing_glue_natural,
                        trailing_consumed: g.trailing_glue_initially_consumed,
                    },
                )
            })
            .collect();
        Self {
            natural_clusters,
            geometries,
            budgets,
            justification_delta_by_cluster: HashMap::new(),
            raw_edge_trim_by_cluster: HashMap::new(),
            ruby_spread_by_cluster: HashMap::new(),
            inline_box_advance_by_cluster: HashMap::new(),
            attached_inline_trailing_glue_by_cluster: HashMap::new(),
        }
        .consume_spacing(spacing_plan)
    }
    pub fn resolve_clusters(&self) -> Vec<Cluster> {
        self.natural_clusters
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let resolved = self.resolved_advance(i as i32, c);
                let shift = self
                    .geometries
                    .get(&(i as i32))
                    .map_or(0., |g| g.glyph_inline_shift);
                if resolved == c.advance && shift == 0. {
                    c.clone()
                } else {
                    let mut out = c.clone();
                    out.advance = resolved;
                    out.glyph_inline_shift += shift;
                    out
                }
            })
            .collect()
    }
    pub fn with_inline_box_advances(&self, advance: &HashMap<i32, f32>) -> Self {
        if advance.is_empty() {
            self.clone()
        } else {
            let mut out = self.clone();
            out.inline_box_advance_by_cluster = advance.clone();
            out
        }
    }
    pub fn consume_trailing_by_cluster(&self, consumption: &HashMap<i32, f32>) -> Self {
        let mut out = self.clone();
        out.budgets = consume(&out.budgets, consumption, |b, a| GlueBudget {
            trailing_consumed: (b.trailing_consumed + a).min(b.trailing_natural),
            ..b
        });
        out
    }
    pub fn consume_leading_by_cluster(&self, consumption: &HashMap<i32, f32>) -> Self {
        let mut out = self.clone();
        out.budgets = consume(&out.budgets, consumption, |b, a| GlueBudget {
            leading_consumed: (b.leading_consumed + a).min(b.leading_natural),
            ..b
        });
        out
    }
    pub fn glue_capacities(&self) -> HashMap<i32, GlueCapacity> {
        self.budgets
            .iter()
            .filter_map(|(i, b)| {
                let l = b.leading_remaining();
                let t = b.trailing_remaining();
                (l > 0. || t > 0.).then(|| {
                    (
                        *i,
                        GlueCapacity {
                            leading: l,
                            trailing: t,
                            paired: self.geometries.get(i).and_then(|g| g.anchor)
                                == Some(PunctuationAnchor::Center),
                        },
                    )
                })
            })
            .collect()
    }
    pub fn add_justification_deltas(&self, delta: &HashMap<i32, f32>) -> Self {
        let mut out = self.clone();
        out.justification_delta_by_cluster = delta.clone();
        out
    }
    pub fn with_ruby_spread(&self, spread: &HashMap<i32, f32>) -> Self {
        if spread.is_empty() {
            self.clone()
        } else {
            let mut out = self.clone();
            out.ruby_spread_by_cluster = spread.clone();
            out
        }
    }
    pub fn with_raw_edge_trims(&self, trims: &HashMap<i32, f32>) -> Self {
        if trims.is_empty() {
            return self.clone();
        }
        let mut out = self.clone();
        for (i, a) in trims {
            *out.raw_edge_trim_by_cluster.entry(*i).or_insert(0.) += a;
        }
        out
    }
    /** `AttachedInlineVirtualPunctuationBoundary`：attached run 在决定 punctuation spacing 时被略过；两侧按 prose cluster 相邻重新计算，右标点保留自身必要 leading glue，其余由 attached run trailing edge 拥有。 */
    pub fn resolve_attached_inline_punctuation_boundaries(
        &self,
        attachments: &[InlineAttachment],
        atoms: &[PunctuationAtom],
        em: f32,
    ) -> AttachedInlinePunctuationBoundaryResult {
        assert!(
            attachments.len() == self.natural_clusters.len(),
            "Inline attachments must align with punctuation geometry clusters."
        );
        if self.budgets.is_empty() || !attachments.contains(&InlineAttachment::Previous) {
            return AttachedInlinePunctuationBoundaryResult {
                geometry: self.clone(),
                trailing_glue_by_cluster: HashMap::new(),
                decisions: Vec::new(),
            };
        }
        let mut budgets = self.budgets.clone();
        let mut glue = HashMap::new();
        let mut decisions = Vec::new();
        for b in resolve_attached_inline_virtual_boundaries(attachments) {
            let prev = b.previous_cluster_index;
            let end = b.attached_cluster_range.1;
            let previous_budget = budgets.get(&prev).copied();
            let left = previous_budget.map_or(0., |x| x.trailing_remaining());
            let next = b
                .next_cluster_index
                .filter(|n| !self.is_mandatory_break(*n));
            let next_budget = next.and_then(|n| budgets.get(&n).copied());
            let right = next_budget.map_or(0., |x| x.leading_remaining());
            let left_atom = atoms
                .iter()
                .rev()
                .find(|a| inside(a.range, self.natural_clusters[prev as usize].range));
            let right_atom = next.and_then(|n| {
                atoms
                    .iter()
                    .find(|a| inside(a.range, self.natural_clusters[n as usize].range))
            });
            let next_char =
                next.and_then(|n| self.natural_clusters[n as usize].text.chars().next());
            let natural = left + right;
            let adjusted=if next.is_none(){0.}else if left_atom.is_some()&&right_atom.is_some(){(natural-em/2.).max(0.)}else if left_atom.is_some_and(|a|a.punctuation_class==PunctuationClass::Closing)&&next_char.is_some_and(super::super::clreq::clreq_profile::clreq_punctuation_policies::is_ascii_point_mark){(natural-em/2.).max(0.)}else{natural};
            if let Some(mut p) = previous_budget {
                p.trailing_consumed = p.trailing_natural;
                budgets.insert(prev, p);
            }
            let kept = right.min(adjusted);
            if let (Some(n), Some(mut nb)) = (next, next_budget)
                && kept < right
            {
                nb.leading_consumed = nb.leading_natural - kept;
                budgets.insert(n, nb);
            }
            let target = (adjusted - kept).max(0.);
            if target > 0. {
                glue.insert(end, target);
            }
            if left > 0. || right != adjusted {
                let p = &self.natural_clusters[prev as usize];
                let nc = next.map(|n| &self.natural_clusters[n as usize]);
                decisions.push(SpacingDecisionInfo{range:TextRange::new(p.range.start(),nc.map_or(self.natural_clusters[end as usize].range.end(),|c|c.range.end())),left_char:p.text.chars().next_back().unwrap_or('\0'),right_char:nc.and_then(|c|c.text.chars().next()).unwrap_or('\0'),natural_inner_glue:natural,adjusted_inner_glue:adjusted,reduction:natural-adjusted,reduction_target_range:p.range,reason:(if next.is_none(){"AttachedInlineVirtualPunctuationBoundary:line-end"}else if left_atom.is_some()&&right_atom.is_some(){"AttachedInlineVirtualPunctuationBoundary:adjacent-punctuation"}else if left_atom.is_some_and(|a|a.punctuation_class==PunctuationClass::Closing)&&next_char.is_some_and(super::super::clreq::clreq_profile::clreq_punctuation_policies::is_ascii_point_mark){"AttachedInlineVirtualPunctuationBoundary:ascii-point-mark"}else{"AttachedInlineVirtualPunctuationBoundary:natural"}).to_owned()});
            }
        }
        let mut geometry = self.clone();
        geometry.budgets = budgets;
        for (i, a) in &glue {
            let old = geometry
                .attached_inline_trailing_glue_by_cluster
                .get(i)
                .copied()
                .unwrap_or(0.);
            geometry
                .attached_inline_trailing_glue_by_cluster
                .insert(*i, old.max(*a));
        }
        AttachedInlinePunctuationBoundaryResult {
            geometry,
            trailing_glue_by_cluster: glue,
            decisions,
        }
    }
    pub fn consume_line_edge_glue(
        &self,
        lines: &[super::line_breaker::LineCandidate],
        force_line_end_half_width: bool,
    ) -> LineEdgeTrimResult {
        if lines.is_empty() || self.budgets.is_empty() {
            return LineEdgeTrimResult {
                geometry: self.clone(),
                decisions: Vec::new(),
            };
        }
        let mut lead = HashMap::new();
        let mut trail = HashMap::new();
        let mut decisions = Vec::new();
        for line in lines {
            if line.cluster_range.is_empty() {
                continue;
            }
            if force_line_end_half_width {
                self.schedule_edge(
                    line,
                    line.cluster_range.last(),
                    PunctuationLineEdge::End,
                    &mut lead,
                    &mut trail,
                    &mut decisions,
                );
            }
            self.schedule_edge(
                line,
                line.cluster_range.first(),
                PunctuationLineEdge::Start,
                &mut lead,
                &mut trail,
                &mut decisions,
            );
        }
        let geometry = self
            .consume_leading_by_cluster(&lead)
            .consume_trailing_by_cluster(&trail);
        LineEdgeTrimResult {
            geometry,
            decisions,
        }
    }
    fn schedule_edge(
        &self,
        line: &super::line_breaker::LineCandidate,
        index: i32,
        edge: PunctuationLineEdge,
        lead: &mut HashMap<i32, f32>,
        trail: &mut HashMap<i32, f32>,
        decisions: &mut Vec<LineEdgeTrimDecisionInfo>,
    ) {
        let Some(b) = self.budgets.get(&index) else {
            return;
        };
        let l = (b.leading_remaining() - lead.get(&index).copied().unwrap_or(0.)).max(0.);
        let t = (b.trailing_remaining() - trail.get(&index).copied().unwrap_or(0.)).max(0.);
        let paired =
            self.geometries.get(&index).and_then(|g| g.anchor) == Some(PunctuationAnchor::Center);
        let side = if paired { l.min(t) } else { 0. };
        let la = if paired {
            side
        } else if edge == PunctuationLineEdge::Start {
            l
        } else {
            0.
        };
        let ta = if paired {
            side
        } else if edge == PunctuationLineEdge::End {
            t
        } else {
            0.
        };
        let total = la + ta;
        if total <= 0. {
            return;
        }
        if la > 0. {
            *lead.entry(index).or_insert(0.) += la;
        }
        if ta > 0. {
            *trail.entry(index).or_insert(0.) += ta;
        }
        decisions.push(LineEdgeTrimDecisionInfo {
            line_range: line.source_range,
            cluster_range: self.natural_clusters[index as usize].range,
            side: if paired {
                "both".to_owned()
            } else {
                edge.side().to_owned()
            },
            trim_amount: total,
            consumed_before: if paired {
                b.leading_consumed + b.trailing_consumed
            } else if edge == PunctuationLineEdge::Start {
                b.leading_consumed
            } else {
                b.trailing_consumed
            },
            natural_glue: if paired {
                b.leading_natural + b.trailing_natural
            } else if edge == PunctuationLineEdge::Start {
                b.leading_natural
            } else {
                b.trailing_natural
            },
            reason: if paired {
                format!(
                    "Line{}CenteredPunctuationPairedCompression",
                    edge.reason_part()
                )
            } else {
                format!("Line{}HalfWidthPunctuation", edge.reason_part())
            },
        })
    }
    pub fn to_decision_info(&self) -> Vec<ClusterGeometryDecisionInfo> {
        self.natural_clusters
            .iter()
            .enumerate()
            .filter_map(|(index, cluster)| {
                let i = index as i32;
                let g = self.geometries.get(&i)?;
                let b = self.budgets[&i];
                Some(ClusterGeometryDecisionInfo {
                    range: g.range,
                    source_text: g.source_text.clone(),
                    display_text: g.display_text.clone(),
                    base_advance: g.base_advance,
                    body_width: g.body_width,
                    leading_glue_natural: b.leading_natural,
                    leading_glue_consumed: b.leading_consumed,
                    trailing_glue_natural: b.trailing_natural,
                    trailing_glue_consumed: b.trailing_consumed,
                    justification_delta: self
                        .justification_delta_by_cluster
                        .get(&i)
                        .copied()
                        .unwrap_or(0.),
                    ruby_spread: self.ruby_spread_by_cluster.get(&i).copied().unwrap_or(0.),
                    glyph_inline_shift: g.glyph_inline_shift,
                    glyph_placement_reason: g.glyph_placement_reason.clone(),
                    resolved_advance: self.resolved_advance(i, cluster),
                    source: "PunctuationGeometryLedger".to_owned(),
                    reason: g.reason.clone(),
                })
            })
            .collect()
    }
    fn consume_spacing(mut self, plan: &PunctuationSpacingCompressionResult) -> Self {
        self.budgets = consume_by_range(
            &self.budgets,
            &self.natural_clusters,
            &self.geometries,
            &plan.adjustments,
        );
        self
    }
    fn resolved_advance(&self, i: i32, c: &Cluster) -> f32 {
        let raw = self.raw_edge_trim_by_cluster.get(&i).copied().unwrap_or(0.);
        let spread = self.ruby_spread_by_cluster.get(&i).copied().unwrap_or(0.);
        let Some(g) = self.geometries.get(&i) else {
            return (c.advance
                + self
                    .justification_delta_by_cluster
                    .get(&i)
                    .copied()
                    .unwrap_or(0.)
                + spread
                + self
                    .attached_inline_trailing_glue_by_cluster
                    .get(&i)
                    .copied()
                    .unwrap_or(0.)
                - raw)
                .max(0.);
        };
        let inline = self
            .inline_box_advance_by_cluster
            .get(&i)
            .copied()
            .unwrap_or(0.);
        let Some(b) = self.budgets.get(&i) else {
            return (g.body_width
                + inline
                + self
                    .justification_delta_by_cluster
                    .get(&i)
                    .copied()
                    .unwrap_or(0.)
                + spread
                - raw)
                .max(0.);
        };
        (g.body_width
            + b.leading_remaining()
            + b.trailing_remaining()
            + self
                .justification_delta_by_cluster
                .get(&i)
                .copied()
                .unwrap_or(0.)
            + spread
            - raw
            + inline
            + self
                .attached_inline_trailing_glue_by_cluster
                .get(&i)
                .copied()
                .unwrap_or(0.))
        .max(0.)
    }
    fn is_mandatory_break(&self, index: i32) -> bool {
        let c = &self.natural_clusters[index as usize];
        c.font_key == "mandatory-break" && c.display_text.is_empty()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedInlinePunctuationBoundaryResult {
    pub geometry: PunctuationGeometryLedger,
    pub trailing_glue_by_cluster: HashMap<i32, f32>,
    pub decisions: Vec<SpacingDecisionInfo>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationClusterGeometry {
    pub range: TextRange,
    pub source_text: super::super::core::text::Text,
    pub display_text: super::super::core::text::Text,
    pub base_advance: f32,
    pub body_width: f32,
    pub leading_glue_natural: f32,
    pub trailing_glue_natural: f32,
    pub leading_glue_initially_consumed: f32,
    pub trailing_glue_initially_consumed: f32,
    pub glyph_inline_shift: f32,
    pub glyph_placement_reason: Option<String>,
    pub anchor: Option<PunctuationAnchor>,
    pub reason: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlueBudget {
    pub leading_natural: f32,
    pub leading_consumed: f32,
    pub trailing_natural: f32,
    pub trailing_consumed: f32,
}
impl GlueBudget {
    pub fn leading_remaining(self) -> f32 {
        (self.leading_natural - self.leading_consumed).max(0.)
    }
    pub fn trailing_remaining(self) -> f32 {
        (self.trailing_natural - self.trailing_consumed).max(0.)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct LineEdgeTrimResult {
    pub geometry: PunctuationGeometryLedger,
    pub decisions: Vec<LineEdgeTrimDecisionInfo>,
}
#[derive(Clone, Copy, PartialEq)]
enum PunctuationLineEdge {
    Start,
    End,
}
impl PunctuationLineEdge {
    fn side(self) -> &'static str {
        if self == Self::Start {
            "leading"
        } else {
            "trailing"
        }
    }
    fn reason_part(self) -> &'static str {
        if self == Self::Start { "Start" } else { "End" }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlueCapacity {
    pub leading: f32,
    pub trailing: f32,
    pub paired: bool,
}
pub fn cluster_index_range_for(
    clusters: &[Cluster],
    source_range: TextRange,
) -> Option<(i32, i32)> {
    let first = clusters.partition_point(|cluster| cluster.range.start() < source_range.start());
    let last_exclusive = first
        + clusters[first..].partition_point(|cluster| cluster.range.end() <= source_range.end());
    (first < last_exclusive).then_some((first as i32, last_exclusive as i32 - 1))
}
fn build_geometries(
    clusters: &[Cluster],
    atoms: &[PunctuationAtom],
) -> HashMap<i32, PunctuationClusterGeometry> {
    let mut out = HashMap::new();
    for (i, c) in clusters.iter().enumerate() {
        let v: Vec<_> = atoms.iter().filter(|a| inside(a.range, c.range)).collect();
        if v.is_empty() {
            continue;
        }
        let one = (v.len() == 1).then(|| v[0]);
        out.insert(
            i as i32,
            PunctuationClusterGeometry {
                range: c.range,
                source_text: c.text.clone(),
                display_text: c.display_text.clone(),
                base_advance: c.advance,
                body_width: v.iter().map(|a| a.body_width).sum(),
                leading_glue_natural: v[0].leading_glue.natural,
                trailing_glue_natural: v[v.len() - 1].trailing_glue.natural,
                leading_glue_initially_consumed: v[0].leading_glue_initially_consumed,
                trailing_glue_initially_consumed: v[v.len() - 1].trailing_glue_initially_consumed,
                glyph_inline_shift: one.map_or(0., |a| a.glyph_inline_shift),
                glyph_placement_reason: one.and_then(|a| a.glyph_placement_reason.clone()),
                anchor: one.map(|a| a.anchor),
                reason: v[0].geometry_source.clone(),
            },
        );
    }
    out
}
fn inside(r: TextRange, o: TextRange) -> bool {
    r.start() >= o.start() && r.end() <= o.end()
}
fn consume<F: Fn(GlueBudget, f32) -> GlueBudget>(
    source: &HashMap<i32, GlueBudget>,
    amounts: &HashMap<i32, f32>,
    apply: F,
) -> HashMap<i32, GlueBudget> {
    let mut out = source.clone();
    for (i, a) in amounts {
        if *a > 0.
            && let Some(b) = out.get(i).copied()
        {
            out.insert(*i, apply(b, *a));
        }
    }
    out
}
fn consume_by_range(
    source: &HashMap<i32, GlueBudget>,
    clusters: &[Cluster],
    geometry: &HashMap<i32, PunctuationClusterGeometry>,
    adjustments: &[PunctuationSpacingAdjustment],
) -> HashMap<i32, GlueBudget> {
    let mut out = source.clone();
    for a in adjustments {
        let target = clusters
            .iter()
            .position(|c| inside(a.reduction_target_range, c.range));
        let Some(i) = target else { continue };
        let key = i as i32;
        let Some(mut b) = out.get(&key).copied() else {
            continue;
        };
        if geometry.get(&key).and_then(|g| g.anchor) == Some(PunctuationAnchor::Center) {
            let p = (a.reduction / 2.)
                .min(b.leading_remaining())
                .min(b.trailing_remaining());
            b.leading_consumed += p;
            b.trailing_consumed += p;
        } else if b.trailing_remaining() >= b.leading_remaining() {
            b.trailing_consumed = (b.trailing_consumed + a.reduction).min(b.trailing_natural)
        } else {
            b.leading_consumed = (b.leading_consumed + a.reduction).min(b.leading_natural)
        }
        out.insert(key, b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clreq::clreq_profile::{PunctuationGluePlacement, PunctuationWidthPolicy};
    use crate::core::text::Text;
    use crate::layout::punctuation_geometry_stage::punctuation_atoms;
    use crate::layout::punctuation_model::{PunctuationAtomBuilder, PunctuationSpacingCompressionResult};

    #[test]
    fn geometry_without_budget_falls_back_to_body_width() {
        let clusters = vec![
            Cluster::new(TextRange::new(0, 1), Text::from("「"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("中"), "cjk".to_owned(), 16.0),
        ];
        let builder = PunctuationAtomBuilder::default();
        let atoms = punctuation_atoms(
            &clusters[0],
            16.0,
            &builder,
            &[],
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        );
        let mut ledger = PunctuationGeometryLedger::from(
            clusters,
            &atoms,
            &PunctuationSpacingCompressionResult::new(Vec::new()),
        );
        ledger.budgets.clear();
        assert_eq!(8.0, ledger.resolve_clusters()[0].advance);

        let decorated = ledger
            .add_justification_deltas(&HashMap::from([(0, 1.0)]))
            .with_ruby_spread(&HashMap::from([(0, 2.0)]))
            .with_raw_edge_trims(&HashMap::from([(0, 1.0)]));
        assert_eq!(10.0, decorated.resolve_clusters()[0].advance);
    }

    #[test]
    fn attached_boundary_records_null_characters_for_empty_text_clusters() {
        let textless_next = vec![
            Cluster::new(TextRange::new(0, 1), Text::from("」"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("r"), "latin".to_owned(), 16.0),
            Cluster::with_display_text(
                TextRange::new(2, 2),
                Text::from(""),
                Text::from("a"),
                "latin".to_owned(),
                16.0,
            ),
        ];
        let builder = PunctuationAtomBuilder::default();
        let atoms = punctuation_atoms(
            &textless_next[0],
            16.0,
            &builder,
            &[],
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        );
        let ledger = PunctuationGeometryLedger::from(
            textless_next,
            &atoms,
            &PunctuationSpacingCompressionResult::new(Vec::new()),
        );
        let result = ledger.resolve_attached_inline_punctuation_boundaries(
            &[
                InlineAttachment::None,
                InlineAttachment::Previous,
                InlineAttachment::None,
            ],
            &atoms,
            16.0,
        );
        assert_eq!('\0', result.decisions[0].right_char);
        assert_eq!("AttachedInlineVirtualPunctuationBoundary:natural", result.decisions[0].reason);
        assert_eq!(8.0, result.trailing_glue_by_cluster[&1]);

        let textless_previous = vec![
            Cluster::with_display_text(
                TextRange::new(0, 0),
                Text::from(""),
                Text::from("」"),
                "cjk".to_owned(),
                16.0,
            ),
            Cluster::new(TextRange::new(0, 1), Text::from("r"), "latin".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("「"), "cjk".to_owned(), 16.0),
        ];
        let previous_atoms: Vec<_> = textless_previous
            .iter()
            .flat_map(|cluster| {
                punctuation_atoms(
                    cluster,
                    16.0,
                    &builder,
                    &[],
                    PunctuationGluePlacement::MainlandSimplified,
                    PunctuationWidthPolicy::default(),
                )
            })
            .collect();
        let ledger = PunctuationGeometryLedger::from(
            textless_previous,
            &previous_atoms,
            &PunctuationSpacingCompressionResult::new(Vec::new()),
        );
        let result = ledger.resolve_attached_inline_punctuation_boundaries(
            &[
                InlineAttachment::None,
                InlineAttachment::Previous,
                InlineAttachment::None,
            ],
            &previous_atoms,
            16.0,
        );
        assert_eq!('\0', result.decisions[0].left_char);
        assert_eq!("AttachedInlineVirtualPunctuationBoundary:adjacent-punctuation", result.decisions[0].reason);
    }
}
