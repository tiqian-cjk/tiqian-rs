# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.2.0 (2026-09-17)

### Chore

 - <csr-id-29fd8374983c26d351698e519b783cb37d5ecc30/> adjust folder structure of tests
 - <csr-id-211969777a0b417293afd604336fa704e15d5790/> adjust folder structure
 - <csr-id-403648fc60e7d37deefdbad05e00779964a5d9fd/> move fixture-layout-dump to example
 - <csr-id-2024b725fc82bb3c5b69a3b7bd887120501a1794/> measure paragraph demo layout time
 - <csr-id-56631b631edffa529fca00d39b3f21da54cf5aab/> clippy
 - <csr-id-cec61ff6bb49e5235e2defe622c6b79163453279/> format

### Documentation

 - <csr-id-7977e18ba51f4b1db901ea45eed1edf1af461834/> update readme
 - <csr-id-fbe5eaeaa285f33a03f06f4f5259d2cec1399fe2/> update unified font backend integration guidance
 - <csr-id-874297031a7a10344b4dccc180ef1ef84d1e8451/> add project overview and development guide
 - <csr-id-21f3b29bdb7c7cecb734aaded7e5a2a6a0a4ef43/> clarify interaction query divergence wording
 - <csr-id-fe2798778825641b302b72203e5eeb530cf83289/> standardize iteration naming and update references
 - <csr-id-bed2d4036597df7079843c1878dd573df74eda37/> clarify API design report and builder integration
 - <csr-id-2a29658f367bf0719c2aff5c1dc86b482ce28cad/> consolidate layout performance optimization notes
 - <csr-id-5cdc0a34226d29b732c848d5a27961823d5fd505/> update docs
 - <csr-id-36edff56044a4cad42d9fad4116f53a4a5fbbd48/> document rich-text output through LayoutResult
 - <csr-id-d0b0467d52e52346c864678b3897dbae646807a1/> document unified rich-text paint model
 - <csr-id-6f4f18caf6f7a3822c8bfb3798f76be628a1699f/> add paragraph builder API design and usage guide
 - <csr-id-d5543fb9d76dfd4188c15e0d50a69c6926a32e82/> finalize Unicode scalar source coordinate migration records
 - <csr-id-b3986c31b7c0a10b11a4e9746b5a469ec071d283/> update docs
 - <csr-id-ba9de1fc186a25247de863a491297d6999c9518c/> update docs
 - <csr-id-0ae1c5e88f39973b4154e03ef6781236d8e5629f/> mark P3 giant-token optimization complete
 - <csr-id-2ef2352eec57a23bcf24d4e7c28f7ad63de76cbb/> document T5 audit completion and remaining P3 performance work
 - <csr-id-060d3e5d6408765fe7d1d1891a1345fb02e6390f/> document Kotlin upstream audit and test mapping
 - <csr-id-5ecb03c00ea081650639a7e22a9bcecad631c394/> document recorded shaping verification and fixture count
 - <csr-id-df07fe1f517bd571b7eaa2cb488f843f15c886cd/> document Unicode data and UTF-16 text migrations
 - <csr-id-8f6afb841cd931e0f224e7fb7ff9600eefd6d541/> document key Kotlin/Rust differences
 - <csr-id-34200e8046e081e5ff67bdfdf1dfa062fd4f0b67/> document large-span sync workflow
 - <csr-id-c74b4410379f87742e0a290d401ca1401c7b6882/> record Kotlin upstream sync by commit range
 - <csr-id-3808ea72f216a01ce6e0d6f21435de4b2be05e4a/> add Kotlin upstream sync tracking records

### New Features

 - <csr-id-0fb6f2048d5f00282a8705110d0f6ad79c743b61/> add layout replay index and paint geometry utilities
 - <csr-id-09399bd8ae02cbd861a743cda82de5980329b1a6/> add layered rich-text paint helpers
 - <csr-id-153c6ef4cb939055c6ab174c09abce19c11b739a/> support layered rich-text spans and semantic markers
 - <csr-id-e9ece62bd89073f5d124bf4a2b8e16e9c20a5288/> expose paragraph builder API and migrate demo content
 - <csr-id-d7e1869f0ab94fdfc80564d9565455bb7ca5d018/> add shaping diagnostics to prepared paragraph JSON
 - <csr-id-08cdf41a8d141b58c548c6d0f62d695617f7d182/> verify recorded fixtures with shaping evidence
 - <csr-id-0ab1bef222e5cc75f9d1aa45f3bec648fc943745/> add optional render evidence to prepared paragraph JSON
 - <csr-id-997569a35d895a19221ac645bc236662235cbdd3/> resolve dash and ellipsis roles from script context
 - <csr-id-64643e1e4e9a24633e58567cdca0657efdd63f46/> support configurable collection implementations
 - <csr-id-7888614dab606dd8b4106b5965e5c8946f65df2d/> add Unicode emoji sequence role promotion
 - <csr-id-48f938bcd96cbab870e293f2218295a714613e7a/> add multilingual paragraph samples

