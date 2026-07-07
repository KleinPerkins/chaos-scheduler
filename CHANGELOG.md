# Changelog

## [1.0.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.6.1...chaos-scheduler-v1.0.0) (2026-07-07)


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

* **integrations:** give Remove and Prepare-to-uninstall independent confirm gates ([#123](https://github.com/KleinPerkins/chaos-scheduler/issues/123)) ([d90e476](https://github.com/KleinPerkins/chaos-scheduler/commit/d90e476bcdc76493c3baf3e997186c3af303b51d))
* **mcp-server:** bundle transitive runtime deps for zero npm footprint ([#117](https://github.com/KleinPerkins/chaos-scheduler/issues/117)) ([9131daf](https://github.com/KleinPerkins/chaos-scheduler/commit/9131daf444bb7a368cb7271523481e604228f8d3))
* **mcp:** bound npm install with a timeout instead of blocking forever ([#131](https://github.com/KleinPerkins/chaos-scheduler/issues/131)) ([5857838](https://github.com/KleinPerkins/chaos-scheduler/commit/58578383ae7f6d920333bdca1181c86da7e902cf))
* **mcp:** don't trust managed_key_id as live until mcp.json merge succeeds ([#124](https://github.com/KleinPerkins/chaos-scheduler/issues/124)) ([b0b2c91](https://github.com/KleinPerkins/chaos-scheduler/commit/b0b2c915893cb70aa78397943ce043c16a9f3524))
* **mcp:** give invalid-JSON mcp.json backups sub-second-unique filenames ([#125](https://github.com/KleinPerkins/chaos-scheduler/issues/125)) ([c8957ce](https://github.com/KleinPerkins/chaos-scheduler/commit/c8957ceabf26af043f582bc1938bee784e7b67f2))
* **mcp:** harden npm install and validate resolved CLI path ([#116](https://github.com/KleinPerkins/chaos-scheduler/issues/116)) ([f6a89dd](https://github.com/KleinPerkins/chaos-scheduler/commit/f6a89ddb421b82b320f18858fa9c58c5a5c96483))
* **mcp:** recover from mutex poisoning instead of bricking provisioning ([#119](https://github.com/KleinPerkins/chaos-scheduler/issues/119)) ([15312c4](https://github.com/KleinPerkins/chaos-scheduler/commit/15312c4cf2fc8316ab80dc8457d5bb4d5c466e6f))
* **mcp:** resolve common nvm alias forms, not just literal versions ([#120](https://github.com/KleinPerkins/chaos-scheduler/issues/120)) ([8e8e040](https://github.com/KleinPerkins/chaos-scheduler/commit/8e8e04054e6a80a7fac8b7b02f6b7e7cfe889ddd))
* **mcp:** roll back promote_staging when the final rename fails ([#130](https://github.com/KleinPerkins/chaos-scheduler/issues/130)) ([e7f38a3](https://github.com/KleinPerkins/chaos-scheduler/commit/e7f38a3241aa62edc1ba16f78f2382e039b7059b))
* **mcp:** sweep orphaned staging/displaced dirs on startup ([#121](https://github.com/KleinPerkins/chaos-scheduler/issues/121)) ([0d8d7e2](https://github.com/KleinPerkins/chaos-scheduler/commit/0d8d7e2ff78005861dbf37f2a326187f945c70f7))
* **settings:** remove dead legacy update UI, source Settings from useAppUpdate() ([#127](https://github.com/KleinPerkins/chaos-scheduler/issues/127)) ([02ca1e6](https://github.com/KleinPerkins/chaos-scheduler/commit/02ca1e6eab8a94e0b957e7195fb538b8a2d652a1))
* **test:** return a fresh copy from the mocked get_mcp_integration_status ([#122](https://github.com/KleinPerkins/chaos-scheduler/issues/122)) ([2790023](https://github.com/KleinPerkins/chaos-scheduler/commit/2790023bf82e07319a1b7509dd0e40a5892a490c))
* **test:** return fresh objects from update IPC fixture handlers ([#129](https://github.com/KleinPerkins/chaos-scheduler/issues/129)) ([e52cc7e](https://github.com/KleinPerkins/chaos-scheduler/commit/e52cc7e6b6050c8543f9afeec47b6a2d866a1db0))
* **updater:** broadcast preference changes to every window and the tray ([#126](https://github.com/KleinPerkins/chaos-scheduler/issues/126)) ([2507359](https://github.com/KleinPerkins/chaos-scheduler/commit/25073592493b231aa2ecf70784e04bf3ebb6ed5e))
* **updater:** make apply() single-flight claim atomic ([#118](https://github.com/KleinPerkins/chaos-scheduler/issues/118)) ([2cec175](https://github.com/KleinPerkins/chaos-scheduler/commit/2cec1759acb7c708b861f9311e891916cfdb9c12))

## [0.6.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.6.0...chaos-scheduler-v0.6.1) (2026-07-06)


### Bug Fixes

* **cursor_agent:** backward-compat repo field fallback + review follow-ups ([#106](https://github.com/KleinPerkins/chaos-scheduler/issues/106)) ([7212663](https://github.com/KleinPerkins/chaos-scheduler/commit/7212663a29d0ea00600a9eb6c80788be16851e42))

## [0.6.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.5.0...chaos-scheduler-v0.6.0) (2026-07-06)


### Features

* expose email profiles over REST, SDK, and MCP ([#104](https://github.com/KleinPerkins/chaos-scheduler/issues/104)) ([6be9e0d](https://github.com/KleinPerkins/chaos-scheduler/commit/6be9e0d67126566478f10938e80d56ed5dff437b))


### Bug Fixes

* **cursor_agent:** correct Cloud Agents v1 schema and harden execution ([#105](https://github.com/KleinPerkins/chaos-scheduler/issues/105)) ([ae1ca39](https://github.com/KleinPerkins/chaos-scheduler/commit/ae1ca3994b6833e131978a8fc087c2fe199f22f4))


### Refactors

* **db:** drop the vestigial corpus column from workflows ([#102](https://github.com/KleinPerkins/chaos-scheduler/issues/102)) ([107cc69](https://github.com/KleinPerkins/chaos-scheduler/commit/107cc692bbef4b57cadbb7ed21fef6646f913e8a))
* rename mission-control corpus_filter to environment_filter ([#103](https://github.com/KleinPerkins/chaos-scheduler/issues/103)) ([1b80ed2](https://github.com/KleinPerkins/chaos-scheduler/commit/1b80ed2df3d1a7b306fe3054833bd1d0b77bfb06))
* **scheduler:** drop corpus from the serialized read contract ([#101](https://github.com/KleinPerkins/chaos-scheduler/issues/101)) ([ea52ca3](https://github.com/KleinPerkins/chaos-scheduler/commit/ea52ca3a5fdacbab3999057c3e674c066069d1d4))
* **scheduler:** make environment the authoritative workflow partition ([#99](https://github.com/KleinPerkins/chaos-scheduler/issues/99)) ([683263c](https://github.com/KleinPerkins/chaos-scheduler/commit/683263ce7bf067167ea5e70eacae2f7ad9620c91))

## [0.5.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.4.0...chaos-scheduler-v0.5.0) (2026-07-06)


### Features

* **email:** named email profiles for workflow failure alerts ([#95](https://github.com/KleinPerkins/chaos-scheduler/issues/95)) ([fd48423](https://github.com/KleinPerkins/chaos-scheduler/commit/fd48423a16cfae410a4ef5d40603d21ff20d8cc8))
* **ui:** add light/dark design-system tokens and a real icon set ([#93](https://github.com/KleinPerkins/chaos-scheduler/issues/93)) ([ef75978](https://github.com/KleinPerkins/chaos-scheduler/commit/ef75978376cd0fc1bb5c4b4b2d914c9c7b144f9b))
* **ui:** unified per-workflow detail hub ([#97](https://github.com/KleinPerkins/chaos-scheduler/issues/97)) ([2d18bf4](https://github.com/KleinPerkins/chaos-scheduler/commit/2d18bf4cddf381bd01092e88bd25c2655740eede))


### Bug Fixes

* **ui:** surface revoked state so API-key revoke visibly persists ([#96](https://github.com/KleinPerkins/chaos-scheduler/issues/96)) ([0d61fc0](https://github.com/KleinPerkins/chaos-scheduler/commit/0d61fc02b8ed1d58c8482424b677f45122e86935))

## [0.4.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v0.3.1...chaos-scheduler-v0.4.0) (2026-07-06)


### Features

* **mcp-server:** advertise server icon + website in MCP handshake ([#91](https://github.com/KleinPerkins/chaos-scheduler/issues/91)) ([538bd89](https://github.com/KleinPerkins/chaos-scheduler/commit/538bd8990aadbd78ccd4c5333d91b397a48781da))
* **ui:** consolidate to a single Mission Control home ([#92](https://github.com/KleinPerkins/chaos-scheduler/issues/92)) ([9491039](https://github.com/KleinPerkins/chaos-scheduler/commit/949103946d08a9560d8d6c7b6d61ec8bd435804c))


### Bug Fixes

* correct desktop version drift and auto-bump on release ([#90](https://github.com/KleinPerkins/chaos-scheduler/issues/90)) ([8d1fd6f](https://github.com/KleinPerkins/chaos-scheduler/commit/8d1fd6f4ee976ad3386dfef31f9ab378bb23c459))
* **release:** pin GitHub's Latest flag to the desktop release ([#85](https://github.com/KleinPerkins/chaos-scheduler/issues/85)) ([b5fc1e3](https://github.com/KleinPerkins/chaos-scheduler/commit/b5fc1e3b4fefab527f9c9ebd213618dcc27dfbc3))


### Documentation

* **memory:** publish session learnings (hardening + release + tauri CI gate + signing correction) ([#89](https://github.com/KleinPerkins/chaos-scheduler/issues/89)) ([c90cffa](https://github.com/KleinPerkins/chaos-scheduler/commit/c90cffaef0748e37fbca2a7c967a59338281a059))

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
