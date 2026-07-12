# Changelog

## [1.1.0](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v1.0.1...sdk-ts-v1.1.0) (2026-07-12)


### Features

* **sdk:** deprecate runWorkflow in favor of enqueueWorkflow ([#265](https://github.com/KleinPerkins/chaos-scheduler/issues/265)) ([4894ec5](https://github.com/KleinPerkins/chaos-scheduler/commit/4894ec5d6bfd5b8f77b4fd47336d077ef106bf70))

## [1.0.1](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v1.0.0...sdk-ts-v1.0.1) (2026-07-07)


### Bug Fixes

* **security:** remove polynomial-time regexes flagged by CodeQL ([#139](https://github.com/KleinPerkins/chaos-scheduler/issues/139)) ([edbb395](https://github.com/KleinPerkins/chaos-scheduler/commit/edbb3950793058f6087a5a454653826b4ee2aa89))

## [1.0.0](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v0.4.0...sdk-ts-v1.0.0) (2026-07-07)


### ⚠ BREAKING CHANGES

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132))

### Features

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132)) ([f5017e5](https://github.com/KleinPerkins/chaos-scheduler/commit/f5017e5254984989c5bce70ebd350960af8d1e52))

## [0.4.0](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v0.3.0...sdk-ts-v0.4.0) (2026-07-06)


### Features

* expose email profiles over REST, SDK, and MCP ([#104](https://github.com/KleinPerkins/chaos-scheduler/issues/104)) ([6be9e0d](https://github.com/KleinPerkins/chaos-scheduler/commit/6be9e0d67126566478f10938e80d56ed5dff437b))

## [0.3.0](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v0.2.0...sdk-ts-v0.3.0) (2026-07-05)


### Features

* **api:** add REST workflow patch and rerun endpoints ([#62](https://github.com/KleinPerkins/chaos-scheduler/issues/62)) ([e9632cc](https://github.com/KleinPerkins/chaos-scheduler/commit/e9632cc25e429c672db0f67803e21dd9c1ac09ec))
* **packages:** add SDK/MCP read methods for runs and queues ([#59](https://github.com/KleinPerkins/chaos-scheduler/issues/59)) ([8898873](https://github.com/KleinPerkins/chaos-scheduler/commit/8898873c6affcf768355cf380cd7fdf66f126ce1))


### Bug Fixes

* enforce protected environment writes ([#44](https://github.com/KleinPerkins/chaos-scheduler/issues/44)) ([2432dce](https://github.com/KleinPerkins/chaos-scheduler/commit/2432dcee356c0bf7b714770622e36223b045ae61))
* persist queued idempotency outcomes ([#45](https://github.com/KleinPerkins/chaos-scheduler/issues/45)) ([c131c14](https://github.com/KleinPerkins/chaos-scheduler/commit/c131c14d9bf7b0de10effb5f154507c3b84319c3))
* **sdk:** canonical inbound webhook signing ([#69](https://github.com/KleinPerkins/chaos-scheduler/issues/69)) ([913cffc](https://github.com/KleinPerkins/chaos-scheduler/commit/913cffcb1eac035087afdbc774ce8bb0cf38fa0d))

## [0.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/sdk-ts-v0.1.0...sdk-ts-v0.2.0) (2026-07-04)


### Features

* make chaos-scheduler independent from chaos-labs ([54b1944](https://github.com/KleinPerkins/chaos-scheduler/commit/54b1944a6dd682462cc8d9ee6be4f9efff928ba3))


### Bug Fixes

* **packages:** add self-contained vitest config to avoid loading root vite.config ([#20](https://github.com/KleinPerkins/chaos-scheduler/issues/20)) ([e856dd2](https://github.com/KleinPerkins/chaos-scheduler/commit/e856dd2b25c775823d1b0cbc85c06edab71e7dd1))
