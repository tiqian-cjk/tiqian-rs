# 2026-09-21 字体合成策略与最终实例身份

> 状态：已完成
>
> 关联实现：Huozi 的 HarfRust、SkRifa 与 SDF 字体后端

## 目标

为平台字体后端提供可继承的仿粗体、仿斜体 fallback 策略，并使实际采用的合成参数成为可重放的
`FontFaceId` 身份一部分。

## 范围

本迭代在 Tiqian 中新增：

- `FontSynthesis` 位标志，表达调用方允许的 weight 与 style 合成；
- `TextStyle.font_synthesis` 及 `TextStyleOverride` 的整体继承、覆盖语义；
- `FontSynthesisInstance`，记录最终字体实例实际采用的 em 相对仿粗量与仿斜角度；
- `FontFaceId` 的构造、访问和 debug display，使真实 variation 与实际合成参数同时参与身份。

Huozi 负责真实字体优先级、HarfRust shaping、SkRifa outline shear、ink bounds 与 SDF 阈值外扩。
合成策略不是 OpenType variation，不写入 `FontVariationInstance`；实际合成参数不改变 glyph advance 或
纵向 metrics。

## 设计

`FontSynthesis` 为零依赖的紧凑位标志值：`NONE`、`WEIGHT`、`STYLE`。默认值为 `WEIGHT | STYLE`。
它只表达允许的 fallback 策略，不能作为最终字体身份。

`FontSynthesisInstance` 使用精确 `f32` 位模式保存两个可选参数：`embolden_em` 与
`oblique_degrees`。`FontFaceId::new` 与 `with_resource_id` 仍构造没有合成参数的实例；后端在确定
实际 fallback 后使用 `with_synthesis` 生成新的最终 identity。

## 验证

- `TextStyle::default()` 启用两种合成策略；
- 未指定的 `TextStyleOverride` 继承基础策略，指定值整体替换；
- 相同物理资源与 variation、不同实际合成参数的 `FontFaceId` 不相等，且 display 可区分；
- 原有构造入口仍产生无合成实例。

已执行 `cargo test default_font_synthesis_allows_weight_and_style --lib`、
`cargo test synthetic_parameters_distinguish_final_font_instances --lib`、
`cargo test text_style_override --lib`、`cargo test`（1341 个测试通过）与
`cargo check --all-targets`，均通过；`git diff --check` 无空白错误。

## 回滚

若平台后端尚未完成合成绘制，可将其候选选择保持为真实 face/axis fallback；公开策略与 identity
契约不需要回滚，也不会自行改变布局结果。
