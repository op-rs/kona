# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2025-03-21

### ⚙️ Miscellaneous Tasks

- Release 0.3.0
- Release 0.3.0
- Release 0.3.0
- Release 0.3.0
- Release 0.2.0
- Release 0.3.0
- Release 0.2.0
- Release 0.3.0
- Release 0.3.0
- Release 0.3.0
- Release 0.3.0
- Release 0.2.0
- Release 0.2.0
- Release 0.2.0

## [0.3.0] - 2025-03-21

### ⚙️ Miscellaneous Tasks

- Release 0.3.0

## [0.2.0] - 2025-03-21

### 🚀 Features

- *(examples)* Pulls Discovery and P2P Gossip into Examples (#1250)
- *(node)* P2P Wiring (#1246)
- *(node)* P2P Overhaul (#1260)
- *(engine)* Synchronous task queue (#1256)
- *(engine)* Block building task (#1258)
- *(node)* P2P Upgrades (#1271)

### 🐛 Bug Fixes

- *(executor)* Use correct empty `sha256` hash (#1267)
- *(proof)* EIP-2935 walkback fix (#1273)

### ⚙️ Miscellaneous Tasks

- *(node)* Wire in Sync Config (#1249)
- *(node)* Simplify Node CLI (#1251)
- *(node)* P2P Secret Key (#1254)
- Remove B256 Value Parser (#1255)
- *(cli)* Remove CLI Parsers (#1259)
- *(workspace)* Fix udeps check (#1263)
- *(genesis)* Localize Import for Lints (#1265)
- Fixup Benchmark CI Job (#1274)
- *(ci)* Deprecate --all Flag (#1275)
- Cleanup and Dependency Bumps (#1235)
- *(workspace)* Remove `reth` dependency (#1279)
- *(ci)* Bump Monorepo Commit for Operator Fee Tests (#1277)
- Bump Deps before Release (#1288)
- Release 0.2.0

### Refactor

- Clap attribute macros from #[clap(...)] to #[arg(...)] and #[command(...)] in v4.x (#1285)

## [kona-host/v0.1.0-beta.13] - 2025-03-11

### 🚀 Features

- *(node)* P2P CLI Args (#1242)

### ⚙️ Miscellaneous Tasks

- Allow udeps in `-Zbuild-std` lints (#1245)
- *(workspace)* Use versioned `asterisc-builder` + `cannon-builder` images (#1243)

## [kona-host/v0.1.0-beta.12] - 2025-03-11

### 🚀 Features

- *(proof)* EIP-2935 lookback (#1088)
- *(bin-utils)* Add prometheus server initializer (#1100)
- *(node)* Engine Controller (#1136)
- *(node)* Initial orchestration logic (#1166)
- *(registry)* Lookup `Chain` + `RollupConfig` by identifier (#1156)
- *(engine)* Version Providers (#1168)
- *(interop)* Dedup logic for parsing `Log` to `ExecutingMessage` (#1171)
- *(node)* Hook up `RollupNodeBuilder` to CLI (#1179)
- *(book)* Umbrella Crate RFC (#1063)
- *(node)* Derivation actor (#1180)
- *(engine)* Actor + Task Model (#1177)
- *(engine)* FCU Task Updates (#1191)
- *(protocol)* Update `RollupConfig` (#1170)
- *(engine)* Engine Task Cleanup + Insert Payload Task Stub (#1193)
- *(engine)* Insert Task Updates (#1194)
- *(providers-alloy)* Refactor `AlloyChainProvider` (#1203)
- *(providers-alloy)* Refactor `AlloyL2ChainProvider` (#1204)
- *(engine)* Insert New Payload (#1197)
- *(engine)* Wire up Insert Task (#1202)
- *(node)* Add `sync_start` module (#1207)
- *(interop)* Clean up interop validator RPC component (#1172)
- *(node)* Refactor orchestration (#1231)
- *(hardforks)* Isthmus Network Upgrade Transactions (#1080)
- *(node)* P2P Wiring (#1233)

### 🐛 Bug Fixes

- *(genesis)* System Config Tests (#1090)
- *(derive)* Use `SystemConfig` batcher key for DAP (#1106)
- *(derive)* Hardfork Deps (#1151)
- 2021 Edition Fragment Specifier (#1155)
- *(ci)* Cargo Deny Checks (#1163)
- *(engine)* Engine Client (#1169)
- *(protocol)* Use `Prague` blob fee calculation for L1 info tx (#1192)
- *(protocol)* Add optional pectra blob fee schedule fork (#1195)
- *(executor)* Dep on kona-host (#1224)

### ⚙️ Miscellaneous Tasks

- *(ci)* Dependabot Label Update (#1077)
- Crate Shields (#1078)
- *(genesis)* Rename HardForkConfiguration (#1091)
- *(genesis)* Serde Test Types (#1089)
- *(genesis)* Flatten Hardforks in Rollup Config (#1092)
- *(workspace)* Bump MSRV to `1.82` (#1097)
- *(ci)* Split doc lint + doc test jobs
- *(bin)* Split up bin utilities (#1098)
- *(nexus)* Use `kona-bin-utils` (#1099)
- *(workspace)* Adjust build recipes (#1101)
- Cleanup Crate Docs (#1116)
- *(bin)* Rework Node Binary (#1120)
- *(proof-interop)* Adjust `TRANSITION_STATE_MAX_STEPS` (#1144)
- *(rpc)* Remove L2BlockRef (#1140)
- *(book)* Book Cleanup for Node Docs (#1143)
- *(book)* Maili Rename (#1145)
- *(book)* Update Protocol Crate Docs (#1146)
- *(hardforks)* Fix Alloy Reference (#1147)
- *(book)* Cleanup Protocol Docs (#1149)
- *(interop)* Replace Interop Feat Flag (#1150)
- *(workspace)* Bump `rustc` edition to 2024 (#1152)
- *(host)* Replace anyhow with thiserror (#1093)
- *(node)* Move Engine into Crate (#1164)
- *(engine)* Sync Types (#1167)
- *(engine)* Fixup EngineClient (#1173)
- *(workspace)* Updates op-alloy Dependencies (#1174)
- *(engine)* Remove pub mod Visibility Idents (#1175)
- *(workspace)* Move `external` crates to `node` (#1182)
- *(book)* Teeny Update (#1184)
- *(executor)* Fix comments in EIP-2935 syscall module (#1181)
- *(docs)* Update `README.md` (#1186)
- *(registry)* Remove Default Hasher (#1185)
- *(preimage)* Add labels to `README.md` (#1187)
- *(executor)* Add labels to `README.md` (#1188)
- *(host)* Update `README.md` (#1189)
- *(workspace)* Update `README.md` (#1190)
- *(node)* Simplify L1 watcher (#1196)
- Scr updates (#1199)
- Bump op-alloy Deps (#1205)
- Cleanup Other Deps (#1206)
- *(protocol)* RPC Block -> L2BlockInfo (#1176)
- Fix Deny Config (#1212)
- *(node-rpc)* Delete dead code (#1213)
- *(protocol)* Update Sepolia-only fork to activate on L1 blocktime (#1210)
- *(rpc)* Rename `RollupNode` -> `RollupNodeApi`, export (#1215)
- Bump alloy 0.12 (#1208)
- *(genesis)* Update `SystemConfig` ser (#1217)
- *(net)* P2P Rename (#1221)
- Update Dependencies (#1226)
- Codecov Config (#1225)
- *(book)* Small touchups (#1230)
- *(node)* Tracing Macros (#1234)

### Release

- *(maili)* 0.2.9 (#1087)
- Maili crates one last time (#1218)
- Kona-driver (#1229)

## [kona-host/v0.1.0-beta.11] - 2025-02-21

### 🚀 Features

- *(genesis)* Deny Unknown Fields (#1060)

### 🐛 Bug Fixes

- *(registry)* Use `superchain-registry` as a submodule (#1075)
- *(workspace)* Exclude Maili Shadows (#1076)

### ⚙️ Miscellaneous Tasks

- *(workspace)* Foundry Install Target (#1074)

## [kona-host/v0.1.0-beta.10] - 2025-02-21

### 🐛 Bug Fixes

- Maili Shadows (#1071)
- Remove Maili Shadows from Workspace (#1072)

### 📚 Documentation

- Release Guide (#1067)

### ⚙️ Miscellaneous Tasks

- *(ci)* Remove Release Plz (#1068)

## [kona-hardforks-v0.1.0] - 2025-02-21

### 🚀 Features

- *(protocol)* Introduce Hardforks Crate (#1065)

## [kona-nexus-v0.1.0] - 2025-02-20

### 🚀 Features

- *(bin)* Network Component Runner (#1058)

## [kona-rpc-v0.1.0] - 2025-02-20

### 🐛 Bug Fixes

- *(ci)* Submodule Sync Crate Path (#1061)

### ⚙️ Miscellaneous Tasks

- *(book)* Move the Monorepo Doc to Archives (#1062)

### Release

- *(kona-interop)* 0.1.2 (#1066)

## [kona-serde-v0.1.0] - 2025-02-20

### 🚀 Features

- *(client)* Support cannon mips64r1 (#1054)
- *(client)* Wire up `L2PayloadWitness` hint for single-chain proof (#1034)
- Kona Optimism Monorepo (#1055)

### 🐛 Bug Fixes

- Fix type annotations (#1050)
- Exclude kona-net (#1049)
- *(docker)* `mips64` target data layout (#1056)
- *(std-fpvm)* Allow non-const fn with mut ref (#1057)

### ⚙️ Miscellaneous Tasks

- Monorepo Proposal Doc (#1036)
- *(book)* RFC and Archives Section (#1053)

## [kona-net-v0.1.0] - 2025-02-13

### ⚙️ Miscellaneous Tasks

- Bump Dependencies (#1029)
- *(interop)* Remove horizon timestamp (#1028)
- Restructure Kona to be more Extensible (#1031)
- *(host)* Expose private SingleChainHost methods (#1030)
- *(services)* Networking Crate (#1032)

## [kona-host/v0.1.0-beta.9] - 2025-02-11

### 🚀 Features

- Derive Eq/Ord/Hash for (Archived) PreimageKey(Type) (#956)
- Allow 7702 receipts after Isthmus active (#959)
- Fill eip 7702 tx env with auth list (#958)
- *(executor)* EIP-2935 Syscall Support [ISTHMUS] (#963)
- *(executor)* EIP-7002 Syscall Support [ISTHMUS] (#965)
- *(executor)* EIP-7251 Syscall Support [ISTHMUS] (#968)
- *(executor)* Export receipts (#969)
- *(client)* EIP-2537 BLS12-381 Curve Precompile Acceleration (#960)
- *(host)* Interop optimistic block re-execution hint (#983)
- *(proof-interop)* Support multiple `RollupConfigs` in boot routine (#986)
- *(host)* Re-export default CLIs (#992)
- *(proof-sdk)* Cleanup `Hint` API (#998)
- *(proof-sdk)* Optional L2 chain ID in L2-specific hints (#999)
- *(mpt)* Copy-on-hash (#1001)
- *(host)* Reintroduce `L2BlockData` hint (#1003)
- *(client)* Superchain Consolidation (#1004)
- *(ci)* Coverage for action tests (#1005)
- *(host)* Accelerate all BLS12-381 Precompiles (#1010)
- *(executor)* Sort trie keys (#1016)
- *(host)* Proactive hints (#1017)
- *(ci)* Remove support for features after MSRV (#1018)
- *(interop)* Support full timestamp invariant (#1022)
- Isthmus upgrade txs (#1025)

### 🐛 Bug Fixes

- *(executor)* Don't generate a diff when running tests (#967)
- *(executor)* Withdrawals root (#974)
- *(client)* Interop transition rules (#973)
- *(executor)* Removes EIP-7002 and EIP-7251 Pre-block Calls (#990)
- *(ci)* Action tests (#997)
- *(client)* Interop bugfixes (#1006)
- *(client)* No-op sub-transitions in Superchain STF (#1011)
- *(interop)* Check timestamp invariant against executing timestamp AND horizon timestamp (#1024)

### ⚙️ Miscellaneous Tasks

- *(docs)* Add `kailua` to the README (#955)
- Maili 0.1.9 (#964)
- *(executor)* Update SpecId with Isthmus (#962)
- *(executor)* TxEnv Stuffing (#970)
- *(executor)* De-duplicate `TrieAccount` type (#977)
- Dep Updates (#980)
- Update Maili Deps (#978)
- Update Dependencies (#988)
- *(host)* Remove `HostOrchestrator` (#994)
- Bump op-alloy dep (#996)
- *(host)* Refactor fetchers (#995)
- Maili Dependency Update (#1007)
- *(client)* Dedup MSM Required Gas Fn (#1012)
- *(client)* Precompile Run Macro (#1014)
- *(ci)* Bump `codecov-action` to v5 (#1020)
- Use Updated Maili and op-alloy Deps (#1023)
- *(book)* Adherance to devdocs (#1026)
- *(book)* Devdocs subdirectory (#1027)

## [kona-providers-alloy-v0.1.0] - 2025-01-26

### 🚀 Features

- Use empty requests hash when isthmus enabled (#951)
- *(workspace)* Re-introduce `kona-providers-alloy` (#954)

### ⚙️ Miscellaneous Tasks

- *(ci)* Improve docker releases (#952)

## [kona-client-v0.1.0-beta.8] - 2025-01-24

### 🚀 Features

- *(driver)* Multi-block derivation (#888)
- *(host)* Interop proof support (part 1) (#910)
- *(client)* Interop consolidation sub-problem (#913)
- *(host)* Modular components (#915)
- *(executor)* New static test harness (#938)
- *(build)* Migrate to `mips64r2` target for `cannon` (#943)

### 🐛 Bug Fixes

- *(ci)* Codecov (#911)

### ⚙️ Miscellaneous Tasks

- *(mpt)* Remove `anyhow` dev-dependency (#919)
- *(executor)* Remove `anyhow` dev-dependency (#937)

## [kona-proof-v0.2.3] - 2025-01-16

### 🚀 Features

- *(client)* Interop binary (#903)
- *(host)* Support multiple modes (#904)

### ⚙️ Miscellaneous Tasks

- Fix some typos in comment (#906)
- Update Maili Deps (#908)
- Release (#900)

## [kona-proof-interop-v0.1.0] - 2025-01-14

### 🚀 Features

- *(workspace)* `kona-proof-interop` crate (#902)

## [kona-interop-v0.1.0] - 2025-01-13

### 🚀 Features

- *(workspace)* `kona-interop` crate (#899)

## [kona-proof-v0.2.2] - 2025-01-13

### 🐛 Bug Fixes

- Small Spelling Issue (#893)

### 📚 Documentation

- Edited the link in the documentation (#895)

### ⚙️ Miscellaneous Tasks

- Release v0.2.2 (#891)

## [kona-client-v0.1.0-beta.7] - 2025-01-09

### ⚙️ Miscellaneous Tasks

- Remove unused function in OnlineBlobProvider (#875)
- *(derive)* Test Ignoring EIP-7702 (#887)
- Bump Maili (#894)

## [kona-std-fpvm-v0.1.2] - 2025-01-07

### 🐛 Bug Fixes

- Op-rs rename (#883)

### ⚙️ Miscellaneous Tasks

- Isthmus Withdrawals Root (#881)
- Remove redundant words in comment (#882)
- Add emhane as a codeowner (#884)
- Bump Dependencies (#880)
- Release (#885)

## [kona-client-v0.1.0-beta.6] - 2025-01-02

### 🚀 Features

- *(build)* Adjust RV target - `riscv64g` -> `riscv64ima` (#868)
- *(build)* Bump `asterisc-builder` version (#879)

### 🐛 Bug Fixes

- *(derive)* Make tests compile (#878)
- *(derive)* `BatchStream` Past batch handling (#876)

### ⚙️ Miscellaneous Tasks

- Bump alloy 0.8 (#870)

### Tooling

- Make client justfile's commands take an optional rollup_config_path (#869)

## [kona-client-v0.1.0-beta.5] - 2024-12-04

### 🚀 Features

- *(client)* Re-accelerate precompiles (#866)

## [kona-std-fpvm-v0.1.1] - 2024-12-04

### ⚙️ Miscellaneous Tasks

- Release (#837)

## [kona-client-v0.1.0-beta.4] - 2024-12-03

### 🐛 Bug Fixes

- Bump (#855)
- Bump (#865)

### ⚙️ Miscellaneous Tasks

- *(ci)* Distribute `linux/arm64` `kona-fpp` image (#860)
- Bump Other Dependencies (#856)
- Update deps and clean up misc features (#864)

## [kona-client-v0.1.0-beta.3] - 2024-12-02

### 🚀 Features

- *(workspace)* Bump MSRV (#859)

### 🐛 Bug Fixes

- Nightly lint (#858)

## [kona-client-v0.1.0-beta.2] - 2024-11-28

### 🚀 Features

- *(host)* Delete unused blob providers (#842)
- *(driver)* Refines the executor interface for the driver (#850)
- *(client)* Invalidate impossibly old claims (#852)
- *(driver)* Wait for engine (#851)

### 🐛 Bug Fixes

- Use non problematic hashmap fns (#853)

### ⚙️ Miscellaneous Tasks

- *(derive)* Remove indexed blob hash (#847)
- *(driver)* Advance with optional target (#848)
- *(host)* Hint Parsing Cleanup (#844)

## [kona-std-fpvm-v0.1.0] - 2024-11-26

### 🚀 Features

- *(workspace)* Isolate FPVM-specific platform code (#821)

### ⚙️ Miscellaneous Tasks

- *(driver)* Visibility (#834)

## [kona-proof-v0.1.0] - 2024-11-20

### ⚙️ Miscellaneous Tasks

- Minor release' (#833)

## [kona-proof-v0.0.1] - 2024-11-20

### 🚀 Features

- *(driver)* Abstract, Default Pipeline (#796)
- *(driver,client)* Pipeline Cursor Refactor (#798)
- *(mpt)* Extend `TrieProvider` in `kona-executor` (#813)
- *(preimage)* Decouple from `kona-common` (#817)
- *(workspace)* `kona-proof` (#818)

### 🐛 Bug Fixes

- *(client)* SyncStart Refactor (#797)
- Mdbook version (#810)
- *(mpt)* Remove unnused collapse (#808)
- Imports (#829)

### 📚 Documentation

- Update providers.md to use new next method instead of old open_data (#809)
- Fix typo in custom-backend.md (#825)

### ⚙️ Miscellaneous Tasks

- *(ci)* Bump monorepo commit (#805)
- Dispatch book build without cache (#807)
- *(workspace)* Migrate back to `thiserror` v2 (#811)
- *(common)* Rename IO modules (#812)
- *(workspace)* Reorganize SDK (#816)
- V0.6.6 op-alloy (#804)
- *(driver)* Use tracing macros (#822)
- *(driver)* Use tracing macros (#823)
- Op-alloy 0.6.8 (#830)
- *(derive)* Remove batch reader (#826)

## [kona-driver-v0.0.0] - 2024-11-08

### 🚀 Features

- *(driver)* Introduce driver crate (#794)

### 🐛 Bug Fixes

- Remove kona-derive-alloy (#789)

### ⚙️ Miscellaneous Tasks

- *(derive)* Re-export types (#790)

## [kona-mpt-v0.0.6] - 2024-11-06

### 🚀 Features

- *(TrieProvider)* Abstract TrieNode retrieval (#787)

### 🐛 Bug Fixes

- *(derive)* Hoist types out of traits (#781)
- *(derive)* Data Availability Provider Abstraction (#782)
- *(derive-alloy)* Test coverage (#785)

### ⚙️ Miscellaneous Tasks

- Clean codecov confiv (#783)
- *(derive)* Pipeline error test coverage (#784)
- Bump alloy deps (#788)
- Release (#753)

## [kona-client-v0.1.0-alpha.7] - 2024-11-05

### 🚀 Features

- *(derive)* Sources docs (#754)
- Flush oracle cache on reorg #724 (#756)
- *(docs)* Derivation Docs (#768)
- *(client)* Remove `anyhow` (#779)
- *(derive)* `From<BlobProviderError> for PipelineErrorKind` (#780)

### 🐛 Bug Fixes

- *(derive-alloy)* Changelog (#752)
- Update monorepo (#761)
- *(derive)* Use signal value updated with system config. (#776)
- *(client)* Trace extension support (#778)

### ⚙️ Miscellaneous Tasks

- *(ci)* Use `gotestsum` for action tests (#751)
- *(derive)* Cleanup Exports (#757)
- *(derive)* Error Exports (#758)
- *(derive)* Touchup kona-derive readme (#762)
- *(derive-alloy)* Docs (#763)
- *(executor)* Rm upstream util (#755)
- *(ci)* Use `PAT_TOKEN` for automated monorepo pin update (#773)
- *(workspace)* Bump `asterisc` version (#774)
- *(ci)* Update monorepo pin to include Holocene action tests (#775)

## [kona-mpt-v0.0.5] - 2024-10-29

### 🚀 Features

- *(derive)* Remove metrics (#743)
- Update op-alloy (#745)
- *(derive)* Use upstream op-alloy batch types (#746)

### 🐛 Bug Fixes

- Tracing_subscriber problem in `kona-derive` tests (#741)
- *(client)* Don't shadow `executor` in engine retry (#750)

### ⚙️ Miscellaneous Tasks

- *(derive)* Import hygiene (#744)
- *(ci)* Don't run `online` tests in CI (#747)
- *(derive-alloy)* Remove metrics (#748)
- Release (#749)

## [kona-client-v0.1.0-alpha.6] - 2024-10-28

### 🚀 Features

- *(ci)* Bump `go` version for action tests (#730)
- Remove thiserror (#735)
- *(derive)* Sys config accessor (#722)
- *(host)* Remove `MAX_RETRIES` (#739)
- *(host)* Ensure prefetch is falliable (#740)

### 🐛 Bug Fixes

- Hashmap (#732)
- *(derive)* Holocene action tests / fixes (#733)
- Add feature for `alloy-provider`, fix `test_util` (#738)

### ⚙️ Miscellaneous Tasks

- *(workspace)* Update `asterisc` version to `1.0.3-alpha1` (#729)
- Bump op-alloy version (#731)
- Release (#715)
- *(kona-derive-alloy)* Release v0.0.1 (#736)

### Docs

- Update README (#734)

## [kona-client-v0.1.0-alpha.5] - 2024-10-22

### 🚀 Features

- *(derive)* BatchQueue Update [Holocene] (#601)
- *(derive)* Add `Signal` API (#611)
- *(derive)* Holocene flush signal (#612)
- Frame queue tests (#613)
- *(client)* Pass flush signal (#615)
- *(executor)* Use EIP-1559 parameters from payload attributes (#616)
- *(trusted-sync)* Holocene flush (#617)
- *(primitives)* Blob Test Coverage (#627)
- *(executor)* Update EIP-1559 configurability (#648)
- Codecov Shield (#652)
- Codecov sources (#657)
- Use derive more display (#675)
- *(derive)* `Past` batch validity variant (#684)
- *(derive)* Stage multiplexer (#693)
- *(derive)* Signal receiver logic (#696)
- *(derive)* Add `ChannelAssembler` size limitation (#700)
- Codecov bump threshold to 90 (#674)
- *(executor)* EIP-1559 configurability spec updates (#716)
- *(derive)* `BatchValidator` stage (#703)
- *(workspace)* Distribute pipeline, not providers (#717)
- *(executor)* Clean ups (#719)
- Frame queue test asserter (#619)
- *(derive)* Hoist stage traits (#723)
- *(derive)* `BatchProvider` multiplexed stage (#726)
- *(docker)* Update asterisc reproducible build image (#728)

### 🐛 Bug Fixes

- *(ci)* Action tests (#608)
- *(executor)* Holocene EIP-1559 params in Header (#622)
- *(codecov)* Ignore Test Utilities (#628)
- Add codeowners (#635)
- *(providers)* Remove slot derivation (#636)
- *(derive)* Remove unused online mod (#637)
- Codecov (#656)
- *(derive)* Retain L1 blocks (#683)
- Typos (#690)
- *(derive)* Holocene `SpanBatch` prefix checks (#688)
- *(derive)* SpanBatch element limit + channel RLP size limit (#692)
- *(mpt)* Empty root node case (#705)
- *(providers-alloy)* Recycle Beacon Types (#713)

### ⚙️ Miscellaneous Tasks

- Doc logos (#609)
- Delete `trusted-sync` (#621)
- Refactor test providers (#623)
- Add Test Coverage (#625)
- Test coverage for common (#629)
- *(derive)* Blob Source Test Coverage (#631)
- *(providers)* Codecov Ignore Alloy-backed Providers (#633)
- *(preimage)* Test Coverage (#634)
- Update deps (#610)
- *(derive)* Single Batch Test Coverage (#643)
- *(derive)* Pipeline Core Test Coverage (#642)
- *(providers-alloy)* Blob provider fallback tests (#644)
- *(mpt)* Account conversion tests (#647)
- *(mpt)* Mpt noop trait impls (#649)
- *(providers)* Add changelog (#653)
- *(derive)* Hoist attributes queue test utils (#654)
- *(mpt)* Codecov (#655)
- *(derive)* Test channel bank reset (#658)
- *(derive)* Test channel reader resets (#660)
- *(derive)* Adds more channel bank coverage (#659)
- *(derive)* Test channel reader flushing (#661)
- *(executor)* Test Coverage over Executor Utilities (#650)
- *(derive)* Batch Timestamp Tests (#664)
- *(client)* Improve `BootInfo` field names (#665)
- *(host)* Improve CLI flag naming (#666)
- *(derive)* Test Stage Resets and Flushes (#669)
- *(derive)* Test and Clean Batch Types (#670)
- *(ci)* Reduce monorepo auto-update frequency (#671)
- *(host)* Support environment variables for `kona-host` flags (#667)
- *(executor)* Use Upstreamed op-alloy Methods  (#651)
- *(derive)* Stage coverage (#673)
- *(executor)* Cover Builder (#676)
- *(executor)* Move todo to issue: (#680)
- *(derive)* Remove span batch todo comments (#682)
- *(providers-alloy)* Changelog (#685)
- Remove todos (#687)
- *(host)* Reduce disk<->mem KV proptest runs (#689)
- *(workspace)* Update dependencies + fix build (#702)
- *(derive)* Add tracing to `ChannelAssembler` (#701)
- *(workspace)* Removes Primitives (#638)
- Remove version types (#707)
- Hoist trait test utilities (#708)
- Erradicate anyhow (#712)
- Re-org imports (#711)
- Bump alloy dep minor (#718)

## [kona-providers-v0.0.1] - 2024-10-02

### 🚀 Features

- Large dependency update (#528)
- *(primitives)* Remove Attributes (#529)
- *(host)* Exit with client status in native mode (#530)
- *(workspace)* Action test runner (#531)
- *(ci)* Add action tests to CI (#533)
- Remove crates.io patch (#537)
- *(derive)* Typed error handling (#540)
- *(mpt)* Migrate to `thiserror` (#541)
- *(preimage/common)* Migrate to `thiserror` (#543)
- *(executor)* Migrate to `thiserror` (#544)
- *(book)* Custom backend, `kona-executor` extensions, and FPVM backend (#552)
- Remove L2 Execution Payload (#542)
- *(derive)* Latest BN (#521)
- *(derive)* Touchup Docs (#555)
- *(derive)* Hoist AttributesBuilder (#571)
- *(derive)* New BatchStream Stage for Holocene (#566)
- *(derive)* Wire up the batch span stage (#567)
- *(derive)* Holocene Activation (#574)
- *(derive)* Holocene Frame Queue (#579)
- *(derive)* Holocene Channel Bank Checks (#572)
- *(derive)* Holocene Buffer Flushing (#575)
- *(ci)* Split online/offline tests (#582)
- *(derive)* Interleaved channel tests (#585)
- *(derive)* Refactor out Online Providers (#569)
- *(derive)* BatchStreamProvider (#591)
- *(derive)* `BatchStream` buffering (#590)
- *(derive)* Span batch prefix checks (#592)
- Kona-providers (#596)
- Monorepo Pin Update (#604)
- *(derive)* Bump op-alloy dep (#605)

### 🐛 Bug Fixes

- *(derive)* Sequence window expiry (#532)
- *(preimage)* Improve error differentiation in preimage servers (#535)
- *(client)* Channel reader error handling (#539)
- *(client)* Continue derivation on execution failure (#545)
- *(derive)* Move attributes builder trait (#570)
- *(workspace)* Hoist and fix lints (#577)
- Derive pipeline params (#587)

### ⚙️ Miscellaneous Tasks

- *(host)* Make `l2-chain-id` optional if a rollup config was passed. (#534)
- *(host)* Clean up CLI (#538)
- *(workspace)* Bump MSRV to `1.81` (#546)
- *(ci)* Delete program diff job (#547)
- *(workspace)* Allow stdlib in `cfg(test)` (#548)
- *(workspace)* Bump dependencies (#550)
- *(readme)* Remove `kona-plasma` link (#551)
- Channel reader docs (#568)
- *(workspace)* `just lint` (#584)
- *(derive)* [Holocene] Drain previous channel in one iteration (#583)
- Use alloy primitives map (#586)
- *(ci)* Pin action tests monorepo rev (#603)

## [kona-client-v0.1.0-alpha.3] - 2024-09-10

### 🚀 Features

- *(host)* Add `TryFrom<DiskKeyValueStore>` for `MemoryKeyValueStore` (#512)
- Expose store (#513)
- *(ci)* Release prestate build image (#523)

### ⚙️ Miscellaneous Tasks

- *(primitives)* Rm RawTransaction (#505)
- Bumps Dependency Versions (#520)
- *(release)* Default to `amd64` platform on prestate artifacts build (#519)

## [kona-client-v0.1.0-alpha.2] - 2024-09-06

### 🚀 Features

- *(host)* Use `RocksDB` as the disk K/V store (#471)
- *(primitives)* Reuse op-alloy-protocol channel and block types (#499)

### 🐛 Bug Fixes

- *(primitives)* Re-use op-alloy frame type (#492)
- *(mpt)* Empty list walker (#493)
- *(ci)* Remove `PAT_TOKEN` ref (#494)
- *(primitives)* Use consensus hardforks (#497)

### ⚙️ Miscellaneous Tasks

- *(docker)* Update prestate builder image (#502)

## [kona-primitives-v0.0.2] - 2024-09-04

### 🚀 Features

- Increase granularity (#365)
- *(examples)* Log payload attributes on error (#371)
- *(examples)* Add metric for latest l2 reference safe head update (#375)
- *(trusted-sync)* Re-org walkback (#379)
- *(client)* Providers generic over oracles (#336)
- Add zkvm target for io (#394)
- *(derive+trusted-sync)* Online blob provider with fallback (#410)
- *(client)* Generic DerivationDriver over any BlobProvider (#412)
- *(ci)* Add scheduled FPP differential tests (#408)
- *(kdn)* Derivation Test Runner for kona-derive (#414)
- *(client+host)* Dynamic `RollupConfig` in bootloader (#439)
- *(kt)* `kdn` -> `kt`, prep for multiple test formats (#445)
- *(client)* Export `CachingOracle` (#455)
- *(primitives)* `serde` for `L1BlockInfoTx` (#460)
- Update superchain registry deps (#463)
- *(workspace)* Workspace Re-exports (#468)
- *(executor)* Expose full revm Handler (#475)
- *(client)* Granite `ecPairing` precompile limit (#479)
- Run cargo hack against workspace (#485)

### 🐛 Bug Fixes

- Trusted-sync metrics url (#363)
- Docker image metrics url set (#364)
- *(examples)* L2 safe head tracking (#373)
- *(examples)* Reduce Origin Advance to Warn (#372)
- *(actions)* Trusted sync docker publish (#376)
- Drift reset (#381)
- Drift Walkback (#382)
- *(derive)* Pipeline Reset (#383)
- Bubble up validation errors (#388)
- Pin two dependencies due to upstream semver issues (#391)
- Don't hold onto intermediate execution cache across block boundaries (#396)
- *(kona-derive)* Remove SignedRecoverable Shim (#400)
- *(deps)* Bump Alloy Dependencies (#409)
- Remove data iter option (#405)
- *(examples)* Backoff trusted-sync invalid payload retries (#411)
- *(trusted-sync)* Remove Panics (#413)
- *(kona-host)* Set explicit types (#421)
- *(derive)* Granite Hardfork Support (#420)
- *(host)* Backoff after `MAX_RETRIES` (#429)
- Fix superchain registry + primitives versions (#425)
- Broken link in readme (#432)
- Link to section (#419)
- *(kdn)* Update with Repository Rename (#441)
- *(kdn)* Updates `kdn` with op-test-vectors Generic Typing (#444)
- *(client)* Bootinfo serde (#448)
- *(derive)* Remove fpvm tests (#447)
- *(workspace)* Add Unused Dependency Lint (#453)
- Downgrade for release plz (#458)
- *(workspace)* Use published `revm` version (#459)
- *(client)* Walkback Channel Timeout (#456)
- *(client)* Break when the pipeline cannot advance (#478)
- Deprecate --all (#484)
- *(host)* Insert empty MPT root hash (#483)
- *(examples)* Revm Features (#482)

### 🧪 Testing

- *(derive)* Channel timeout (#437)

### ⚙️ Miscellaneous Tasks

- *(derive)* Refine channel frame count buckets (#378)
- *(common)* Remove need for cursors in `NativeIO` (#416)
- *(examples)* Add logs to trusted-sync (#415)
- *(derive)* Remove previous stage trait (#423)
- *(workspace)* Remove `minimal` and `simple-revm` examples (#430)
- *(client)* Ensure p256 precompile activation (#431)
- *(client)* Isolate FPVM-specific constructs (#435)
- *(common-proc)* Suppress doc warning (#436)
- *(host)* Remove TODOs (#438)
- Bump scr version (#440)
- *(workspace)* Remove `kona-plasma` (#443)
- Refactor types out of kona-derive (#454)
- *(bin)* Remove `kt` (#461)
- *(derive)* Remove udeps (#462)
- *(derive)* Reset docs (#464)
- *(workspace)* Reorg Workspace Manifest (#465)
- *(workspace)* Hoist Dependencies (#466)
- *(workspace)* Update for `anton-rs` org transfer (#474)
- *(workspace)* Fix `default-features` in workspace root (#472)
- *(workspace)* Alloy Version Bumps (#467)
- *(ci)* Configure codecov patch job (#477)
- Release (#476)

## [kona-client-v0.1.0-alpha.1] - 2024-07-09

### 🚀 Features

- *(examples)* Trusted Sync Metrics (#308)
- *(derive)* Stage Level Metrics (#309)
- *(build)* Dockerize trusted-sync (#299)
- *(examples)* Pipeline step metrics (#320)
- *(examples)* Send Logs to Loki (#321)
- *(derive)* Granular Provider Metrics (#325)
- *(derive)* More stage metrics (#326)
- *(derive)* Track the current channel size (#331)
- *(derive)* Histogram for number of channels for given frame counts (#337)
- *(executor)* Builder pattern for `StatelessL2BlockExecutor` (#339)
- *(executor)* Generic precompile overrides (#340)
- *(client)* `ecrecover` accelerated precompile (#342)
- *(client)* `ecpairing` accelerated precompile (#343)
- *(client)* KZG point evaluation accelerated precompile (#344)
- *(executor)* `StatelessL2BlockExecutor` benchmarks (#350)
- *(docker)* Reproducible `asterisc` prestate (#357)
- *(ci)* Run Host + Client natively in offline mode (#355)
- *(mpt)* `TrieNode` benchmarks (#351)
- *(ci)* Build benchmarks in CI (#352)

### 🐛 Bug Fixes

- Publish trusted-sync to GHCR (#312)
- *(ci)* Publish trusted sync docker (#314)
- *(derive)* Warnings with metrics macro (#322)
- *(examples)* Small cli fix (#323)
- *(examples)* Don't panic on validation fetch failure (#327)
- *(derive)* Prefix all metric names (#330)
- *(derive)* Bind the Pipeline trait to Iterator (#334)
- *(examples)* Reset Failed Payload Derivation Metric (#338)
- *(examples)* Justfile fixes (#341)
- *(derive)* Unused var w/o `metrics` feature (#345)
- *(examples)* Dockerfile fixes (#347)
- *(examples)* Start N Blocks Back from Tip (#349)

### ⚙️ Miscellaneous Tasks

- *(client)* Improve justfile (#305)
- *(derive)* Add targets to stage logs (#310)
- *(docs)* Label Cleanup (#307)
- Bump `superchain-registry` version (#306)
- *(derive)* Remove noisy batch logs (#329)
- *(preimage)* Remove dynamic dispatch (#354)
- *(host)* Make `exec` flag optional (#356)
- *(docker)* Pin `asterisc-builder` version in reproducible prestate builder (#362)

## [kona-primitives-v0.0.1] - 2024-06-22

### ⚙️ Miscellaneous Tasks

- Pin op-alloy-consensus (#304)

## [kona-common-v0.0.2] - 2024-06-22

### 🚀 Features

- *(precompile)* Add `precompile` key type (#179)
- *(preimage)* Async server components (#183)
- *(host)* Host program scaffold (#184)
- *(host)* Disk backed KV store (#185)
- *(workspace)* Add aliases in root `justfile` (#191)
- *(host)* Add local key value store (#189)
- *(preimage)* Async client handles (#200)
- *(mpt)* Trie node insertion (#195)
- *(mpt)* Trie DB commit (#196)
- *(mpt)* Simplify `TrieDB` (#198)
- *(book)* Add minimal program stage documentation (#202)
- *(mpt)* Block hash walkback (#199)
- Refactor reset provider (#207)
- Refactor the pipeline builder (#209)
- *(client)* `BootInfo` (#205)
- Minimal ResetProvider Implementation (#208)
- Pipeline Builder (#217)
- *(client)* `StatelessL2BlockExecutor` (#210)
- *(ci)* Add codecov (#233)
- *(ci)* Dependabot config (#236)
- *(kona-derive)* Updated interface (#230)
- *(client)* Add `current_output_root` to block executor (#225)
- *(client)* Account + Account storage hinting in `TrieDB` (#228)
- *(plasma)* Online Plasma Input Fetcher (#167)
- *(host)* More hint routes (#232)
- *(kona-derive)* Towards Derivation (#243)
- *(client)* Add `RollupConfig` to `BootInfo` (#251)
- *(client)* Oracle-backed derive traits (#252)
- *(client/host)* Oracle-backed Blob fetcher (#255)
- *(client)* Derivation integration (#257)
- *(preimage)* Add serde feature flag to preimage crate for keys (#271)
- *(fjord)* Fjord parameter changes (#284)

### 🐛 Bug Fixes

- *(ci)* Run CI on `pull_request` and `merge_group` triggers (#186)
- *(primitives)* Use decode_2718() to gracefully handle the tx type (#182)
- Strong Error Typing (#187)
- *(readme)* CI badges (#190)
- *(host)* Blocking native client program (#201)
- *(derive)* Alloy EIP4844 Blob Type (#215)
- Derivation Pipeline (#220)
- Use 2718 encoding (#231)
- *(ci)* Do not run coverage in merge queue (#239)
- *(kona-derive)* Reuse upstream reqwest provider (#229)
- Output root version to 32 bytes (#248)
- *(examples)* Clean up trusted sync logging (#263)
- Type re-exports (#280)
- *(common)* Pipe IO support (#282)
- *(examples)* Dynamic Rollup Config Loading (#293)
- Example dep feature (#297)
- *(derive)* Fjord brotli decompression (#298)
- *(mpt)* Fix extension node truncation (#300)

### ⚙️ Miscellaneous Tasks

- *(common)* Use `Box::leak` rather than `mem::forget` (#180)
- *(derive)* Data source unit tests (#181)
- *(ci)* Workflow trigger changes (#203)
- *(mpt)* Do not expose recursion vars (#197)
- Use alloy withdrawal type (#213)
- *(host)* Simplify host program (#206)
- Update README (#227)
- *(kona-derive)* Online Pipeline Cleanup (#241)
- *(workspace)* `kona-executor` (#259)
- *(derive)* Sources Touchups (#266)
- *(derive)* Online module touchups (#265)
- *(derive)* Cleanup pipeline tracing (#264)
- Update `README.md` (#269)
- Re-export input types (#279)
- *(client)* Add justfile for running client program (#283)
- *(ci)* Remove codecov from binaries (#285)
- Payload decoding tests (#289)
- Payload decoding tests (#287)
- *(workspace)* Reorganize binary example programs (#294)
- Version dependencies (#296)
- *(workspace)* Prep release (#301)
- Release (#302)

## [kona-common-proc-v0.0.1] - 2024-05-23

### 🚀 Features

- *(primitives)* Kona-derive type refactor (#135)
- *(derive)* Pipeline Builder (#127)
- *(plasma)* Implements Plasma Support for kona derive (#152)
- *(derive)* Online Data Source Factory Wiring (#150)
- *(derive)* Abstract Alt DA out of `kona-derive` (#156)
- *(derive)* Return the concrete online attributes queue type from the online stack constructor (#158)
- *(primitives)* Move attributes into primitives (#163)
- *(mpt)* Refactor `TrieNode` (#172)
- *(mpt)* `TrieNode` retrieval (#173)
- *(mpt)* `TrieCacheDB` scaffold (#174)
- *(workspace)* Client programs in workspace (#178)

### 🐛 Bug Fixes

- *(workspace)* Release plz (#138)
- *(derive)* Small Fixes and Span Batch Validation Fix (#139)
- *(derive)* Move span batch conversion to try from trait (#142)
- *(ci)* Release plz (#145)
- *(derive)* Remove unnecessary online feature decorator (#160)
- *(plasma)* Reduce plasma source generic verbosity (#165)
- *(plasma)* Plasma Data Source Cleanup (#164)
- *(derive)* Ethereum Data Source (#159)
- *(derive)* Fix span batch utils read_tx_data() (#170)
- *(derive)* Inline blob verification into the blob provider (#175)

### ⚙️ Miscellaneous Tasks

- *(workspace)* Exclude all crates except `kona-common` from cannon/asterisc lint job (#168)
- *(host)* Split CLI utilities out from binary (#169)

## [kona-mpt-v0.0.1] - 2024-04-24

### 🚀 Features

- L1 traversal (#39)
- Add `TxDeposit` type (#40)
- Add OP receipt fields (#41)
- System config update event parsing (#42)
- L1 retrieval (#44)
- Frame queue stage (#45)
- *(derive)* Channel bank (#46)
- Single batch type (#43)
- Data sources
- Clean up data sources to use concrete bytes type
- Async iterator and cleanup
- Fix async iterator issue
- *(derive)* Most of blob data source impl
- *(derive)* Fill blob pointers
- *(derive)* Blob decoding
- *(derive)* Test Utilities (#62)
- *(derive)* Share the rollup config across stages using an arc
- *(ci)* Add workflow to cycle issues (#73)
- *(derive)* Channel Reader Implementation (#65)
- *(types)* Span batches
- *(derive)* Raw span type refactoring
- *(derive)* Fixed bytes and encoding
- *(derive)* Refactor serialization; `SpanBatchPayload` WIP
- *(derive)* Derive raw batches, mocks
- *(derive)* `add_txs` function
- *(derive)* Reorganize modules
- *(derive)* `SpanBatch` type implementation WIP
- *(workspace)* Add `rustfmt.toml`
- *(derive)* Initial pass at telemetry
- *(derive)* Add signature protection check in `SpanBatchTransactions`
- *(derive)* Batch type for the channel reader
- *(derive)* Channel reader implementation with batch reader
- *(derive)* Batch queue
- *(derive)* Basic batch queue next batch derivation
- *(derive)* Finish up batch derivation
- *(derive)* Attributes queue stage
- *(derive)* Add next_attributes test
- *(derive)* Use upstream alloy (#89)
- *(derive)* Add `ecrecover` trait + features (#90)
- *(derive)* Batch Queue Logging (#86)
- *(common)* Move from `RegisterSize` to native ptr size type (#95)
- *(preimage)* `OracleServer` + `HintReader` (#96)
- *(derive)* Move to `tracing` for telemetry (#94)
- *(derive)* Online `ChainProvider` (#93)
- *(derive)* Payload Attribute Building (#92)
- *(derive)* Add `L1BlockInfoTx` (#100)
- *(derive)* `L2ChainProvider` w/ `op-alloy-consensus` (#98)
- *(derive)* Build `L1BlockInfoTx` in payload builder (#102)
- *(derive)* Deposit derivation testing (#115)
- *(derive)* Payload builder tests (#106)
- *(derive)* Online Blob Provider (#117)
- *(derive)* Use `L2ChainProvider` for system config fetching in attributes builder (#123)
- *(derive)* Span Batch Validation (#121)
- `kona-mpt` crate (#128)

### 🐛 Bug Fixes

- *(derive)* Small l1 retrieval doc comment fix (#61)
- *(derive)* Review cleanup
- *(derive)* Vec deque
- *(derive)* Remove k256 feature
- *(derive)* Async iterator type with data sources
- Result wrapping iterator item
- *(derive)* More types
- *(derive)* Span type encodings and decodings
- *(derive)* Span batch tx rlp
- *(derive)* Bitlist alignment
- *(derive)* Refactor span batch tx types
- *(derive)* Refactor tx enveloped
- *(derive)* Data sources upstream conflicts
- *(derive)* Hoist params from types
- *(derive)* Formatting
- *(derive)* Batch type lints
- *(derive)* Channel bank impl
- *(derive)* Channel reader lints
- *(derive)* Single batch validation
- *(derive)* Merge upstream changes
- *(derive)* Rebase
- *(derive)* Frame queue error bubbling and docs
- *(derive)* Clean up frame queue docs
- *(derive)* L1 retrieval docs (#80)
- *(derive)* Frame Queue Error Bubbling and Docs (#82)
- *(derive)* Fix bricked arc stage param construction (#84)
- *(derive)* Merge upstream changes
- *(derive)* Hoist params
- *(derive)* Upstream merge
- *(derive)* Clean up the channel bank and add tests
- *(derive)* Channel bank tests
- *(derive)* Channel bank testing with spinlocked primitives
- *(derive)* Rebase
- *(derive)* Omit the engine queue stage
- *(derive)* Attributes queue
- *(derive)* Rework abstractions and attributes queue testing
- *(derive)* Error equality fixes and tests
- *(derive)* Successful payload attributes building tests
- *(derive)* Extend attributes queue unit test
- *(derive)* Impl origin provider trait across stages
- *(derive)* Lints
- *(derive)* Add back removed test
- *(derive)* Stage Decoupling (#88)
- *(derive)* Derive full `SpanBatch` in channel reader (#97)
- *(derive)* Doc Touchups and Telemetry (#105)
- *(readme)* Remove blue highlights (#116)
- *(derive)* Span batch bitlist encoding (#122)
- *(derive)* Rebase span batch validation tests (#125)
- *(workspace)* Release plz (#137)

### ⚙️ Miscellaneous Tasks

- Scaffold (#37)
- *(derive)* Clean up RLP encoding + use `TxType` rather than ints
- *(derive)* Rebase + move `alloy` module
- *(derive)* Channel reader tests + fixes, batch type fixes
- *(derive)* L1Traversal Doc and Test Cleanup (#79)
- *(derive)* Cleanups (#91)
- *(workspace)* Cleanup justfiles (#104)
- *(ci)* Fail CI on doclint failure (#101)
- *(workspace)* Move `alloy-primitives` to workspace dependencies (#103)

### Dependabot

- Upgrade mio (#63)

### Wip

- *(derive)* `RawSpanBatch` diff decoding/encoding test

## [kona-preimage-v0.0.1] - 2024-02-22

### 🐛 Bug Fixes

- Specify common version (#32)

## [kona-common-v0.0.1] - 2024-02-22

### 🚀 Features

- `release-plz` release pipeline (#27)
- `release-plz` release pipeline (#29)

<!-- generated by git-cliff -->
