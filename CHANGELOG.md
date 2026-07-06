# Changelog

## [0.3.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.3.0...chaos-scheduler-v0.3.1) (2026-07-06)


### Bug Fixes

* **desktop:** align tauri crate to 2.11 to match @tauri-apps/api ([#82](https://github.com/KleinPerkins/chaos-scheduler/issues/82)) ([1841217](https://github.com/KleinPerkins/chaos-scheduler/commit/1841217356d44e67c1037625e8622f4a00117c36))

## [0.3.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.2.0...chaos-scheduler-v0.3.0) (2026-07-05)


### Features

* add bounded scheduler execution core ([#55](https://github.com/KleinPerkins/chaos-scheduler/issues/55)) ([f2d7013](https://github.com/KleinPerkins/chaos-scheduler/commit/f2d701368cb357c55005e11d792fc3be6a481481))
* **api:** add REST workflow patch and rerun endpoints ([#62](https://github.com/KleinPerkins/chaos-scheduler/issues/62)) ([e9632cc](https://github.com/KleinPerkins/chaos-scheduler/commit/e9632cc25e429c672db0f67803e21dd9c1ac09ec))
* bound pre-migration .bak retention and expand recovery/migration tests ([#76](https://github.com/KleinPerkins/chaos-scheduler/issues/76)) ([1fd70af](https://github.com/KleinPerkins/chaos-scheduler/commit/1fd70af04472a00eb15d88d53802711c5a04a2b1))
* **packages:** add SDK/MCP read methods for runs and queues ([#59](https://github.com/KleinPerkins/chaos-scheduler/issues/59)) ([8898873](https://github.com/KleinPerkins/chaos-scheduler/commit/8898873c6affcf768355cf380cd7fdf66f126ce1))
* surface poll_exhausted as a first-class run status (backend) ([#75](https://github.com/KleinPerkins/chaos-scheduler/issues/75)) ([ab0d562](https://github.com/KleinPerkins/chaos-scheduler/commit/ab0d562066b9fe81e76ad44ff52ba610f5979548))
* **ui:** phase 5 ux/a11y polish and enqueue action ([#67](https://github.com/KleinPerkins/chaos-scheduler/issues/67)) ([eb64ec8](https://github.com/KleinPerkins/chaos-scheduler/commit/eb64ec817d08ac2968478c859d10741063996aeb))
* **ui:** surface poll_exhausted in run status UI ([#77](https://github.com/KleinPerkins/chaos-scheduler/issues/77)) ([b2f872f](https://github.com/KleinPerkins/chaos-scheduler/commit/b2f872fa3f4030c06b8f82493e1b2caea9e82c6f))


### Bug Fixes

* admit runs atomically ([#48](https://github.com/KleinPerkins/chaos-scheduler/issues/48)) ([2505224](https://github.com/KleinPerkins/chaos-scheduler/commit/2505224931ea087dedcf8fe559f1dc3e76dbb5fd))
* bound scheduler chains and action polling ([#60](https://github.com/KleinPerkins/chaos-scheduler/issues/60)) ([499504a](https://github.com/KleinPerkins/chaos-scheduler/commit/499504aa0bf19c28fed1ea24e182b985a9356312))
* enforce protected environment writes ([#44](https://github.com/KleinPerkins/chaos-scheduler/issues/44)) ([2432dce](https://github.com/KleinPerkins/chaos-scheduler/commit/2432dcee356c0bf7b714770622e36223b045ae61))
* fold capacity and trigger state into atomic admission ([#53](https://github.com/KleinPerkins/chaos-scheduler/issues/53)) ([d111171](https://github.com/KleinPerkins/chaos-scheduler/commit/d1111714f97c3f9aff065783a7b2755a429a99a5))
* harden git_pull url, path, and argument handling ([#51](https://github.com/KleinPerkins/chaos-scheduler/issues/51)) ([896168d](https://github.com/KleinPerkins/chaos-scheduler/commit/896168ddf2edbcac5146a9902a26dcac320d607c))
* harden MCP HTTP transport ([#37](https://github.com/KleinPerkins/chaos-scheduler/issues/37)) ([79f22d8](https://github.com/KleinPerkins/chaos-scheduler/commit/79f22d8b051789a5d5fabeaa72a24df37229ee9d))
* harden REST pre-auth guardrails ([#39](https://github.com/KleinPerkins/chaos-scheduler/issues/39)) ([1b52389](https://github.com/KleinPerkins/chaos-scheduler/commit/1b52389ee7ce7cd409211a5c9b74c4d04fe6199e))
* harden webhook security paths ([#54](https://github.com/KleinPerkins/chaos-scheduler/issues/54)) ([c153f93](https://github.com/KleinPerkins/chaos-scheduler/commit/c153f9314edcf8c87760a5ad9ef4cc8a2531adcd))
* install and build packages before test:packages ([#42](https://github.com/KleinPerkins/chaos-scheduler/issues/42)) ([eec8ad0](https://github.com/KleinPerkins/chaos-scheduler/commit/eec8ad0beee5d4a93e1d181ed36dbf1b0f269563))
* **mcp:** fail-closed protected-env guardrail and shared HTTP budget ([#68](https://github.com/KleinPerkins/chaos-scheduler/issues/68)) ([b82087a](https://github.com/KleinPerkins/chaos-scheduler/commit/b82087afd8376408d099db6c1390db77cc7a34ac))
* persist queued idempotency outcomes ([#45](https://github.com/KleinPerkins/chaos-scheduler/issues/45)) ([c131c14](https://github.com/KleinPerkins/chaos-scheduler/commit/c131c14d9bf7b0de10effb5f154507c3b84319c3))
* pin cursor agent API host ([#33](https://github.com/KleinPerkins/chaos-scheduler/issues/33)) ([dc6b02f](https://github.com/KleinPerkins/chaos-scheduler/commit/dc6b02f9c92fd1ef5c959d20aadf8336a4649bd1))
* record accurate API audit outcomes ([#47](https://github.com/KleinPerkins/chaos-scheduler/issues/47)) ([35a39fd](https://github.com/KleinPerkins/chaos-scheduler/commit/35a39fdd536ec99a3d14e18c3978313c4a6e9e13))
* repair retention run foreign keys ([#41](https://github.com/KleinPerkins/chaos-scheduler/issues/41)) ([92614e4](https://github.com/KleinPerkins/chaos-scheduler/commit/92614e4d256bc3f62ab47acdbd2ebc0b3a9ec307))
* roll back partial workflow registration and map dispatch errors ([#52](https://github.com/KleinPerkins/chaos-scheduler/issues/52)) ([29c0b6a](https://github.com/KleinPerkins/chaos-scheduler/commit/29c0b6a54a5c87d243864de46b48e47f2d1cb11c))
* **scheduler:** bounded graceful shutdown via off-main-thread grace exit ([#70](https://github.com/KleinPerkins/chaos-scheduler/issues/70)) ([0ea4c17](https://github.com/KleinPerkins/chaos-scheduler/commit/0ea4c178491054cf68d57bdb96e22f21ad85c781))
* **sdk:** canonical inbound webhook signing ([#69](https://github.com/KleinPerkins/chaos-scheduler/issues/69)) ([913cffc](https://github.com/KleinPerkins/chaos-scheduler/commit/913cffcb1eac035087afdbc774ce8bb0cf38fa0d))
* **security:** gate non-loopback REST + metrics binds behind opt-in flag ([#73](https://github.com/KleinPerkins/chaos-scheduler/issues/73)) ([8b1a2c1](https://github.com/KleinPerkins/chaos-scheduler/commit/8b1a2c1d27acb1d18b9f40c6e87656737d5dade5))
* **security:** pin DNS + block redirects/IPv4-mapped on outbound webhooks ([#71](https://github.com/KleinPerkins/chaos-scheduler/issues/71)) ([31f5dc9](https://github.com/KleinPerkins/chaos-scheduler/commit/31f5dc9b7f4366b104c348ff21bdf49a0315bb66))
* **security:** redact workflow secrets from read-scoped API/MCP responses ([#74](https://github.com/KleinPerkins/chaos-scheduler/issues/74)) ([00d0152](https://github.com/KleinPerkins/chaos-scheduler/commit/00d0152a4eac5a26cb18cb504675e4f08c1b63a4))
* **security:** strip scheduler-internal secrets from child process env ([#72](https://github.com/KleinPerkins/chaos-scheduler/issues/72)) ([22242bf](https://github.com/KleinPerkins/chaos-scheduler/commit/22242bfa39ab54fcb0449126057bd171707c125c))
* **ui:** enable jsx-a11y, WCAG AA contrast, and UX partials ([#79](https://github.com/KleinPerkins/chaos-scheduler/issues/79)) ([3cda072](https://github.com/KleinPerkins/chaos-scheduler/commit/3cda072198060a3b77fe05be437704873951e2bf))
* **ui:** surface Phase 1D early UX trap fixes ([#66](https://github.com/KleinPerkins/chaos-scheduler/issues/66)) ([b033bb8](https://github.com/KleinPerkins/chaos-scheduler/commit/b033bb8d6576d1d62ab365b87c6d677ab57c3f50))


### Documentation

* hardening gap-closure report and security/integration sweep ([#81](https://github.com/KleinPerkins/chaos-scheduler/issues/81)) ([1d566d8](https://github.com/KleinPerkins/chaos-scheduler/commit/1d566d8c04e688b7b7d38e4b41aaf9886fff0982))
* sync SDK/MCP read-method docs and add waitForRun/transport tests ([#65](https://github.com/KleinPerkins/chaos-scheduler/issues/65)) ([52cf812](https://github.com/KleinPerkins/chaos-scheduler/commit/52cf8125e0d2d69fcb87efefccbdd066d8262816))

## [0.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.1.0...chaos-scheduler-v0.2.0) (2026-07-04)


### Features

* make chaos-scheduler independent from chaos-labs ([54b1944](https://github.com/KleinPerkins/chaos-scheduler/commit/54b1944a6dd682462cc8d9ee6be4f9efff928ba3))
* migrate Scheduler to product repo — move from instance-only app.pre-deploy-backup/ to scheduler/, replace hardcoded paths with dynamic detection, add get_app_config Tauri command, update deploy.py and docs ([3cb837d](https://github.com/KleinPerkins/chaos-scheduler/commit/3cb837d449999a49ecbbfd4bbdf2b3ec7db89674))


### Bug Fixes

* **packages:** add self-contained vitest config to avoid loading root vite.config ([#20](https://github.com/KleinPerkins/chaos-scheduler/issues/20)) ([e856dd2](https://github.com/KleinPerkins/chaos-scheduler/commit/e856dd2b25c775823d1b0cbc85c06edab71e7dd1))
* **scheduler:** harden queue runtime edge cases ([099a08f](https://github.com/KleinPerkins/chaos-scheduler/commit/099a08fcec011d2140f1e559b78e090e268cbb77))
* **scheduler:** resolve data root via CHAOS_LABS_ROOT, default to canonical repo ([db76ab1](https://github.com/KleinPerkins/chaos-scheduler/commit/db76ab1dd241133e7feae82f2c25e5a104067488))
