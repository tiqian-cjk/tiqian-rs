# 2026-09-09 Replay 索引构造性能

## 目标与设计

demo 新图中 replay 构造共 3,718 个样本，其中 positioned-cluster 查询为 2,295。
迁移将可见块绘制准备提前到整页更新，暴露了共享查询内部重复派生和全表扫描。

保留公开查询签名与全部 replay 字段。构造索引时只生成一次 positioned clusters 和
普通富文本片段，背景、装饰线复用这些输入。positioned-cluster 查询按 range 建立
glyph、leading geometry 和 autospace 查找表，保持原遍历顺序与首次匹配规则。
单 scalar cluster 不收集内部 source stops 所需 glyph。

不改变 source coordinate、ruby selection geometry、布局 decision、点线规则或 renderer。
辅助查询采用 crate 内可见性，普通查询与 replay 复用同一实现。
本轮属于派生几何实现优化，不需要新 ADR 或更新上游同步终点。

## 验证与回滚

现有 paragraph-layout-bench 增加 --replay，分别计时构造、析构与核心布局；
保留默认纯布局模式。列表临时标记测量结果不构造 replay，与 demo 一致。
保存改前程序，交替运行改前与改后。完整测试、demo example 测试和 golden
必须通过，禁止更新 golden。检查编辑器与文档格式。
出现行为差异时恢复共享查询与索引构造调用；基准可独立保留。

## 结果

实现完成。普通富文本片段、背景与装饰线复用同一份 positioned clusters；
replay 只生成一次普通片段。glyph 借用索引只收录多 scalar range，
geometry 和 autospace 索引用 entry 保留首次匹配，不依赖哈希遍历顺序。
ruby 几何沿用原算法；其内部自然 advance 查询及其他背景度量扫描未在本轮改写。

相同测量代码按 A/B、B/A、A/B 交替运行，每次 20 轮预热、200 轮测量、三个宽度。
下表是每版三次 median 的中位数，单位 ms：

| 宽度 | replay 改前 | replay 改后 | total 改前 | total 改后 |
| --- | --- | --- | --- | --- |
| 672 | 0.776 | 0.237 | 3.397 | 2.847 |
| 360 | 0.788 | 0.245 | 3.492 | 2.918 |
| 960 | 0.768 | 0.235 | 3.361 | 2.816 |

replay 构造下降约 69%，total 下降约 16%。三次 replay median 的范围，
改前依次为 0.776–0.778、0.788–0.791、0.767–0.769；
改后为 0.233–0.250、0.241–0.256、0.230–0.248。
replay 析构 median 保持约 0.04 ms，未缩减输出数据。
total 包含输入准备、布局、索引构造、统计与析构，不包含 overhang、绘制或 GPU。
未进行 GUI 帧率复测，以上结果不代表完整帧耗时。

完整核心测试 1,369 项、demo example 测试 20 项通过，golden 未更新。
补充同一 span 含背景与下划线的 replay/普通查询等价断言并复测通过。
默认与 --replay 两种入口通过；代码无编辑器错误，diff 与文档风格检查通过。
A/B 后将基准的临时标记结果过滤改为 HashSet，并补充模式提示，两种入口已重新验证；
表格使用这项基准维护之前、两份程序测量代码完全一致的数据。