### Bug Fixes

 - <csr-id-e74321e3eef249798a472acfe1e9735482706783/> resolve README package include path
 - <csr-id-b6ef5c1a6a25591fe9139cfa2f029999294559ca/> generate inline boxes for padded rich text
 - <csr-id-a15254964a91a6cbd5af8b93e478a84f6a095bac/> apply technical breaking rules to inline code
 - <csr-id-fa3afcd87d9bb1858154eceb557dd1a1115ce6a3/> emit rich text spans for technical scopes
 - <csr-id-08ee841c924968e4087a07814815f1fb522939c2/> stabilize hanging suffix assertion output
 - <csr-id-120443c19a7ae8ca5ed98b307ab8f3dadbfd1359/> deduplicate paragraph layout preparation
 - <csr-id-227cf682a4f2a462e41d55a5ec2ad4c9a27bc54e/> ignore attached point marks with empty source text
 - <csr-id-0c22a9e21d950fe394d64f092e851371dc71b7e0/> reprocess lines after partial kinsoku push-in
 - <csr-id-de4d0e0779f611cd9613c7a8dab11b53a3db8d03/> remove quotes from punctuation class handling
 - <csr-id-4e211184f94d0f204fb09ebcb5ec84134baf83f8/> classify digit-bound quotes and Latin text correctly
 - <csr-id-c4b09e5407034b22d431876223b5a9e61ee73f53/> handle NaN surplus in compression
 - <csr-id-be17584a4fc4702d6171f95701a8acb2ed7ed399/> exclude fullwidth characters from word-internal quote pairs
 - <csr-id-ad2bc23e69868ff0e607fa82d12b88e066dad642/> exclude numeric boundaries from word-internal quote pairs
 - <csr-id-e82aa51d8a262f2e6d1c1aa46d8a982bdcb33c12/> align CJK dashes with ideograph centers
 - <csr-id-b2f47772af52f8d14deff4fbdefd1676021533cf/> classify non-CJK word-internal quote pairs as Latin text
 - <csr-id-61906cdeb76ee686e047a00b7396f560c2e3cae0/> keep shaping clusters within layout ranges

### Other

 - <csr-id-a6c40095c360e57d41c6d560ab8147bfb4fd368d/> add coverage runner and preserve custom RUSTFLAGS
 - <csr-id-dd82894507083d684d1ac1f86d5d1c3b12dbd3ff/> add Cargo build configuration
 - <csr-id-6f8631f8175d66f65f3b6df549d8e198aec12605/> enable Git LFS checkout
 - <csr-id-8300137b8d2a92518a0196935f7ce9b096fbdaf1/> add GitHub Actions workflow to build and test
 - <csr-id-933891078b7e6c21fb9585d170a2a124d39c56b6/> add GitHub Actions build and test workflow

### Performance

 - <csr-id-520e73561cfd9c88b5586a5d9c067e563a05d3b0/> eliminate repeated progressive tier boundary scans
 - <csr-id-6de2b5762d2a638cf248c8e5bb6bd53b02bb3ad0/> accelerate Lookahead boundary counting with a prefix index
 - <csr-id-aa6d3c868735906eba6816d19661fa989819fc30/> combine progressive density scans and add benchmarks
 - <csr-id-74cded266795900b6ad17ba6f0be891b4d2c6730/> combine hyphen break scans and add benchmark
 - <csr-id-0f5e8e5072e9b4d8c87491a0aa469c54f4af5ed1/> reuse positioned geometry when building replay index
 - <csr-id-f9aa4b2924b33b217755500929faa5395e3179fc/> store metric decision indices instead of cloning
 - <csr-id-fce01fab11d7ba285ee5217be4761d7a476e8049/> reduce layout allocations and add repeatable benchmark
 - <csr-id-7a8492f6bbd64cfafac6e2e59d6fa2af01268d52/> use mimalloc as the global allocator
 - <csr-id-a499bcf337bccc48c2b2d7dd047b1efce3e64f0d/> reuse shared Text views for sliced ranges
 - <csr-id-47b511d767f7e19c9fbddff1b652d57965b3fb35/> optimize UTF-16 indexing and ASCII font classification

### Refactor

 - <csr-id-c4b4c88b50582c0e641b1663d43e112a19e8f7fb/> require explicit font backends for paragraph layout engines
 - <csr-id-e6b297272991c8a28a030adfc6686310efd31d4f/> migrate layout tests to unified font backend
 - <csr-id-4acc4e032783417fdfe46b0039778fb128f759bb/> migrate demos and benchmarks to the unified font backend
 - <csr-id-02d95fbd3afd82e9ffda379cd279c767e84e472d/> unify font resolution, shaping, and metrics behind backend
 - <csr-id-26cd353ce45b102aa1780aad695d3ccac33a790a/> reuse layout replay index in paragraph demo rendering
 - <csr-id-b42acc56e171d61bb74b8d293219ce6596c4482e/> unify rich-text data with layout input
 - <csr-id-14c093beb10389b6468910c56f48b86616c0ab46/> migrate source coordinates from UTF-16 to Unicode scalars
 - <csr-id-abde2acfa6afb1f3cd82d5597a1f917813e02039/> pass borrowed data to justification requests
 - <csr-id-301de9b33ced0b6ea2279f9a94e18a3156c3892e/> index unbreakable ranges and cluster lookups
 - <csr-id-a3e572b55b4dbde8afa77faff249ed9a98f79a9c/> use shared collection type aliases
 - <csr-id-1a1099c1bdc24dbc98da2c5945ba44a6475fc288/> reuse boundary scanning helper and cached text length
 - <csr-id-14f9e8d0fc6efdd475ec34d0486dfa62f7b3588b/> introduce UTF-16-optimized `Text` implemention
 - <csr-id-25c6139eeef72a9c54bb2ee080427804ad4deac7/> replace generated Unicode tables with ICU properties

