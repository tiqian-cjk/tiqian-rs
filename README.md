# tiqian-rs

`tiqian-rs` 是 [Tiqian](../tiqian) 中文横排排版核心的 Rust 严格镜像移植。当前使用 Tiqian 的确定性 stub shaping、stub font metrics、`EarlyLayoutFixtures` 和 layout dump golden 验证移植行为。

生产 API 不依赖 Kotlin、Gradle 或 Tiqian checkout。下面的跨仓库 fixture 命令仅用于开发期的移植验证。

## 开发环境

- Rust toolchain（Cargo，edition 2024）
- 可运行 Tiqian Gradle wrapper 的 JDK
- Tiqian checkout；默认路径为与本仓库同级的 `../tiqian`

## Fixture 验证

每次验证只运行一个 Tiqian fixture。脚本会：

1. 调用 Tiqian 的 `:engine:exportLayoutFixture`，从 `EarlyLayoutFixtures` 导出实际输入；
2. 将 JSON 经标准输入传给 Rust 的 deterministic stub runner；
3. 使用 greedy、lookahead、paragraph-dp 三种 breaker 生成 Kotlin 格式的 layout dump；
4. 与 Tiqian 原始 checked-in golden 作字节级比较。

从仓库根目录运行一个已验证 fixture：

```shell
bash tools/verify-fixture.sh basic-pause-stop
```

预期末行输出：

```text
fixture basic-pause-stop: golden matched
```

迭代 01 已验证的 fixture：

```shell
for fixture in \
  basic-pause-stop \
  mandatory-crlf \
  kinsoku-push-in \
  line-end-kinsoku \
  justify-mixed-paragraph
do
  bash tools/verify-fixture.sh "$fixture"
done
```

Tiqian 不在默认相邻路径时，显式指定其根目录：

```shell
TIQIAN_ROOT=/absolute/path/to/tiqian \
  bash tools/verify-fixture.sh basic-pause-stop
```

失败时脚本保留 unified diff，并在标准错误中输出第一处不同的 dump 行。先核对 fixture 输入、breaker 与 stub 参数，再回到对应 Kotlin/Rust 镜像文件定位；不要复制 fixture/golden，也不要修改 Tiqian golden 来掩盖差异。

## 本地检查

Rust 侧常规编译与测试：

```shell
cargo check
cargo test
```

Tiqian 侧的原始 golden 基线可在 Tiqian checkout 中检查：

```shell
./gradlew :engine:jvmTest --tests 'org.tiqian.layout.LayoutDumpGoldenTest'
```
