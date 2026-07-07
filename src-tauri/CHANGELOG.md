# Changelog

## [1.0.3](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v1.0.2...chaos-scheduler-tauri-v1.0.3) (2026-07-07)


### Miscellaneous Chores

* **chaos-scheduler-tauri:** Synchronize chaos-scheduler-desktop versions

## [1.0.2](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v1.0.1...chaos-scheduler-tauri-v1.0.2) (2026-07-07)


### Miscellaneous Chores

* **chaos-scheduler-tauri:** Synchronize chaos-scheduler-desktop versions

## [1.0.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v1.0.0...chaos-scheduler-tauri-v1.0.1) (2026-07-07)


### Bug Fixes

* **mcp:** persist and broadcast late-stage provisioning failures ([#133](https://github.com/KleinPerkins/chaos-scheduler/issues/133)) ([ccf1019](https://github.com/KleinPerkins/chaos-scheduler/commit/ccf10194ab2b2eeb9557e3f9453803fefa0a7fd7))
* **test:** de-flake cursor_agent poll tests blocking CI ([#135](https://github.com/KleinPerkins/chaos-scheduler/issues/135)) ([0deb0cd](https://github.com/KleinPerkins/chaos-scheduler/commit/0deb0cd30f6047050918e8075e9c14bc1a38d96a))

## [1.0.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.6.1...chaos-scheduler-tauri-v1.0.0) (2026-07-07)


### ⚠ BREAKING CHANGES

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132))

### Features

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132)) ([f5017e5](https://github.com/KleinPerkins/chaos-scheduler/commit/f5017e5254984989c5bce70ebd350960af8d1e52))
* **integrations:** add managed-MCP card + startup re-provision hook ([#114](https://github.com/KleinPerkins/chaos-scheduler/issues/114)) ([0eab261](https://github.com/KleinPerkins/chaos-scheduler/commit/0eab261fcd104dfffd76306e08b284f518a26a78))
* **mcp:** add managed MCP provisioner backend ([#112](https://github.com/KleinPerkins/chaos-scheduler/issues/112)) ([c805af9](https://github.com/KleinPerkins/chaos-scheduler/commit/c805af9d7f30217cfd5dd54f4a30e0bc597810b0))
* **mcp:** emit a status-changed event so Integrations stays live ([#128](https://github.com/KleinPerkins/chaos-scheduler/issues/128)) ([2316da4](https://github.com/KleinPerkins/chaos-scheduler/commit/2316da46aa3c26b0c0735b2d2158c7c4de477e05))
* **updater:** add background check snapshot, status, and preferences ([#110](https://github.com/KleinPerkins/chaos-scheduler/issues/110)) ([ccf001d](https://github.com/KleinPerkins/chaos-scheduler/commit/ccf001d9c48879bb477caef2c9fb718b298272fa))
* **updater:** add download/drain/install/restart apply flow + frontend hook ([#113](https://github.com/KleinPerkins/chaos-scheduler/issues/113)) ([73d8933](https://github.com/KleinPerkins/chaos-scheduler/commit/73d8933bb3769d7058fb4cba85a0446fd65730b9))
* **updater:** add UX surfaces, Settings controls, e2e/axe, and release smoke ([#115](https://github.com/KleinPerkins/chaos-scheduler/issues/115)) ([189049e](https://github.com/KleinPerkins/chaos-scheduler/commit/189049e2223ca7c9b61e59fbbb812e600e8480ed))


### Bug Fixes

* **mcp:** bound npm install with a timeout instead of blocking forever ([#131](https://github.com/KleinPerkins/chaos-scheduler/issues/131)) ([5857838](https://github.com/KleinPerkins/chaos-scheduler/commit/58578383ae7f6d920333bdca1181c86da7e902cf))
* **mcp:** don't trust managed_key_id as live until mcp.json merge succeeds ([#124](https://github.com/KleinPerkins/chaos-scheduler/issues/124)) ([b0b2c91](https://github.com/KleinPerkins/chaos-scheduler/commit/b0b2c915893cb70aa78397943ce043c16a9f3524))
* **mcp:** give invalid-JSON mcp.json backups sub-second-unique filenames ([#125](https://github.com/KleinPerkins/chaos-scheduler/issues/125)) ([c8957ce](https://github.com/KleinPerkins/chaos-scheduler/commit/c8957ceabf26af043f582bc1938bee784e7b67f2))
* **mcp:** harden npm install and validate resolved CLI path ([#116](https://github.com/KleinPerkins/chaos-scheduler/issues/116)) ([f6a89dd](https://github.com/KleinPerkins/chaos-scheduler/commit/f6a89ddb421b82b320f18858fa9c58c5a5c96483))
* **mcp:** recover from mutex poisoning instead of bricking provisioning ([#119](https://github.com/KleinPerkins/chaos-scheduler/issues/119)) ([15312c4](https://github.com/KleinPerkins/chaos-scheduler/commit/15312c4cf2fc8316ab80dc8457d5bb4d5c466e6f))
* **mcp:** resolve common nvm alias forms, not just literal versions ([#120](https://github.com/KleinPerkins/chaos-scheduler/issues/120)) ([8e8e040](https://github.com/KleinPerkins/chaos-scheduler/commit/8e8e04054e6a80a7fac8b7b02f6b7e7cfe889ddd))
* **mcp:** roll back promote_staging when the final rename fails ([#130](https://github.com/KleinPerkins/chaos-scheduler/issues/130)) ([e7f38a3](https://github.com/KleinPerkins/chaos-scheduler/commit/e7f38a3241aa62edc1ba16f78f2382e039b7059b))
* **mcp:** sweep orphaned staging/displaced dirs on startup ([#121](https://github.com/KleinPerkins/chaos-scheduler/issues/121)) ([0d8d7e2](https://github.com/KleinPerkins/chaos-scheduler/commit/0d8d7e2ff78005861dbf37f2a326187f945c70f7))
* **updater:** broadcast preference changes to every window and the tray ([#126](https://github.com/KleinPerkins/chaos-scheduler/issues/126)) ([2507359](https://github.com/KleinPerkins/chaos-scheduler/commit/25073592493b231aa2ecf70784e04bf3ebb6ed5e))
* **updater:** make apply() single-flight claim atomic ([#118](https://github.com/KleinPerkins/chaos-scheduler/issues/118)) ([2cec175](https://github.com/KleinPerkins/chaos-scheduler/commit/2cec1759acb7c708b861f9311e891916cfdb9c12))

## [0.6.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.6.0...chaos-scheduler-tauri-v0.6.1) (2026-07-06)


### Bug Fixes

* **cursor_agent:** backward-compat repo field fallback + review follow-ups ([#106](https://github.com/KleinPerkins/chaos-scheduler/issues/106)) ([7212663](https://github.com/KleinPerkins/chaos-scheduler/commit/7212663a29d0ea00600a9eb6c80788be16851e42))

## [0.6.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.5.0...chaos-scheduler-tauri-v0.6.0) (2026-07-06)


### Features

* expose email profiles over REST, SDK, and MCP ([#104](https://github.com/KleinPerkins/chaos-scheduler/issues/104)) ([6be9e0d](https://github.com/KleinPerkins/chaos-scheduler/commit/6be9e0d67126566478f10938e80d56ed5dff437b))


### Bug Fixes

* **cursor_agent:** correct Cloud Agents v1 schema and harden execution ([#105](https://github.com/KleinPerkins/chaos-scheduler/issues/105)) ([ae1ca39](https://github.com/KleinPerkins/chaos-scheduler/commit/ae1ca3994b6833e131978a8fc087c2fe199f22f4))

## [0.5.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.4.0...chaos-scheduler-tauri-v0.5.0) (2026-07-06)


### Features

* **email:** named email profiles for workflow failure alerts ([#95](https://github.com/KleinPerkins/chaos-scheduler/issues/95)) ([fd48423](https://github.com/KleinPerkins/chaos-scheduler/commit/fd48423a16cfae410a4ef5d40603d21ff20d8cc8))

## [0.4.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.3.1...chaos-scheduler-tauri-v0.4.0) (2026-07-06)


### Miscellaneous Chores

* **chaos-scheduler-tauri:** Synchronize chaos-scheduler-desktop versions

## [0.3.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.3.0...chaos-scheduler-tauri-v0.3.1) (2026-07-06)


### Bug Fixes

* **desktop:** align tauri crate to 2.11 to match @tauri-apps/api ([#82](https://github.com/KleinPerkins/chaos-scheduler/issues/82)) ([1841217](https://github.com/KleinPerkins/chaos-scheduler/commit/1841217356d44e67c1037625e8622f4a00117c36))

## [0.3.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.2.0...chaos-scheduler-tauri-v0.3.0) (2026-07-05)


### Features

* add bounded scheduler execution core ([#55](https://github.com/KleinPerkins/chaos-scheduler/issues/55)) ([f2d7013](https://github.com/KleinPerkins/chaos-scheduler/commit/f2d701368cb357c55005e11d792fc3be6a481481))
* **api:** add REST workflow patch and rerun endpoints ([#62](https://github.com/KleinPerkins/chaos-scheduler/issues/62)) ([e9632cc](https://github.com/KleinPerkins/chaos-scheduler/commit/e9632cc25e429c672db0f67803e21dd9c1ac09ec))
* bound pre-migration .bak retention and expand recovery/migration tests ([#76](https://github.com/KleinPerkins/chaos-scheduler/issues/76)) ([1fd70af](https://github.com/KleinPerkins/chaos-scheduler/commit/1fd70af04472a00eb15d88d53802711c5a04a2b1))
* surface poll_exhausted as a first-class run status (backend) ([#75](https://github.com/KleinPerkins/chaos-scheduler/issues/75)) ([ab0d562](https://github.com/KleinPerkins/chaos-scheduler/commit/ab0d562066b9fe81e76ad44ff52ba610f5979548))
* **ui:** phase 5 ux/a11y polish and enqueue action ([#67](https://github.com/KleinPerkins/chaos-scheduler/issues/67)) ([eb64ec8](https://github.com/KleinPerkins/chaos-scheduler/commit/eb64ec817d08ac2968478c859d10741063996aeb))


### Bug Fixes

* admit runs atomically ([#48](https://github.com/KleinPerkins/chaos-scheduler/issues/48)) ([2505224](https://github.com/KleinPerkins/chaos-scheduler/commit/2505224931ea087dedcf8fe559f1dc3e76dbb5fd))
* bound scheduler chains and action polling ([#60](https://github.com/KleinPerkins/chaos-scheduler/issues/60)) ([499504a](https://github.com/KleinPerkins/chaos-scheduler/commit/499504aa0bf19c28fed1ea24e182b985a9356312))
* enforce protected environment writes ([#44](https://github.com/KleinPerkins/chaos-scheduler/issues/44)) ([2432dce](https://github.com/KleinPerkins/chaos-scheduler/commit/2432dcee356c0bf7b714770622e36223b045ae61))
* fold capacity and trigger state into atomic admission ([#53](https://github.com/KleinPerkins/chaos-scheduler/issues/53)) ([d111171](https://github.com/KleinPerkins/chaos-scheduler/commit/d1111714f97c3f9aff065783a7b2755a429a99a5))
* harden git_pull url, path, and argument handling ([#51](https://github.com/KleinPerkins/chaos-scheduler/issues/51)) ([896168d](https://github.com/KleinPerkins/chaos-scheduler/commit/896168ddf2edbcac5146a9902a26dcac320d607c))
* harden REST pre-auth guardrails ([#39](https://github.com/KleinPerkins/chaos-scheduler/issues/39)) ([1b52389](https://github.com/KleinPerkins/chaos-scheduler/commit/1b52389ee7ce7cd409211a5c9b74c4d04fe6199e))
* harden webhook security paths ([#54](https://github.com/KleinPerkins/chaos-scheduler/issues/54)) ([c153f93](https://github.com/KleinPerkins/chaos-scheduler/commit/c153f9314edcf8c87760a5ad9ef4cc8a2531adcd))
* persist queued idempotency outcomes ([#45](https://github.com/KleinPerkins/chaos-scheduler/issues/45)) ([c131c14](https://github.com/KleinPerkins/chaos-scheduler/commit/c131c14d9bf7b0de10effb5f154507c3b84319c3))
* pin cursor agent API host ([#33](https://github.com/KleinPerkins/chaos-scheduler/issues/33)) ([dc6b02f](https://github.com/KleinPerkins/chaos-scheduler/commit/dc6b02f9c92fd1ef5c959d20aadf8336a4649bd1))
* record accurate API audit outcomes ([#47](https://github.com/KleinPerkins/chaos-scheduler/issues/47)) ([35a39fd](https://github.com/KleinPerkins/chaos-scheduler/commit/35a39fdd536ec99a3d14e18c3978313c4a6e9e13))
* repair retention run foreign keys ([#41](https://github.com/KleinPerkins/chaos-scheduler/issues/41)) ([92614e4](https://github.com/KleinPerkins/chaos-scheduler/commit/92614e4d256bc3f62ab47acdbd2ebc0b3a9ec307))
* roll back partial workflow registration and map dispatch errors ([#52](https://github.com/KleinPerkins/chaos-scheduler/issues/52)) ([29c0b6a](https://github.com/KleinPerkins/chaos-scheduler/commit/29c0b6a54a5c87d243864de46b48e47f2d1cb11c))
* **scheduler:** bounded graceful shutdown via off-main-thread grace exit ([#70](https://github.com/KleinPerkins/chaos-scheduler/issues/70)) ([0ea4c17](https://github.com/KleinPerkins/chaos-scheduler/commit/0ea4c178491054cf68d57bdb96e22f21ad85c781))
* **security:** gate non-loopback REST + metrics binds behind opt-in flag ([#73](https://github.com/KleinPerkins/chaos-scheduler/issues/73)) ([8b1a2c1](https://github.com/KleinPerkins/chaos-scheduler/commit/8b1a2c1d27acb1d18b9f40c6e87656737d5dade5))
* **security:** pin DNS + block redirects/IPv4-mapped on outbound webhooks ([#71](https://github.com/KleinPerkins/chaos-scheduler/issues/71)) ([31f5dc9](https://github.com/KleinPerkins/chaos-scheduler/commit/31f5dc9b7f4366b104c348ff21bdf49a0315bb66))
* **security:** redact workflow secrets from read-scoped API/MCP responses ([#74](https://github.com/KleinPerkins/chaos-scheduler/issues/74)) ([00d0152](https://github.com/KleinPerkins/chaos-scheduler/commit/00d0152a4eac5a26cb18cb504675e4f08c1b63a4))
* **security:** strip scheduler-internal secrets from child process env ([#72](https://github.com/KleinPerkins/chaos-scheduler/issues/72)) ([22242bf](https://github.com/KleinPerkins/chaos-scheduler/commit/22242bfa39ab54fcb0449126057bd171707c125c))

## [0.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-tauri-v0.1.0...chaos-scheduler-tauri-v0.2.0) (2026-07-04)


### Features

* make chaos-scheduler independent from chaos-labs ([54b1944](https://github.com/KleinPerkins/chaos-scheduler/commit/54b1944a6dd682462cc8d9ee6be4f9efff928ba3))
* migrate Scheduler to product repo — move from instance-only app.pre-deploy-backup/ to scheduler/, replace hardcoded paths with dynamic detection, add get_app_config Tauri command, update deploy.py and docs ([3cb837d](https://github.com/KleinPerkins/chaos-scheduler/commit/3cb837d449999a49ecbbfd4bbdf2b3ec7db89674))


### Bug Fixes

* **scheduler:** harden queue runtime edge cases ([099a08f](https://github.com/KleinPerkins/chaos-scheduler/commit/099a08fcec011d2140f1e559b78e090e268cbb77))
* **scheduler:** resolve data root via CHAOS_LABS_ROOT, default to canonical repo ([db76ab1](https://github.com/KleinPerkins/chaos-scheduler/commit/db76ab1dd241133e7feae82f2c25e5a104067488))