### Test

 - <csr-id-fe36609907245d8a4693684c9c5c9301fbe62f7c/> fix test
 - <csr-id-e468356a2c3e70940cf71039ca60fba0b7e14966/> expand coverage for layout and text processing contracts
 - <csr-id-920a1d4ba5ceb9a68635987ccc5823e0c4b38d17/> migrate rich-text coverage to layered spans and semantics
 - <csr-id-ca08d0942880f1110af749382ef5f1f9b0a15c89/> add Unicode scalar source coordinate fixture coverage
 - <csr-id-06855beb2d11127d7fe517df81664450cf7d2a6f/> fix tests
 - <csr-id-246b2146954697e2612b4dccac78e91d138b6d13/> migrate fixture verification into Rust
 - <csr-id-df0d2a10c23d13747ed04e441c7195e0fa810e94/> cover recorded shaping and hyphenation behavior
 - <csr-id-9f7aed70c462202a23d321008ef294cd60d570a1/> expand layout, line breaking, and justification coverage
 - <csr-id-e65278e7a82867516462f2594861c6495583fdf5/> add Kotlin test audit inventory tool
 - <csr-id-d834015446d3c16c6c9342dd493b669052354be9/> expand layout, shaping, and font backend coverage
 - <csr-id-46b89c6c8b4b8587377d359be26f65694acc2deb/> expand layout coverage for annotations, punctuation, and line breaking
 - <csr-id-0d53353c97bb6fd7e092f49e1c1d944d3aa0659f/> expand coverage for layout, CLREQ, font, Unicode, and linebreak behavior
 - <csr-id-95317310630489ce22332e7260f4f1dc2068551a/> expand layout and interaction boundary coverage
 - <csr-id-b8ed5d75c646053f7aad0fdf3ca2af4c96586736/> classify fullwidth letter-bounded quotes as CJK punctuation
 - <csr-id-569d5fb463944e408908dbbc090e53da50c20f70/> distinguish letter- and digit-bounded word-internal quotes
 - <csr-id-fbed5db7adc04e0f3170ef372a679ea8a7ac3ab5/> cover mixed-script quote classification and geometry

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 104 commits contributed to the release over the course of 20 calendar days.
 - 20 days passed between releases.
 - 100 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Resolve README package include path ([`e74321e`](https://github.com/tiqian-cjk/tiqian-rs/commit/e74321e3eef249798a472acfe1e9735482706783))
    - Update readme ([`7977e18`](https://github.com/tiqian-cjk/tiqian-rs/commit/7977e18ba51f4b1db901ea45eed1edf1af461834))
    - Fix test ([`fe36609`](https://github.com/tiqian-cjk/tiqian-rs/commit/fe36609907245d8a4693684c9c5c9301fbe62f7c))
    - Require explicit font backends for paragraph layout engines ([`c4b4c88`](https://github.com/tiqian-cjk/tiqian-rs/commit/c4b4c88b50582c0e641b1663d43e112a19e8f7fb))
    - Expand coverage for layout and text processing contracts ([`e468356`](https://github.com/tiqian-cjk/tiqian-rs/commit/e468356a2c3e70940cf71039ca60fba0b7e14966))
    - Update unified font backend integration guidance ([`fbe5eae`](https://github.com/tiqian-cjk/tiqian-rs/commit/fbe5eaeaa285f33a03f06f4f5259d2cec1399fe2))
    - Migrate layout tests to unified font backend ([`e6b2972`](https://github.com/tiqian-cjk/tiqian-rs/commit/e6b297272991c8a28a030adfc6686310efd31d4f))
    - Migrate demos and benchmarks to the unified font backend ([`4acc4e0`](https://github.com/tiqian-cjk/tiqian-rs/commit/4acc4e032783417fdfe46b0039778fb128f759bb))
    - Unify font resolution, shaping, and metrics behind backend ([`02d95fb`](https://github.com/tiqian-cjk/tiqian-rs/commit/02d95fbd3afd82e9ffda379cd279c767e84e472d))
    - Add project overview and development guide ([`8742970`](https://github.com/tiqian-cjk/tiqian-rs/commit/874297031a7a10344b4dccc180ef1ef84d1e8451))
    - Clarify interaction query divergence wording ([`21f3b29`](https://github.com/tiqian-cjk/tiqian-rs/commit/21f3b29bdb7c7cecb734aaded7e5a2a6a0a4ef43))
    - Standardize iteration naming and update references ([`fe27987`](https://github.com/tiqian-cjk/tiqian-rs/commit/fe2798778825641b302b72203e5eeb530cf83289))
    - Clarify API design report and builder integration ([`bed2d40`](https://github.com/tiqian-cjk/tiqian-rs/commit/bed2d4036597df7079843c1878dd573df74eda37))
    - Consolidate layout performance optimization notes ([`2a29658`](https://github.com/tiqian-cjk/tiqian-rs/commit/2a29658f367bf0719c2aff5c1dc86b482ce28cad))
    - Eliminate repeated progressive tier boundary scans ([`520e735`](https://github.com/tiqian-cjk/tiqian-rs/commit/520e73561cfd9c88b5586a5d9c067e563a05d3b0))
    - Accelerate Lookahead boundary counting with a prefix index ([`6de2b57`](https://github.com/tiqian-cjk/tiqian-rs/commit/6de2b5762d2a638cf248c8e5bb6bd53b02bb3ad0))
    - Combine progressive density scans and add benchmarks ([`aa6d3c8`](https://github.com/tiqian-cjk/tiqian-rs/commit/aa6d3c868735906eba6816d19661fa989819fc30))
    - Combine hyphen break scans and add benchmark ([`74cded2`](https://github.com/tiqian-cjk/tiqian-rs/commit/74cded266795900b6ad17ba6f0be891b4d2c6730))
    - Update docs ([`5cdc0a3`](https://github.com/tiqian-cjk/tiqian-rs/commit/5cdc0a34226d29b732c848d5a27961823d5fd505))
    - Reuse positioned geometry when building replay index ([`0f5e8e5`](https://github.com/tiqian-cjk/tiqian-rs/commit/0f5e8e5072e9b4d8c87491a0aa469c54f4af5ed1))
    - Reuse layout replay index in paragraph demo rendering ([`26cd353`](https://github.com/tiqian-cjk/tiqian-rs/commit/26cd353ce45b102aa1780aad695d3ccac33a790a))
    - Add layout replay index and paint geometry utilities ([`0fb6f20`](https://github.com/tiqian-cjk/tiqian-rs/commit/0fb6f2048d5f00282a8705110d0f6ad79c743b61))
    - Store metric decision indices instead of cloning ([`f9aa4b2`](https://github.com/tiqian-cjk/tiqian-rs/commit/f9aa4b2924b33b217755500929faa5395e3179fc))
    - Reduce layout allocations and add repeatable benchmark ([`fce01fa`](https://github.com/tiqian-cjk/tiqian-rs/commit/fce01fab11d7ba285ee5217be4761d7a476e8049))
    - Merge pull request #2 from tiqian-cjk/refactor/unified-rich-text-model ([`3334b28`](https://github.com/tiqian-cjk/tiqian-rs/commit/3334b287c2f749e8dd09090b402322adbfdca7fc))
    - Document rich-text output through LayoutResult ([`36edff5`](https://github.com/tiqian-cjk/tiqian-rs/commit/36edff56044a4cad42d9fad4116f53a4a5fbbd48))
    - Unify rich-text data with layout input ([`b42acc5`](https://github.com/tiqian-cjk/tiqian-rs/commit/b42acc56e171d61bb74b8d293219ce6596c4482e))
    - Document unified rich-text paint model ([`d0b0467`](https://github.com/tiqian-cjk/tiqian-rs/commit/d0b0467d52e52346c864678b3897dbae646807a1))
    - Migrate rich-text coverage to layered spans and semantics ([`920a1d4`](https://github.com/tiqian-cjk/tiqian-rs/commit/920a1d4ba5ceb9a68635987ccc5823e0c4b38d17))
    - Add layered rich-text paint helpers ([`09399bd`](https://github.com/tiqian-cjk/tiqian-rs/commit/09399bd8ae02cbd861a743cda82de5980329b1a6))
    - Support layered rich-text spans and semantic markers ([`153c6ef`](https://github.com/tiqian-cjk/tiqian-rs/commit/153c6ef4cb939055c6ab174c09abce19c11b739a))
    - Generate inline boxes for padded rich text ([`b6ef5c1`](https://github.com/tiqian-cjk/tiqian-rs/commit/b6ef5c1a6a25591fe9139cfa2f029999294559ca))
    - Apply technical breaking rules to inline code ([`a152549`](https://github.com/tiqian-cjk/tiqian-rs/commit/a15254964a91a6cbd5af8b93e478a84f6a095bac))
    - Emit rich text spans for technical scopes ([`fa3afcd`](https://github.com/tiqian-cjk/tiqian-rs/commit/fa3afcd87d9bb1858154eceb557dd1a1115ce6a3))
    - Add paragraph builder API design and usage guide ([`6f4f18c`](https://github.com/tiqian-cjk/tiqian-rs/commit/6f4f18caf6f7a3822c8bfb3798f76be628a1699f))
    - Expose paragraph builder API and migrate demo content ([`e9ece62`](https://github.com/tiqian-cjk/tiqian-rs/commit/e9ece62bd89073f5d124bf4a2b8e16e9c20a5288))
    - Merge pull request #1 from tiqian-cjk/refactor/unicode_scalar_value ([`9f4e04c`](https://github.com/tiqian-cjk/tiqian-rs/commit/9f4e04cc07ed8f03ef5a8ce52db2dbbac7d58927))
    - Finalize Unicode scalar source coordinate migration records ([`d5543fb`](https://github.com/tiqian-cjk/tiqian-rs/commit/d5543fb9d76dfd4188c15e0d50a69c6926a32e82))
    - Add Unicode scalar source coordinate fixture coverage ([`ca08d09`](https://github.com/tiqian-cjk/tiqian-rs/commit/ca08d0942880f1110af749382ef5f1f9b0a15c89))
    - Update docs ([`b3986c3`](https://github.com/tiqian-cjk/tiqian-rs/commit/b3986c31b7c0a10b11a4e9746b5a469ec071d283))
    - Fix tests ([`06855be`](https://github.com/tiqian-cjk/tiqian-rs/commit/06855beb2d11127d7fe517df81664450cf7d2a6f))
    - Migrate source coordinates from UTF-16 to Unicode scalars ([`14c093b`](https://github.com/tiqian-cjk/tiqian-rs/commit/14c093beb10389b6468910c56f48b86616c0ab46))
    - Update docs ([`ba9de1f`](https://github.com/tiqian-cjk/tiqian-rs/commit/ba9de1fc186a25247de863a491297d6999c9518c))
    - Migrate fixture verification into Rust ([`246b214`](https://github.com/tiqian-cjk/tiqian-rs/commit/246b2146954697e2612b4dccac78e91d138b6d13))
    - Migrate fixtures and golden ([`afba945`](https://github.com/tiqian-cjk/tiqian-rs/commit/afba9453aed7db7763fb82d612aa2d0c0be08684))
    - Cover recorded shaping and hyphenation behavior ([`df0d2a1`](https://github.com/tiqian-cjk/tiqian-rs/commit/df0d2a10c23d13747ed04e441c7195e0fa810e94))
    - Add coverage runner and preserve custom RUSTFLAGS ([`a6c4009`](https://github.com/tiqian-cjk/tiqian-rs/commit/a6c40095c360e57d41c6d560ab8147bfb4fd368d))
    - Stabilize hanging suffix assertion output ([`08ee841`](https://github.com/tiqian-cjk/tiqian-rs/commit/08ee841c924968e4087a07814815f1fb522939c2))
    - Add Cargo build configuration ([`dd82894`](https://github.com/tiqian-cjk/tiqian-rs/commit/dd82894507083d684d1ac1f86d5d1c3b12dbd3ff))
    - Mark P3 giant-token optimization complete ([`0ae1c5e`](https://github.com/tiqian-cjk/tiqian-rs/commit/0ae1c5e88f39973b4154e03ef6781236d8e5629f))
    - Pass borrowed data to justification requests ([`abde2ac`](https://github.com/tiqian-cjk/tiqian-rs/commit/abde2acfa6afb1f3cd82d5597a1f917813e02039))
    - Deduplicate paragraph layout preparation ([`120443c`](https://github.com/tiqian-cjk/tiqian-rs/commit/120443c19a7ae8ca5ed98b307ab8f3dadbfd1359))
    - Document T5 audit completion and remaining P3 performance work ([`2ef2352`](https://github.com/tiqian-cjk/tiqian-rs/commit/2ef2352eec57a23bcf24d4e7c28f7ad63de76cbb))
    - Expand layout, line breaking, and justification coverage ([`9f7aed7`](https://github.com/tiqian-cjk/tiqian-rs/commit/9f7aed70c462202a23d321008ef294cd60d570a1))
    - Index unbreakable ranges and cluster lookups ([`301de9b`](https://github.com/tiqian-cjk/tiqian-rs/commit/301de9b33ced0b6ea2279f9a94e18a3156c3892e))
    - Document Kotlin upstream audit and test mapping ([`060d3e5`](https://github.com/tiqian-cjk/tiqian-rs/commit/060d3e5d6408765fe7d1d1891a1345fb02e6390f))
    - Add Kotlin test audit inventory tool ([`e65278e`](https://github.com/tiqian-cjk/tiqian-rs/commit/e65278e7a82867516462f2594861c6495583fdf5))
    - Expand layout, shaping, and font backend coverage ([`d834015`](https://github.com/tiqian-cjk/tiqian-rs/commit/d834015446d3c16c6c9342dd493b669052354be9))
    - Expand layout coverage for annotations, punctuation, and line breaking ([`46b89c6`](https://github.com/tiqian-cjk/tiqian-rs/commit/46b89c6c8b4b8587377d359be26f65694acc2deb))
    - Ignore attached point marks with empty source text ([`227cf68`](https://github.com/tiqian-cjk/tiqian-rs/commit/227cf682a4f2a462e41d55a5ec2ad4c9a27bc54e))
    - Reprocess lines after partial kinsoku push-in ([`0c22a9e`](https://github.com/tiqian-cjk/tiqian-rs/commit/0c22a9e21d950fe394d64f092e851371dc71b7e0))
    - Document recorded shaping verification and fixture count ([`5ecb03c`](https://github.com/tiqian-cjk/tiqian-rs/commit/5ecb03c00ea081650639a7e22a9bcecad631c394))
    - Document Unicode data and UTF-16 text migrations ([`df07fe1`](https://github.com/tiqian-cjk/tiqian-rs/commit/df07fe1f517bd571b7eaa2cb488f843f15c886cd))
    - Document key Kotlin/Rust differences ([`8f6afb8`](https://github.com/tiqian-cjk/tiqian-rs/commit/8f6afb841cd931e0f224e7fb7ff9600eefd6d541))
    - Expand coverage for layout, CLREQ, font, Unicode, and linebreak behavior ([`0d53353`](https://github.com/tiqian-cjk/tiqian-rs/commit/0d53353c97bb6fd7e092f49e1c1d944d3aa0659f))
    - Add shaping diagnostics to prepared paragraph JSON ([`d7e1869`](https://github.com/tiqian-cjk/tiqian-rs/commit/d7e1869f0ab94fdfc80564d9565455bb7ca5d018))
    - Verify recorded fixtures with shaping evidence ([`08cdf41`](https://github.com/tiqian-cjk/tiqian-rs/commit/08cdf41a8d141b58c548c6d0f62d695617f7d182))
    - Remove quotes from punctuation class handling ([`de4d0e0`](https://github.com/tiqian-cjk/tiqian-rs/commit/de4d0e0779f611cd9613c7a8dab11b53a3db8d03))
    - Add optional render evidence to prepared paragraph JSON ([`0ab1bef`](https://github.com/tiqian-cjk/tiqian-rs/commit/0ab1bef222e5cc75f9d1aa45f3bec648fc943745))
    - Classify digit-bound quotes and Latin text correctly ([`4e21118`](https://github.com/tiqian-cjk/tiqian-rs/commit/4e211184f94d0f204fb09ebcb5ec84134baf83f8))
    - Resolve dash and ellipsis roles from script context ([`997569a`](https://github.com/tiqian-cjk/tiqian-rs/commit/997569a35d895a19221ac645bc236662235cbdd3))
    - Document large-span sync workflow ([`34200e8`](https://github.com/tiqian-cjk/tiqian-rs/commit/34200e8046e081e5ff67bdfdf1dfa062fd4f0b67))
    - Record Kotlin upstream sync by commit range ([`c74b441`](https://github.com/tiqian-cjk/tiqian-rs/commit/c74b4410379f87742e0a290d401ca1401c7b6882))
    - Add Kotlin upstream sync tracking records ([`3808ea7`](https://github.com/tiqian-cjk/tiqian-rs/commit/3808ea72f216a01ce6e0d6f21435de4b2be05e4a))
    - Expand layout and interaction boundary coverage ([`9531731`](https://github.com/tiqian-cjk/tiqian-rs/commit/95317310630489ce22332e7260f4f1dc2068551a))
    - Handle NaN surplus in compression ([`c4b09e5`](https://github.com/tiqian-cjk/tiqian-rs/commit/c4b09e5407034b22d431876223b5a9e61ee73f53))
    - Classify fullwidth letter-bounded quotes as CJK punctuation ([`b8ed5d7`](https://github.com/tiqian-cjk/tiqian-rs/commit/b8ed5d75c646053f7aad0fdf3ca2af4c96586736))
    - Exclude fullwidth characters from word-internal quote pairs ([`be17584`](https://github.com/tiqian-cjk/tiqian-rs/commit/be17584a4fc4702d6171f95701a8acb2ed7ed399))
    - Distinguish letter- and digit-bounded word-internal quotes ([`569d5fb`](https://github.com/tiqian-cjk/tiqian-rs/commit/569d5fb463944e408908dbbc090e53da50c20f70))
    - Exclude numeric boundaries from word-internal quote pairs ([`ad2bc23`](https://github.com/tiqian-cjk/tiqian-rs/commit/ad2bc23e69868ff0e607fa82d12b88e066dad642))
    - Cover mixed-script quote classification and geometry ([`fbed5db`](https://github.com/tiqian-cjk/tiqian-rs/commit/fbed5db7adc04e0f3170ef372a679ea8a7ac3ab5))
    - Align CJK dashes with ideograph centers ([`e82aa51`](https://github.com/tiqian-cjk/tiqian-rs/commit/e82aa51d8a262f2e6d1c1aa46d8a982bdcb33c12))
    - Classify non-CJK word-internal quote pairs as Latin text ([`b2f4777`](https://github.com/tiqian-cjk/tiqian-rs/commit/b2f47772af52f8d14deff4fbdefd1676021533cf))
    - Enable Git LFS checkout ([`6f8631f`](https://github.com/tiqian-cjk/tiqian-rs/commit/6f8631f8175d66f65f3b6df549d8e198aec12605))
    - Add GitHub Actions workflow to build and test ([`8300137`](https://github.com/tiqian-cjk/tiqian-rs/commit/8300137b8d2a92518a0196935f7ce9b096fbdaf1))
    - Add GitHub Actions build and test workflow ([`9338910`](https://github.com/tiqian-cjk/tiqian-rs/commit/933891078b7e6c21fb9585d170a2a124d39c56b6))
    - Adjust folder structure of tests ([`29fd837`](https://github.com/tiqian-cjk/tiqian-rs/commit/29fd8374983c26d351698e519b783cb37d5ecc30))
    - Rename files ([`ef434ca`](https://github.com/tiqian-cjk/tiqian-rs/commit/ef434ca5ff2380e9cde7bebc6508fac2feb711ac))
    - Adjust folder structure ([`2119697`](https://github.com/tiqian-cjk/tiqian-rs/commit/211969777a0b417293afd604336fa704e15d5790))
    - Move fixture-layout-dump to example ([`403648f`](https://github.com/tiqian-cjk/tiqian-rs/commit/403648fc60e7d37deefdbad05e00779964a5d9fd))
    - Use mimalloc as the global allocator ([`7a8492f`](https://github.com/tiqian-cjk/tiqian-rs/commit/7a8492f6bbd64cfafac6e2e59d6fa2af01268d52))
    - Use shared collection type aliases ([`a3e572b`](https://github.com/tiqian-cjk/tiqian-rs/commit/a3e572b55b4dbde8afa77faff249ed9a98f79a9c))
    - Support configurable collection implementations ([`64643e1`](https://github.com/tiqian-cjk/tiqian-rs/commit/64643e1e4e9a24633e58567cdca0657efdd63f46))
    - Reuse shared Text views for sliced ranges ([`a499bcf`](https://github.com/tiqian-cjk/tiqian-rs/commit/a499bcf337bccc48c2b2d7dd047b1efce3e64f0d))
    - Reuse boundary scanning helper and cached text length ([`1a1099c`](https://github.com/tiqian-cjk/tiqian-rs/commit/1a1099c1bdc24dbc98da2c5945ba44a6475fc288))
    - Optimize UTF-16 indexing and ASCII font classification ([`47b511d`](https://github.com/tiqian-cjk/tiqian-rs/commit/47b511d767f7e19c9fbddff1b652d57965b3fb35))
    - Introduce UTF-16-optimized `Text` implemention ([`14f9e8d`](https://github.com/tiqian-cjk/tiqian-rs/commit/14f9e8d0fc6efdd475ec34d0486dfa62f7b3588b))
    - Measure paragraph demo layout time ([`2024b72`](https://github.com/tiqian-cjk/tiqian-rs/commit/2024b725fc82bb3c5b69a3b7bd887120501a1794))
    - Clippy ([`56631b6`](https://github.com/tiqian-cjk/tiqian-rs/commit/56631b631edffa529fca00d39b3f21da54cf5aab))
    - Format ([`cec61ff`](https://github.com/tiqian-cjk/tiqian-rs/commit/cec61ff6bb49e5235e2defe622c6b79163453279))
    - Replace generated Unicode tables with ICU properties ([`25c6139`](https://github.com/tiqian-cjk/tiqian-rs/commit/25c6139eeef72a9c54bb2ee080427804ad4deac7))
    - Add Unicode emoji sequence role promotion ([`7888614`](https://github.com/tiqian-cjk/tiqian-rs/commit/7888614dab606dd8b4106b5965e5c8946f65df2d))
    - Add multilingual paragraph samples ([`48f938b`](https://github.com/tiqian-cjk/tiqian-rs/commit/48f938bcd96cbab870e293f2218295a714613e7a))
    - Keep shaping clusters within layout ranges ([`61906cd`](https://github.com/tiqian-cjk/tiqian-rs/commit/61906cdeb76ee686e047a00b7396f560c2e3cae0))
</details>

## v0.1.0 (2026-08-28)

<csr-id-a94b09a38ae71ff52da8d64b9e6e7e15d5b66da8/>
<csr-id-205b80fd6c796bf3ca8b3fdddf7fbb0e5d7b8c3c/>
<csr-id-a1d715e4341898cfdfeef276ab6a96f240482f33/>
<csr-id-5caba540355d4d5741a298db54d6dceb24c5185f/>
<csr-id-89f9c0a7a7109a1e3b7b03f983913cd258eab81c/>
<csr-id-3a862f5b8616dca69939a8bb8774e7b6259c3298/>

### Chore

 - <csr-id-a94b09a38ae71ff52da8d64b9e6e7e15d5b66da8/> include package resources and metadata
 - <csr-id-205b80fd6c796bf3ca8b3fdddf7fbb0e5d7b8c3c/> update repository URL
 - <csr-id-a1d715e4341898cfdfeef276ab6a96f240482f33/> add font resources and Git LFS tracking
 - <csr-id-5caba540355d4d5741a298db54d6dceb24c5185f/> add Mozilla Public License
 - <csr-id-89f9c0a7a7109a1e3b7b03f983913cd258eab81c/> format package manifest

### New Features

 - <csr-id-8f3b66ae3c2da26b0cc336e679b77184a633dfd5/> migrate paragraph demo from CPU to GPU rendering
 - <csr-id-581cd67c2436c976b970c613ad7d523363f03707/> add ligature font samples to paragraph demo
 - <csr-id-df751f9d4f61b52ec6978c050fd9fb065549efa4/> group narrow layout samples under boundary appendix
 - <csr-id-d5ec991fb4f7117485be681e8e16f4a25f6ac296/> add Vello rendering and color emoji support
 - <csr-id-577f75a0448c9eb57e794f84cb8506acd0e7338d/> support explicit font family selection in paragraph demo
 - <csr-id-0b82611d344826e8138b5db44105868d8c6db805/> expand paragraph demo feature coverage
 - <csr-id-e609d8a9e052383eeaaf01ce9e47438c0edf79fc/> add paragraph rendering demo
 - <csr-id-811b6bc3e28f0fa749c2957a31a9436898709e6f/> add all-fixture verification command
 - <csr-id-e66101202aafced6c9f65f270c1fe39b9080adae/> add fixture verification tooling
 - <csr-id-560ceec1c4cbd0816d9290b2cec633a9a5f499dc/> initialize project

### Bug Fixes

 - <csr-id-81e9533476fe079741ad772bb33dfa88793eefe2/> preserve emoji shaping across soft boundaries
 - <csr-id-041461f2bb69f0e50936f1861752576097401a34/> classify emoji using Unicode properties
 - <csr-id-23594057159d5c45e7fc177eae5aa2307fd4896f/> preserve complex emoji graphemes during shaping
 - <csr-id-332ae2408b4929a26e44f518561383da95965f01/> preserve positive zero for empty line widths

### Test

 - <csr-id-3a862f5b8616dca69939a8bb8774e7b6259c3298/> porting unit tests

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 24 commits contributed to the release over the course of 33 calendar days.
 - 20 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release tiqian v0.1.0 ([`ca8196a`](https://github.com/tiqian-cjk/tiqian-rs/commit/ca8196aa40074220c48e415b0d2bc2e7fec14408))
    - Include package resources and metadata ([`a94b09a`](https://github.com/tiqian-cjk/tiqian-rs/commit/a94b09a38ae71ff52da8d64b9e6e7e15d5b66da8))
    - Update repository URL ([`205b80f`](https://github.com/tiqian-cjk/tiqian-rs/commit/205b80fd6c796bf3ca8b3fdddf7fbb0e5d7b8c3c))
    - Porting unit tests ([`3a862f5`](https://github.com/tiqian-cjk/tiqian-rs/commit/3a862f5b8616dca69939a8bb8774e7b6259c3298))
    - Preserve emoji shaping across soft boundaries ([`81e9533`](https://github.com/tiqian-cjk/tiqian-rs/commit/81e9533476fe079741ad772bb33dfa88793eefe2))
    - Migrate paragraph demo from CPU to GPU rendering ([`8f3b66a`](https://github.com/tiqian-cjk/tiqian-rs/commit/8f3b66ae3c2da26b0cc336e679b77184a633dfd5))
    - Add ligature font samples to paragraph demo ([`581cd67`](https://github.com/tiqian-cjk/tiqian-rs/commit/581cd67c2436c976b970c613ad7d523363f03707))
    - Group narrow layout samples under boundary appendix ([`df751f9`](https://github.com/tiqian-cjk/tiqian-rs/commit/df751f9d4f61b52ec6978c050fd9fb065549efa4))
    - Add Vello rendering and color emoji support ([`d5ec991`](https://github.com/tiqian-cjk/tiqian-rs/commit/d5ec991fb4f7117485be681e8e16f4a25f6ac296))
    - Classify emoji using Unicode properties ([`041461f`](https://github.com/tiqian-cjk/tiqian-rs/commit/041461f2bb69f0e50936f1861752576097401a34))
    - Preserve complex emoji graphemes during shaping ([`2359405`](https://github.com/tiqian-cjk/tiqian-rs/commit/23594057159d5c45e7fc177eae5aa2307fd4896f))
    - Support explicit font family selection in paragraph demo ([`577f75a`](https://github.com/tiqian-cjk/tiqian-rs/commit/577f75a0448c9eb57e794f84cb8506acd0e7338d))
    - Expand paragraph demo feature coverage ([`0b82611`](https://github.com/tiqian-cjk/tiqian-rs/commit/0b82611d344826e8138b5db44105868d8c6db805))
    - Add paragraph rendering demo ([`e609d8a`](https://github.com/tiqian-cjk/tiqian-rs/commit/e609d8a9e052383eeaaf01ce9e47438c0edf79fc))
    - Add font resources and Git LFS tracking ([`a1d715e`](https://github.com/tiqian-cjk/tiqian-rs/commit/a1d715e4341898cfdfeef276ab6a96f240482f33))
    - Add Mozilla Public License ([`5caba54`](https://github.com/tiqian-cjk/tiqian-rs/commit/5caba540355d4d5741a298db54d6dceb24c5185f))
    - Format package manifest ([`89f9c0a`](https://github.com/tiqian-cjk/tiqian-rs/commit/89f9c0a7a7109a1e3b7b03f983913cd258eab81c))
    - Clippy ([`09753c5`](https://github.com/tiqian-cjk/tiqian-rs/commit/09753c5239a564cb637588f9ce9e1f4c69eccdf1))
    - Add all-fixture verification command ([`811b6bc`](https://github.com/tiqian-cjk/tiqian-rs/commit/811b6bc3e28f0fa749c2957a31a9436898709e6f))
    - Preserve positive zero for empty line widths ([`332ae24`](https://github.com/tiqian-cjk/tiqian-rs/commit/332ae2408b4929a26e44f518561383da95965f01))
    - Add fixture verification tooling ([`e661012`](https://github.com/tiqian-cjk/tiqian-rs/commit/e66101202aafced6c9f65f270c1fe39b9080adae))
    - Format ([`1bce8c0`](https://github.com/tiqian-cjk/tiqian-rs/commit/1bce8c06cbea168372f188be58979a2ac079d338))
    - Initial version ([`3322457`](https://github.com/tiqian-cjk/tiqian-rs/commit/3322457754d3db8a56a79acfc81b8653b1330c9e))
    - Initialize project ([`560ceec`](https://github.com/tiqian-cjk/tiqian-rs/commit/560ceec1c4cbd0816d9290b2cec633a9a5f499dc))
</details>

