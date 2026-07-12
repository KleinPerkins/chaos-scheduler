# Changelog

## [1.3.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.2.0...chaos-scheduler-v1.3.0) (2026-07-12)


### Features

* **charts:** add DualAxisLine, ImpactBars, and QueueLine ([#211](https://github.com/KleinPerkins/chaos-scheduler/issues/211)) ([36ded44](https://github.com/KleinPerkins/chaos-scheduler/commit/36ded442fd8a9d46d5e2f815d77dbc1109911984))
* **charts:** add Gauge and StatusDonut radial charts ([#210](https://github.com/KleinPerkins/chaos-scheduler/issues/210)) ([66f9f24](https://github.com/KleinPerkins/chaos-scheduler/commit/66f9f2477a29a61cf4917d8842af93ed5be5e9b5))
* **charts:** add RaceTrack + Vehicle race-view primitives + Vehicle Code Connect ([#212](https://github.com/KleinPerkins/chaos-scheduler/issues/212)) ([40a8511](https://github.com/KleinPerkins/chaos-scheduler/commit/40a851127ea9f960a3f40f748402ecc3ffa76ee8))
* **charts:** add SVG chart primitive foundation + d3 deps ([#209](https://github.com/KleinPerkins/chaos-scheduler/issues/209)) ([3495ac0](https://github.com/KleinPerkins/chaos-scheduler/commit/3495ac015451a4668b0472525c8c149e06ddce74))
* **components:** add BrandMark orbital-8 logo component ([#182](https://github.com/KleinPerkins/chaos-scheduler/issues/182)) ([8226086](https://github.com/KleinPerkins/chaos-scheduler/commit/8226086cdbad45ea2e7b26abc77290c979f05094))
* **components:** add InfoTip hover/focus definition affordance ([#181](https://github.com/KleinPerkins/chaos-scheduler/issues/181)) ([3efe3d2](https://github.com/KleinPerkins/chaos-scheduler/commit/3efe3d24d0de9f3cb9307fecf9004ee76303a2cb))
* **components:** add LookbackSelect segmented window selector ([#180](https://github.com/KleinPerkins/chaos-scheduler/issues/180)) ([39d5109](https://github.com/KleinPerkins/chaos-scheduler/commit/39d510937af8b0b1ca581846c180f97f189c4667))
* **components:** add StatusBar status-distribution primitive ([#178](https://github.com/KleinPerkins/chaos-scheduler/issues/178)) ([70f1820](https://github.com/KleinPerkins/chaos-scheduler/commit/70f1820fed48579e949601537fd84eb2ca2a1a37))
* **components:** dismiss InfoTip on Escape (keyboard a11y) ([#208](https://github.com/KleinPerkins/chaos-scheduler/issues/208)) ([1264e39](https://github.com/KleinPerkins/chaos-scheduler/commit/1264e396f91ec7f0fc587b18ca5abd5271b969f9))
* **dashboard:** add global FilterBar + custom-lookback wire format ([#213](https://github.com/KleinPerkins/chaos-scheduler/issues/213)) ([a95a58d](https://github.com/KleinPerkins/chaos-scheduler/commit/a95a58d3e13f01fba29b9baacb33d212850402c8))
* **dashboard:** add Needs Attention drill-down to Mission Control ([#215](https://github.com/KleinPerkins/chaos-scheduler/issues/215)) ([7babac0](https://github.com/KleinPerkins/chaos-scheduler/commit/7babac0a3e2db5324fcf44b4eb335aa96b321406))
* **dashboard:** add Operational Health drill-down to Mission Control ([#217](https://github.com/KleinPerkins/chaos-scheduler/issues/217)) ([c955ae6](https://github.com/KleinPerkins/chaos-scheduler/commit/c955ae6865a0237582cf0cc60d1df6dce27e3c8a))
* **dashboard:** add Resources drill-down to Mission Control ([#219](https://github.com/KleinPerkins/chaos-scheduler/issues/219)) ([574ba4d](https://github.com/KleinPerkins/chaos-scheduler/commit/574ba4de51176f628989c1adc43531873eb777c7))
* **dashboard:** blocked/waiting reason taxonomy + heavy-blocker Σ-wait ([#194](https://github.com/KleinPerkins/chaos-scheduler/issues/194)) ([328ae2e](https://github.com/KleinPerkins/chaos-scheduler/commit/328ae2e463ffb27935e9c18aefb5f3c1156fcc16))
* **dashboard:** compose Mission Control Overview vNext on real data ([#214](https://github.com/KleinPerkins/chaos-scheduler/issues/214)) ([a63d08b](https://github.com/KleinPerkins/chaos-scheduler/commit/a63d08be0fb0682e78e979fe54ebeef7af232022))
* **dashboard:** downstream blast-radius (chain count + depth) per workflow ([#197](https://github.com/KleinPerkins/chaos-scheduler/issues/197)) ([5066530](https://github.com/KleinPerkins/chaos-scheduler/commit/5066530b88bd1a1f5ae0a114be0283455a342b01))
* **dashboard:** execution slots (running vs configured capacity) per queue + global ([#196](https://github.com/KleinPerkins/chaos-scheduler/issues/196)) ([4699dcc](https://github.com/KleinPerkins/chaos-scheduler/commit/4699dcc6fade2e3cb8dc4595df2912af958a3007))
* **dashboard:** failure-recurrence per workflow + queue-health signals ([#191](https://github.com/KleinPerkins/chaos-scheduler/issues/191)) ([264f1cf](https://github.com/KleinPerkins/chaos-scheduler/commit/264f1cf648836bf5967e4fac39babbb17b55316f))
* **dashboard:** queue-utilization history sampler + (env, lookback) series ([#195](https://github.com/KleinPerkins/chaos-scheduler/issues/195)) ([650f2f8](https://github.com/KleinPerkins/chaos-scheduler/commit/650f2f8e1ff23cc0d76a20343e35e5c8bba223e4))
* **dashboard:** reconcile Activity feed into Mission Control IA ([#220](https://github.com/KleinPerkins/chaos-scheduler/issues/220)) ([8d124d5](https://github.com/KleinPerkins/chaos-scheduler/commit/8d124d5ec2133582315f24947d51d0d9e0f894e0))
* **dashboard:** rolling per-workflow p50 + mean runtime baselines ([#192](https://github.com/KleinPerkins/chaos-scheduler/issues/192)) ([ef283f7](https://github.com/KleinPerkins/chaos-scheduler/commit/ef283f7008da6d41fcf0e8421838807a3ebefcc8))
* **dashboard:** status-distribution counts keyed by (environment, lookback) ([#188](https://github.com/KleinPerkins/chaos-scheduler/issues/188)) ([b951bce](https://github.com/KleinPerkins/chaos-scheduler/commit/b951bcebaa9a3c1ea4232d9f533586508303f116))
* **dashboard:** success/fail trend buckets keyed by (environment, lookback) ([#189](https://github.com/KleinPerkins/chaos-scheduler/issues/189)) ([7fb79e8](https://github.com/KleinPerkins/chaos-scheduler/commit/7fb79e8a7d641eebbe786f64254c3fe31580aa7b))
* **dashboard:** wait+runtime trend buckets with 30-day trailing baseline ([#190](https://github.com/KleinPerkins/chaos-scheduler/issues/190)) ([9b579d0](https://github.com/KleinPerkins/chaos-scheduler/commit/9b579d0925003a28220ae372f2e8ef55484f7ed2))
* **dashboard:** week-over-week KPI deltas (current vs prior equal window) ([#193](https://github.com/KleinPerkins/chaos-scheduler/issues/193)) ([0b4a82a](https://github.com/KleinPerkins/chaos-scheduler/commit/0b4a82a09969b4f60d35a4967d5c59707cf7fbca))
* **dashboard:** windowed KPI summary keyed by (environment, lookback) ([#187](https://github.com/KleinPerkins/chaos-scheduler/issues/187)) ([94f7ec5](https://github.com/KleinPerkins/chaos-scheduler/commit/94f7ec549cd118fce03f37bb2b2e8bd3f6508d4e))
* **history:** add lean filtered log-free run-history read model ([#247](https://github.com/KleinPerkins/chaos-scheduler/issues/247)) ([c9db657](https://github.com/KleinPerkins/chaos-scheduler/commit/c9db657d9756ffabc7d8635af0121aedd08c1a60))
* **history:** lighten global history with bounded search ([#233](https://github.com/KleinPerkins/chaos-scheduler/issues/233)) ([01a1d73](https://github.com/KleinPerkins/chaos-scheduler/commit/01a1d73291472a707ab8f3f7daf49321ff0696f5))
* **history:** lighten workflow run history ([#235](https://github.com/KleinPerkins/chaos-scheduler/issues/235)) ([f07ac21](https://github.com/KleinPerkins/chaos-scheduler/commit/f07ac214a207451c8c1febe3f60e80a10a7e4f22))
* **history:** snapshot run-time environment provenance (schema v13) ([#251](https://github.com/KleinPerkins/chaos-scheduler/issues/251)) ([d88e928](https://github.com/KleinPerkins/chaos-scheduler/commit/d88e9284dbb39ede28b0c6ad9b90c136c6211e44))
* **lib:** add shared Lookback contract (type, presets, range resolver) ([#207](https://github.com/KleinPerkins/chaos-scheduler/issues/207)) ([3e31f7b](https://github.com/KleinPerkins/chaos-scheduler/commit/3e31f7b972b3640b458d36cdf4b15e069c2a4a37))
* **mcp-server:** deprecate run_workflow_now in favor of enqueue_workflow ([#267](https://github.com/KleinPerkins/chaos-scheduler/issues/267)) ([081ed6f](https://github.com/KleinPerkins/chaos-scheduler/commit/081ed6f6c9c9b627fd7cc50da0bf80d9f41b0750))
* **popup:** complete semantic and visual parity ([#238](https://github.com/KleinPerkins/chaos-scheduler/issues/238)) ([9d45f11](https://github.com/KleinPerkins/chaos-scheduler/commit/9d45f11d537e5a2155ad6125547c7a42687151c9))
* **popup:** complete tray mini-dashboard at 384x590 ([#269](https://github.com/KleinPerkins/chaos-scheduler/issues/269)) ([9fc45c8](https://github.com/KleinPerkins/chaos-scheduler/commit/9fc45c81ad78c9de3c51ec7edb9ea9e9c602b292))
* **popup:** queue upcoming runs via admission control ([#237](https://github.com/KleinPerkins/chaos-scheduler/issues/237)) ([6e0bbd3](https://github.com/KleinPerkins/chaos-scheduler/commit/6e0bbd3e22ad3091445015214395928ebf19a0af))
* **runs:** prioritize run observability ([#236](https://github.com/KleinPerkins/chaos-scheduler/issues/236)) ([a4e3cf2](https://github.com/KleinPerkins/chaos-scheduler/commit/a4e3cf204d65552db74299915ff568cbe284e839))
* **scheduler:** route rerun_workflow through admission control ([#263](https://github.com/KleinPerkins/chaos-scheduler/issues/263)) ([1ec0649](https://github.com/KleinPerkins/chaos-scheduler/commit/1ec064973de581b8d26d6801004cfec28df1be51))
* **sdk:** deprecate runWorkflow in favor of enqueueWorkflow ([#265](https://github.com/KleinPerkins/chaos-scheduler/issues/265)) ([4894ec5](https://github.com/KleinPerkins/chaos-scheduler/commit/4894ec5d6bfd5b8f77b4fd47336d077ef106bf70))
* **sidebar:** add collapsible navigation rail ([#223](https://github.com/KleinPerkins/chaos-scheduler/issues/223)) ([7238015](https://github.com/KleinPerkins/chaos-scheduler/commit/7238015cb942a24d736b8a70a9c0d6fb110ab1ea))
* **tokens:** add categorical data-viz color palette (8 hues, dark+light) ([#205](https://github.com/KleinPerkins/chaos-scheduler/issues/205)) ([75e2f55](https://github.com/KleinPerkins/chaos-scheduler/commit/75e2f553cc1ceede82c28f7c231baa4884640a72))
* **ui:** surface admission outcome when rerunning a run ([#264](https://github.com/KleinPerkins/chaos-scheduler/issues/264)) ([4780502](https://github.com/KleinPerkins/chaos-scheduler/commit/4780502bc94621a8fa5facc7da5cdd31ef56cdaa))
* **workflows:** add searchable workflow cards ([#230](https://github.com/KleinPerkins/chaos-scheduler/issues/230)) ([499681c](https://github.com/KleinPerkins/chaos-scheduler/commit/499681c63f97b5a4dfff63fb78a448baf0dbb72c))
* **workflows:** lighten workflow detail hierarchy ([#231](https://github.com/KleinPerkins/chaos-scheduler/issues/231)) ([77df0e5](https://github.com/KleinPerkins/chaos-scheduler/commit/77df0e565427651a4f2fcb5d2cad51c20e913248))
* **workflows:** lighten workflow editor hierarchy ([#232](https://github.com/KleinPerkins/chaos-scheduler/issues/232)) ([ab541e0](https://github.com/KleinPerkins/chaos-scheduler/commit/ab541e0afdbf2f4cb66d619e4fd6ba3046e66d4f))


### Bug Fixes

* **a11y:** announce async load failures with role=alert ([#248](https://github.com/KleinPerkins/chaos-scheduler/issues/248)) ([ddc5759](https://github.com/KleinPerkins/chaos-scheduler/commit/ddc5759f96a586f8475730cc9f485a32caf23e05))
* **a11y:** enforce status badge contrast ([#222](https://github.com/KleinPerkins/chaos-scheduler/issues/222)) ([2a09ddc](https://github.com/KleinPerkins/chaos-scheduler/commit/2a09ddc9a8dff2babdd05ff69062abd58c0601ac))
* **a11y:** name the Global History runs table ([#259](https://github.com/KleinPerkins/chaos-scheduler/issues/259)) ([da6034e](https://github.com/KleinPerkins/chaos-scheduler/commit/da6034efe5690847328e8849f4d65ec8bef03c2e))
* **a11y:** prevent low-contrast theme transitions ([#225](https://github.com/KleinPerkins/chaos-scheduler/issues/225)) ([c38f157](https://github.com/KleinPerkins/chaos-scheduler/commit/c38f15772b8b84bb0e76107496cbb070ea930fb9))
* **a11y:** remove duplicate Run Detail logs landmark ([#257](https://github.com/KleinPerkins/chaos-scheduler/issues/257)) ([cf11f68](https://github.com/KleinPerkins/chaos-scheduler/commit/cf11f688c001efbd57fe0d602ab52d291f10c97d))
* **a11y:** remove history and popup structure exceptions ([#221](https://github.com/KleinPerkins/chaos-scheduler/issues/221)) ([e49d29b](https://github.com/KleinPerkins/chaos-scheduler/commit/e49d29b3120dde96ac356ec85d9a93afcad02abf))
* **a11y:** trap focus in the shared Modal ([#261](https://github.com/KleinPerkins/chaos-scheduler/issues/261)) ([0df1501](https://github.com/KleinPerkins/chaos-scheduler/commit/0df1501d35f727d5b46b5b571af02e9dc12e0449))
* **history:** make failure-heatmap cells keyboard-accessible ([#244](https://github.com/KleinPerkins/chaos-scheduler/issues/244)) ([98adeff](https://github.com/KleinPerkins/chaos-scheduler/commit/98adeffae694dd5631a7ae311ae92e6885d166e8))
* **infotip:** dismiss on Escape regardless of how the tip opened ([#246](https://github.com/KleinPerkins/chaos-scheduler/issues/246)) ([2362046](https://github.com/KleinPerkins/chaos-scheduler/commit/236204653090947a19bf1f6a587ad55af64b4e49))
* **infotip:** raise glyph contrast to WCAG AA and un-suppress axe ([#249](https://github.com/KleinPerkins/chaos-scheduler/issues/249)) ([e78d0eb](https://github.com/KleinPerkins/chaos-scheduler/commit/e78d0ebdab870b2dfdcc322c9d7a251146380c04))
* **mission-control:** map warning status dot to the warning color (D04) ([#240](https://github.com/KleinPerkins/chaos-scheduler/issues/240)) ([1279117](https://github.com/KleinPerkins/chaos-scheduler/commit/12791172aef43d6041a37805b956d5e4df33ea3f))
* **release:** lift skip-cascade so partial releases still build+publish ([#250](https://github.com/KleinPerkins/chaos-scheduler/issues/250)) ([c1a6b9c](https://github.com/KleinPerkins/chaos-scheduler/commit/c1a6b9c3eafa441e7b9cce0d4d3de7dc771c2b68))
* **run-detail:** expose completed task status to assistive tech ([#239](https://github.com/KleinPerkins/chaos-scheduler/issues/239)) ([8dd93d3](https://github.com/KleinPerkins/chaos-scheduler/commit/8dd93d3d8b64f8077fb6477eef80328d03730793))
* **schedule:** stabilize builder contract ([#228](https://github.com/KleinPerkins/chaos-scheduler/issues/228)) ([cb69ad5](https://github.com/KleinPerkins/chaos-scheduler/commit/cb69ad57299bb87164bc1182a5c119aec02a3211))
* **status:** align dot semantics and styling ([#224](https://github.com/KleinPerkins/chaos-scheduler/issues/224)) ([8bf6f11](https://github.com/KleinPerkins/chaos-scheduler/commit/8bf6f11caa8ecb433b06ac8b363e4fb91133b6b0))
* **status:** unify run-status canonicalization across KPIs and scheduler gates ([#242](https://github.com/KleinPerkins/chaos-scheduler/issues/242)) ([3b000c9](https://github.com/KleinPerkins/chaos-scheduler/commit/3b000c90ae448bb9e200030bcdf70b3a7269b9f3))
* **theme:** synchronize preference consumers ([#226](https://github.com/KleinPerkins/chaos-scheduler/issues/226)) ([cc2f957](https://github.com/KleinPerkins/chaos-scheduler/commit/cc2f95722cea234b512e91cc6b4f8b8498870d07))
* **workflows:** persist selected environment ([#227](https://github.com/KleinPerkins/chaos-scheduler/issues/227)) ([0f3715b](https://github.com/KleinPerkins/chaos-scheduler/commit/0f3715ba2ca588e8678b1beb540dfb7a1d100566))
* **workflows:** unify manual queue execution ([#229](https://github.com/KleinPerkins/chaos-scheduler/issues/229)) ([e463168](https://github.com/KleinPerkins/chaos-scheduler/commit/e4631688989b82ed7912e5d76ec5e87b4430b9f9))


### Refactors

* remove dead trigger_workflow command ([#266](https://github.com/KleinPerkins/chaos-scheduler/issues/266)) ([190f709](https://github.com/KleinPerkins/chaos-scheduler/commit/190f709226d87c21fea1020996db8cd328a07d6c))
* **scheduler:** add dispatch_manual_run admission choke point ([#262](https://github.com/KleinPerkins/chaos-scheduler/issues/262)) ([3112c63](https://github.com/KleinPerkins/chaos-scheduler/commit/3112c63692ec08b366582f00e1a3bdc2bae1d770))
* **ui:** consolidate run/task duration formatting into shared util ([#206](https://github.com/KleinPerkins/chaos-scheduler/issues/206)) ([4dc45fd](https://github.com/KleinPerkins/chaos-scheduler/commit/4dc45fd8ffc0b9015689bcac2ba8868a5839429a))


### Documentation

* **agents:** record bespoke in-repo SVG chart architecture (D07) ([#243](https://github.com/KleinPerkins/chaos-scheduler/issues/243)) ([2dae8a5](https://github.com/KleinPerkins/chaos-scheduler/commit/2dae8a54bb14883c3c26ebed17a64843f105bd21))
* **design-system:** correct stale Code Connect + cs.* mirror claims ([#245](https://github.com/KleinPerkins/chaos-scheduler/issues/245)) ([12ffd9f](https://github.com/KleinPerkins/chaos-scheduler/commit/12ffd9f2b8dfa9984b45838d3fa50b4b2f3684e0))
* **design:** add G01 divergence ledger ([#241](https://github.com/KleinPerkins/chaos-scheduler/issues/241)) ([2935ff0](https://github.com/KleinPerkins/chaos-scheduler/commit/2935ff002666c3b7f455f0da06ab5ff0e0542bf5))
* **design:** refresh divergence ledger for landed P4/P5 work ([#252](https://github.com/KleinPerkins/chaos-scheduler/issues/252)) ([19995da](https://github.com/KleinPerkins/chaos-scheduler/commit/19995da8c69aa33276da973664810951b5816712))
* **ledger:** record G04 binding audit evidence ([#258](https://github.com/KleinPerkins/chaos-scheduler/issues/258)) ([9329508](https://github.com/KleinPerkins/chaos-scheduler/commit/9329508a8e61dbb9222c7ad74ade6c7095a0e262))
* **ledger:** record popup 384x590 ([#269](https://github.com/KleinPerkins/chaos-scheduler/issues/269)) + decision-3 completion ([#270](https://github.com/KleinPerkins/chaos-scheduler/issues/270)) ([df6525f](https://github.com/KleinPerkins/chaos-scheduler/commit/df6525fb7c9a23b4a6d72ca4817047896aab0137))
* reword manual-run guidance to queue-only (prefer enqueue) ([#268](https://github.com/KleinPerkins/chaos-scheduler/issues/268)) ([f43f173](https://github.com/KleinPerkins/chaos-scheduler/commit/f43f17394c87bdfaefa69557a252c6587bf3ac70))

## [1.2.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.1.0...chaos-scheduler-v1.2.0) (2026-07-10)


### Features

* **figma:** add Code Connect mappings for 10 masters + AGENTS.md fact fixes ([#174](https://github.com/KleinPerkins/chaos-scheduler/issues/174)) ([3cc958d](https://github.com/KleinPerkins/chaos-scheduler/commit/3cc958d6bc8e4ede5d42d491f04369f3d56ff69e))


### Documentation

* **agents:** reflect live Code Connect + CI auto-publish ([#176](https://github.com/KleinPerkins/chaos-scheduler/issues/176)) ([8117c8d](https://github.com/KleinPerkins/chaos-scheduler/commit/8117c8deee1bbf3a4304324eaf24f29ecba4888f))

## [1.1.0](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.0.4...chaos-scheduler-v1.1.0) (2026-07-09)


### Features

* **brand:** add monochrome menu-bar tray glyph ([#148](https://github.com/KleinPerkins/chaos-scheduler/issues/148)) ([21e4b4e](https://github.com/KleinPerkins/chaos-scheduler/commit/21e4b4e66148d4b1db91f99eaa722c4f5a3cb840))
* design-system token foundation + orbital-8 app icon ([#146](https://github.com/KleinPerkins/chaos-scheduler/issues/146)) ([b25ce59](https://github.com/KleinPerkins/chaos-scheduler/commit/b25ce594189594256ae120ce0c58e18ba4b9cce7))
* **tokens:** emit figma-tokens.json mirror manifest ([#149](https://github.com/KleinPerkins/chaos-scheduler/issues/149)) ([7f6929c](https://github.com/KleinPerkins/chaos-scheduler/commit/7f6929c41570655afb36ea20f8ab05f969998676))
* **ui:** extract Button primitive and add Code Connect mapping ([#155](https://github.com/KleinPerkins/chaos-scheduler/issues/155)) ([b19a3e5](https://github.com/KleinPerkins/chaos-scheduler/commit/b19a3e5d072635965118addef96c40d2bd236bd2))
* **ui:** extract EditorField component and migrate call sites ([#172](https://github.com/KleinPerkins/chaos-scheduler/issues/172)) ([03031a8](https://github.com/KleinPerkins/chaos-scheduler/commit/03031a88eb09e7db8c49ad88658cd741437b22ff))
* **ui:** extract EnvSelect primitive and add Code Connect mapping ([#167](https://github.com/KleinPerkins/chaos-scheduler/issues/167)) ([1e22691](https://github.com/KleinPerkins/chaos-scheduler/commit/1e22691b5c608069844e7f965229017639689c9b))
* **ui:** extract Field primitive ([#161](https://github.com/KleinPerkins/chaos-scheduler/issues/161)) ([2b05bf4](https://github.com/KleinPerkins/chaos-scheduler/commit/2b05bf4534581f2e4170adc50d26d0906d1f31b1))
* **ui:** extract Input primitive ([#159](https://github.com/KleinPerkins/chaos-scheduler/issues/159)) ([e4a7f90](https://github.com/KleinPerkins/chaos-scheduler/commit/e4a7f90e55df6d400be3f8df5698fb3254fc1c4b))
* **ui:** extract Modal shell primitive ([#162](https://github.com/KleinPerkins/chaos-scheduler/issues/162)) ([1d2b388](https://github.com/KleinPerkins/chaos-scheduler/commit/1d2b3884221175c9fdda7108d657908765d0eca5))
* **ui:** extract NavItem primitive and add Code Connect mapping ([#165](https://github.com/KleinPerkins/chaos-scheduler/issues/165)) ([a2b2c2f](https://github.com/KleinPerkins/chaos-scheduler/commit/a2b2c2fa513da0c7d23729f3598555e70449269b))
* **ui:** extract PageHeader component and migrate call sites ([#169](https://github.com/KleinPerkins/chaos-scheduler/issues/169)) ([06a369f](https://github.com/KleinPerkins/chaos-scheduler/commit/06a369f7757d3725ab9d473d7dca1e2e463a74f0))
* **ui:** extract RunsTable primitive and add Code Connect mapping ([#168](https://github.com/KleinPerkins/chaos-scheduler/issues/168)) ([249a0bf](https://github.com/KleinPerkins/chaos-scheduler/commit/249a0bf2a0a8f22537a1c63c930f6f9f5f14e25b))
* **ui:** extract Select primitive ([#160](https://github.com/KleinPerkins/chaos-scheduler/issues/160)) ([de8d737](https://github.com/KleinPerkins/chaos-scheduler/commit/de8d7377c649cf2d6c38b6701dfbae4f73a501b6))
* **ui:** extract SettingsCheck component and migrate call sites ([#171](https://github.com/KleinPerkins/chaos-scheduler/issues/171)) ([491c7d1](https://github.com/KleinPerkins/chaos-scheduler/commit/491c7d1fa93dc323f41f58fff494be11acbc86de))
* **ui:** extract SettingsField component and migrate call sites ([#170](https://github.com/KleinPerkins/chaos-scheduler/issues/170)) ([e4473f4](https://github.com/KleinPerkins/chaos-scheduler/commit/e4473f461604f21f5d7b1f44755f2681a20e7898))
* **ui:** extract Sidebar primitive and add Code Connect mapping ([#166](https://github.com/KleinPerkins/chaos-scheduler/issues/166)) ([a52a9a2](https://github.com/KleinPerkins/chaos-scheduler/commit/a52a9a2b55917eca23f4ddcaa6a6e6a93f63cf96))
* **ui:** extract StatCard primitive and add Code Connect mapping ([#164](https://github.com/KleinPerkins/chaos-scheduler/issues/164)) ([5df6a45](https://github.com/KleinPerkins/chaos-scheduler/commit/5df6a45c51d64368183bf66ab02908da5851a33e))
* **ui:** extract StatusBadge primitive and add Code Connect mapping ([#156](https://github.com/KleinPerkins/chaos-scheduler/issues/156)) ([a0fce59](https://github.com/KleinPerkins/chaos-scheduler/commit/a0fce593510d10f1d53fafcd28c76b2e04c65375))
* **ui:** extract StatusDot primitive ([#158](https://github.com/KleinPerkins/chaos-scheduler/issues/158)) ([1ed3129](https://github.com/KleinPerkins/chaos-scheduler/commit/1ed31296cd832c78ed226242cbb79ce3c16d97fb))
* **ui:** extract Textarea primitive ([#163](https://github.com/KleinPerkins/chaos-scheduler/issues/163)) ([061019f](https://github.com/KleinPerkins/chaos-scheduler/commit/061019f16a176e2226aad6c0f5ea98853c8a68b0))

## [1.0.4](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.0.3...chaos-scheduler-v1.0.4) (2026-07-07)


### Bug Fixes

* **release:** guard updater latest during desktop build gap ([#144](https://github.com/KleinPerkins/chaos-scheduler/issues/144)) ([014c146](https://github.com/KleinPerkins/chaos-scheduler/commit/014c146afa8bfef47b1b86a3bf53cb971ef5c820))

## [1.0.3](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.0.2...chaos-scheduler-v1.0.3) (2026-07-07)


### Bug Fixes

* **release:** pass release tags to release.yml as strings, not coerced booleans ([#142](https://github.com/KleinPerkins/chaos-scheduler/issues/142)) ([0ca02c4](https://github.com/KleinPerkins/chaos-scheduler/commit/0ca02c4b1f7b08b0755b6425e4e964cbc268a63d))
* **security:** remove polynomial-time regexes flagged by CodeQL ([#139](https://github.com/KleinPerkins/chaos-scheduler/issues/139)) ([edbb395](https://github.com/KleinPerkins/chaos-scheduler/commit/edbb3950793058f6087a5a454653826b4ee2aa89))
* **test:** avoid dynamic regexes in build output assertions ([#141](https://github.com/KleinPerkins/chaos-scheduler/issues/141)) ([8e98009](https://github.com/KleinPerkins/chaos-scheduler/commit/8e980094e6bae7601ca30990a3239535babbad00))

## [1.0.2](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.0.1...chaos-scheduler-v1.0.2) (2026-07-07)


### Bug Fixes

* **release:** gate release build on tag_name, not the unreliable release_created ([#137](https://github.com/KleinPerkins/chaos-scheduler/issues/137)) ([7d2dd64](https://github.com/KleinPerkins/chaos-scheduler/commit/7d2dd64e1ff369c426c9952b0eb7f200c95a1b85))

## [1.0.1](https://github.com/KleinPerkins/chaos-scheduler/compare/chaos-scheduler-v1.0.0...chaos-scheduler-v1.0.1) (2026-07-07)


### Bug Fixes

* **mcp-server:** keep @chaos-scheduler/sdk external so SDK hotfixes reach users ([#136](https://github.com/KleinPerkins/chaos-scheduler/issues/136)) ([75342a9](https://github.com/KleinPerkins/chaos-scheduler/commit/75342a96a3b227f11e252ce8bfd40128409ff705))
* **mcp:** persist and broadcast late-stage provisioning failures ([#133](https://github.com/KleinPerkins/chaos-scheduler/issues/133)) ([ccf1019](https://github.com/KleinPerkins/chaos-scheduler/commit/ccf10194ab2b2eeb9557e3f9453803fefa0a7fd7))
* **test:** de-flake cursor_agent poll tests blocking CI ([#135](https://github.com/KleinPerkins/chaos-scheduler/issues/135)) ([0deb0cd](https://github.com/KleinPerkins/chaos-scheduler/commit/0deb0cd30f6047050918e8075e9c14bc1a38d96a))

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
