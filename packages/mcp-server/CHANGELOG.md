# Changelog

## [1.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v1.1.0...mcp-server-v1.2.0) (2026-07-12)


### Features

* **mcp-server:** deprecate run_workflow_now in favor of enqueue_workflow ([#267](https://github.com/KleinPerkins/chaos-scheduler/issues/267)) ([081ed6f](https://github.com/KleinPerkins/chaos-scheduler/commit/081ed6f6c9c9b627fd7cc50da0bf80d9f41b0750))

## [1.1.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v1.0.2...mcp-server-v1.1.0) (2026-07-09)


### Features

* design-system token foundation + orbital-8 app icon ([#146](https://github.com/KleinPerkins/chaos-scheduler/issues/146)) ([b25ce59](https://github.com/KleinPerkins/chaos-scheduler/commit/b25ce594189594256ae120ce0c58e18ba4b9cce7))

## [1.0.2](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v1.0.1...mcp-server-v1.0.2) (2026-07-07)


### Bug Fixes

* **security:** remove polynomial-time regexes flagged by CodeQL ([#139](https://github.com/KleinPerkins/chaos-scheduler/issues/139)) ([edbb395](https://github.com/KleinPerkins/chaos-scheduler/commit/edbb3950793058f6087a5a454653826b4ee2aa89))
* **test:** avoid dynamic regexes in build output assertions ([#141](https://github.com/KleinPerkins/chaos-scheduler/issues/141)) ([8e98009](https://github.com/KleinPerkins/chaos-scheduler/commit/8e980094e6bae7601ca30990a3239535babbad00))

## [1.0.1](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v1.0.0...mcp-server-v1.0.1) (2026-07-07)


### Bug Fixes

* **mcp-server:** keep @chaos-scheduler/sdk external so SDK hotfixes reach users ([#136](https://github.com/KleinPerkins/chaos-scheduler/issues/136)) ([75342a9](https://github.com/KleinPerkins/chaos-scheduler/commit/75342a96a3b227f11e252ce8bfd40128409ff705))

## [1.0.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v0.5.0...mcp-server-v1.0.0) (2026-07-07)


### ⚠ BREAKING CHANGES

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132))

### Features

* **env:** rename source/instance environments to production/sandbox ([#132](https://github.com/KleinPerkins/chaos-scheduler/issues/132)) ([f5017e5](https://github.com/KleinPerkins/chaos-scheduler/commit/f5017e5254984989c5bce70ebd350960af8d1e52))


### Bug Fixes

* **mcp-server:** bundle transitive runtime deps for zero npm footprint ([#117](https://github.com/KleinPerkins/chaos-scheduler/issues/117)) ([9131daf](https://github.com/KleinPerkins/chaos-scheduler/commit/9131daf444bb7a368cb7271523481e604228f8d3))

## [0.5.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v0.4.0...mcp-server-v0.5.0) (2026-07-06)


### Features

* expose email profiles over REST, SDK, and MCP ([#104](https://github.com/KleinPerkins/chaos-scheduler/issues/104)) ([6be9e0d](https://github.com/KleinPerkins/chaos-scheduler/commit/6be9e0d67126566478f10938e80d56ed5dff437b))

## [0.4.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v0.3.0...mcp-server-v0.4.0) (2026-07-06)


### Features

* **mcp-server:** advertise server icon + website in MCP handshake ([#91](https://github.com/KleinPerkins/chaos-scheduler/issues/91)) ([538bd89](https://github.com/KleinPerkins/chaos-scheduler/commit/538bd8990aadbd78ccd4c5333d91b397a48781da))

## [0.3.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v0.2.0...mcp-server-v0.3.0) (2026-07-05)


### Features

* **api:** add REST workflow patch and rerun endpoints ([#62](https://github.com/KleinPerkins/chaos-scheduler/issues/62)) ([e9632cc](https://github.com/KleinPerkins/chaos-scheduler/commit/e9632cc25e429c672db0f67803e21dd9c1ac09ec))
* **packages:** add SDK/MCP read methods for runs and queues ([#59](https://github.com/KleinPerkins/chaos-scheduler/issues/59)) ([8898873](https://github.com/KleinPerkins/chaos-scheduler/commit/8898873c6affcf768355cf380cd7fdf66f126ce1))


### Bug Fixes

* harden MCP HTTP transport ([#37](https://github.com/KleinPerkins/chaos-scheduler/issues/37)) ([79f22d8](https://github.com/KleinPerkins/chaos-scheduler/commit/79f22d8b051789a5d5fabeaa72a24df37229ee9d))
* **mcp:** fail-closed protected-env guardrail and shared HTTP budget ([#68](https://github.com/KleinPerkins/chaos-scheduler/issues/68)) ([b82087a](https://github.com/KleinPerkins/chaos-scheduler/commit/b82087afd8376408d099db6c1390db77cc7a34ac))
* persist queued idempotency outcomes ([#45](https://github.com/KleinPerkins/chaos-scheduler/issues/45)) ([c131c14](https://github.com/KleinPerkins/chaos-scheduler/commit/c131c14d9bf7b0de10effb5f154507c3b84319c3))

## [0.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/mcp-server-v0.1.0...mcp-server-v0.2.0) (2026-07-04)


### Features

* make chaos-scheduler independent from chaos-labs ([54b1944](https://github.com/KleinPerkins/chaos-scheduler/commit/54b1944a6dd682462cc8d9ee6be4f9efff928ba3))


### Bug Fixes

* **packages:** add self-contained vitest config to avoid loading root vite.config ([#20](https://github.com/KleinPerkins/chaos-scheduler/issues/20)) ([e856dd2](https://github.com/KleinPerkins/chaos-scheduler/commit/e856dd2b25c775823d1b0cbc85c06edab71e7dd1))
