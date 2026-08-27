# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2026-08-28)

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

 - 23 commits contributed to the release over the course of 33 calendar days.
 - 20 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
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

