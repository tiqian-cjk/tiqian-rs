// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineOptimization.kt

use std::collections::HashSet;

use super::super::core::Geometry::TextRange;
use super::super::core::IntRange::IntRange;
use super::super::core::LayoutModel::LineEndReason;
use super::super::linebreak::LineBreak::BreakKind;
use super::ProgressiveBreakDecisions::ShrinkChannel;

#[derive(Clone, Debug, PartialEq)]
pub struct BreakCandidate {
    pub index: i32,
    pub kind: BreakKind,
    pub natural_width: f32,
    pub compressed_width: f32,
    pub expanded_width: f32,
    pub forbidden_reason: Option<String>,
    pub repair_options: Vec<RepairOption>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RepairOption {
    PushIn { penalty: i32, reason: String, offender_cluster_index: i32, allocations: Vec<PushInAllocation>, total_shrink: f32, total_available_capacity: f32 },
    Hang { penalty: i32, reason: String, offender_cluster_index: i32 },
    CarryPrevious { penalty: i32, reason: String, offender_cluster_index: i32, carried_cluster_index: i32 },
    CarryNext { penalty: i32, reason: String, moved_cluster_index: i32 },
    LeaveRagged { penalty: i32, reason: String, offender_cluster_index: i32 },
}
impl RepairOption { pub fn penalty(&self)->i32{match self{Self::PushIn{penalty,..}|Self::Hang{penalty,..}|Self::CarryPrevious{penalty,..}|Self::CarryNext{penalty,..}|Self::LeaveRagged{penalty,..}=>*penalty}}pub fn reason(&self)->&str{match self{Self::PushIn{reason,..}|Self::Hang{reason,..}|Self::CarryPrevious{reason,..}|Self::CarryNext{reason,..}|Self::LeaveRagged{reason,..}=>reason}}}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PushInAllocation { pub cluster_index: i32,pub shrink: f32,pub available_capacity: f32,pub channel: ShrinkChannel }
impl PushInAllocation { pub fn new(cluster_index:i32,shrink:f32,available_capacity:f32)->Self{Self{cluster_index,shrink,available_capacity,channel:ShrinkChannel::TrailingGlue}} }

#[derive(Clone, Debug, PartialEq)]
pub struct LineCandidate { pub cluster_range:IntRange,pub source_range:TextRange,pub natural_width:f32,pub adjusted_width:f32,pub end_reason:LineEndReason,pub repair:Option<RepairOption>,pub repair_candidates:Vec<RepairCandidate>,pub hanging_cluster_indices:HashSet<i32> }
impl LineCandidate { pub fn new(cluster_range:IntRange,source_range:TextRange,natural_width:f32,adjusted_width:f32)->Self{let candidate=Self{cluster_range,source_range,natural_width,adjusted_width,end_reason:LineEndReason::AutoWrap,repair:None,repair_candidates:Vec::new(),hanging_cluster_indices:HashSet::new()};candidate.validate_hanging_suffix();candidate}pub fn validate_hanging_suffix(&self){if let Some(first)=self.hanging_cluster_indices.iter().min().copied(){assert!(self.cluster_range.contains(first)&&self.hanging_cluster_indices.iter().max().copied()==Some(self.cluster_range.last()),"Hanging clusters must be a trailing line suffix: line={:?} hanging={:?}",self.cluster_range,self.hanging_cluster_indices);assert_eq!(self.hanging_cluster_indices.len()as i32,self.cluster_range.last()-first+1,"Hanging clusters must be contiguous: line={:?} hanging={:?}",self.cluster_range,self.hanging_cluster_indices)}}pub fn hanging_cluster_index(&self)->Option<i32>{match &self.repair{Some(RepairOption::Hang{offender_cluster_index,..})=>Some(*offender_cluster_index),_=>self.hanging_cluster_indices.iter().max().copied()}}pub fn in_measure_cluster_range(&self)->IntRange{self.hanging_cluster_indices.iter().min().copied().map_or(self.cluster_range,|first|IntRange::new(self.cluster_range.first(),first-1))}}

#[derive(Clone, Debug, PartialEq)]
pub struct RepairCandidate { pub kind:String,pub reason_code:String,pub offender_cluster_index:i32,pub penalty:i32,pub accepted:bool,pub rejection_reason:Option<String>,pub target_cluster_index:Option<i32>,pub carried_cluster_index:Option<i32>,pub shrink:f32,pub required_shrink:f32,pub available_capacity:f32 }
impl RepairCandidate { pub fn new(kind:String,reason_code:String,offender_cluster_index:i32,penalty:i32,accepted:bool)->Self{Self{kind,reason_code,offender_cluster_index,penalty,accepted,rejection_reason:None,target_cluster_index:None,carried_cluster_index:None,shrink:0.,required_shrink:0.,available_capacity:0.}} }

#[derive(Clone, Debug, PartialEq)]
pub struct LineSolution { pub lines:Vec<LineCandidate>,pub total_badness:f32 }
impl LineSolution { pub fn new(lines:Vec<LineCandidate>)->Self{Self{lines,total_badness:0.}}pub fn with_badness(lines:Vec<LineCandidate>,total_badness:f32)->Self{Self{lines,total_badness}} }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineOptimizationStrategy { Greedy, Lookahead, ParagraphDynamicProgramming }
