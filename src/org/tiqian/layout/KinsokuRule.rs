// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/KinsokuRule.kt

use super::super::clreq::ClreqProfile::{KinsokuLevel, clreq_punctuation_policies};
use super::super::core::LayoutModel::Cluster;

/**
 * KinsokuRule：CJK 标点的行首/行尾禁则。
 *
 * 默认 [`ClreqKinsokuRule`] 在给定 CLREQ [`KinsokuLevel`]（默认 [`KinsokuLevel::Basic`]，最推荐）
 * 下读取 `clreq_punctuation_policies`。profile 专属覆盖应使用另一个 level 构造或替换此规则，
 * 而不是编辑引擎。
 */
pub trait KinsokuRule {
    fn forbidden_at_line_start(&self, cluster: &Cluster) -> bool;
    fn forbidden_at_line_end(&self, cluster: &Cluster) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub struct ClreqKinsokuRule {
    level: KinsokuLevel,
}

impl Default for ClreqKinsokuRule {
    fn default() -> Self {
        Self::new(KinsokuLevel::Basic)
    }
}

impl ClreqKinsokuRule {
    pub fn new(level: KinsokuLevel) -> Self {
        Self { level }
    }
}

impl KinsokuRule for ClreqKinsokuRule {
    fn forbidden_at_line_start(&self, cluster: &Cluster) -> bool {
        cluster
            .display_text
            .chars()
            .next()
            .is_some_and(|character| clreq_punctuation_policies::forbidden_at_line_start(character, self.level))
    }

    fn forbidden_at_line_end(&self, cluster: &Cluster) -> bool {
        cluster
            .display_text
            .chars()
            .next()
            .is_some_and(|character| clreq_punctuation_policies::forbidden_at_line_end(character, self.level))
    }
}
