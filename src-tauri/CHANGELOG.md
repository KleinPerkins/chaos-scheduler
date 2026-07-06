# Changelog

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
