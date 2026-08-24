# Credential-Security Hardening — Chaos Scheduler

**Version:** v1
**Status:** DRAFT — 2026-08-23
**Date:** 2026-08-23
**Owning repository:** `KleinPerkins/chaos-scheduler` (`~/dev/personal/chaos-scheduler`)
**Plan-type:** standard
**Review provenance:** Consolidated from a read-only, multi-reviewer credential-security audit (2026-07-14, plan-mode; five parallel explore reviewers over inbound/outbound webhooks, the Cursor/agent path, the SDK, and the remote HTTP MCP), re-anchored to an enterprise "corporate-managed individual laptop" threat model. The managed-key scope and the MCP tool-vs-resource redaction asymmetry were **re-verified against current `main`** (file:line below); the remaining findings are cited to code/PRs/issue #292 and **must be re-verified at dispatch**. **At-rest-encryption posture DECIDED 2026-08-23:** Option A (FileVault-stack, implemented in PR-C) adopted as the chosen at-rest posture; PR-E / Option B (envelope encryption) deferred as optional — revisit only if Affirm IT/security mandates application-level at-rest encryption beyond FileVault.

**Goal:** Close the credential-exposure gaps that a company IT/security review would flag for an individual using Chaos Scheduler on a managed laptop — chiefly stopping workflow secrets from reaching agent/LLM context via MCP, hardening secrets at rest, gating secrets out of git, and giving an auditable revoke/offboard path — without moving business logic out of `SchedulerService` or adding a secret-manager to paths that do not need one.
**Architecture:** Additive, defence-in-depth hardening delivered as small independently-revertible slices (PR-A…PR-E) layered on the existing surfaces; the foundational managed-MCP-key → macOS Keychain migration (tracked under security issue #292) is the base this plan amends. Redaction is treated as a system invariant with a coverage test; at-rest protection is FileVault + file-hardening by default, with app-level envelope encryption as an optional, operator-gated escalation.
**Tech stack:** Rust/Tauri backend (`src-tauri/src/`: `mcp.rs`, `service.rs`, `api.rs`, `actions.rs`, `db.rs`), the MCP server (`packages/mcp-server/src/`: `server.ts`, `resource-projection.ts`), the TypeScript SDK (`packages/sdk-ts`), macOS Keychain + the signed launcher, `SECURITY.md`, and a secret-scanning CI/pre-commit gate.

---

## Global Constraints

Project-wide requirements that implicitly apply to every work item below:

- **Service boundary (ADR-0001):** all governed logic stays in `SchedulerService`; MCP/SDK/REST/IPC stay thin. Redaction lives on the service/read path, not duplicated per adapter.
- **App-owned provisioning (ADR-0004/0005):** the MCP server and SDK are version-pinned provisioned artifacts; credential changes ship with the build, never via floating `npx` or DMG-bundled config.
- **Fix-agent invariants (ADR-0009):** the D05 fix-agent stays propose-only/born-draft; any Cursor-Cloud/agent token handling changed here must not weaken it.
- **Append-only migrations (ADR-0003):** any schema/read-model change (e.g. an audit read view) is a new numbered, transactional migration; never mutate a shipped migration.
- **No secret ever printed, hashed for display, or committed.** Symmetric secrets (webhook HMAC keys) must stay runtime-usable and therefore cannot be one-way hashed like API keys.
- **Delivery:** Conventional Commits; never `--no-verify`; land via the GitHub git-data API single-commit PR pattern (working tree stays pristine); `main` is protected (PR + `ci-required` + linear history + 1 approving review satisfied by the `chaos-scheduler-automerge` App).
- **Baseline at rest:** FileVault full-disk encryption is assumed present on the managed device; this plan hardens the layers above it.

## 1. Purpose

A prior read-only audit found that, although Chaos Scheduler already does many things well (constant-time API-key and webhook comparison, replay protection, unconditional SMTP-password masking, `GITHUB_TOKEN`/`GH_TOKEN` scrubbing in the local fix-agent, and `chaos://` resource projection redaction #296), several credential-exposure paths would not pass an enterprise IT/security review for an individual on a managed laptop. The highest-impact path is that the **managed Cursor MCP integration mints a write-scoped key**, so everyday MCP **tool** reads return **unredacted** workflow secrets straight into agent/LLM context (which is transmitted off-device to a model vendor). This plan consolidates those findings into one governed, dispatchable workstream so the hardening is durable and reviewable rather than living only in a local plan artifact and open issue #292.

## 2. Scope and non-goals

**In scope:**

- **PR-A — Redaction as an invariant.** Redact MCP **tool** reads (`list_workflows`, `get_workflow`) to match the `chaos://` **resource** redaction, regardless of key scope, plus a redaction-coverage test asserting no read surface emits a raw secret.
- **PR-B — Secret-scan gate.** A CI secret-scan (e.g. gitleaks) + a pre-commit hook; an e2e/artifact scan asserting no known test secret appears in the built app, process env/argv, or config files.
- **PR-C — At-rest hardening (FileVault-stack).** `0600` on the DB and backups, `0700` on app-data; `secure_delete` + VACUUM-on-delete; Time-Machine/cloud-sync exclusion; `.bak` sidecars covered by `.gitignore`.
- **PR-D — Audit + offboarding + least-privilege.** A read-only `api_audit_log` access view; a one-action revoke-all-keys / purge-secrets offboarding path; default the managed Cursor MCP key to **read-only**, elevating to write only during an explicit authoring action.
- **PR-E — Optional envelope encryption (DEFERRED, staged after the Keychain foundation).** Encrypt secrets at rest with a Keychain-held data key, only if IT requires application-level encryption beyond FileVault.
- **Documentation/caller-responsibility fixes:** correct `SECURITY.md`/SDK-README language on MCP-tool-vs-resource redaction, the SDK env-var key being the embedding app's responsibility, the `inbound_webhook_secret` setter/HMAC status, the `fix_agent_dispatches.detail` doc/code mismatch, and remove the "Add to Cursor" argv key-passing flow.

**Non-goals:**

- Moving every symmetric webhook/SMTP secret into Keychain, or whole-DB SQLCipher (justified: HMAC secrets must be runtime-usable; FileVault + `0600` + `secure_delete` + no-sync + redaction is a defensible baseline). Envelope encryption is offered as PR-E only.
- Re-opening the D05 fix-agent posture (ADR-0009) or the admission-control choke point (ADR-0007).
- The design-to-code UI roadmap (`design-to-code-completion-v1.md`); this is a distinct security subsystem.
- Any change to the remote HTTP MCP's client-managed credential model (caller-owned; documented, not enforced).

## 3. Background and context

Verified against current `main` where cited; other findings are audit-reported (2026-07-14) and must be re-verified at dispatch.

- **Managed MCP key is write-scoped (re-verified).** `src-tauri/src/mcp.rs:817` mints the managed integration key with `&["read", "write"]`. The MCP **tools** `list_workflows` and `get_workflow` (`packages/mcp-server/src/server.ts`) return the client result directly, while only the `chaos://` **resource** handlers apply `projectWorkflowsForResource` / `projectWorkflowForResource`. `SECURITY.md` documents that `read`-scope callers are redacted but `write`/`admin` callers receive full secrets for round-trip edits — so the write-scoped managed key means everyday tool reads surface unredacted workflow secrets (chiefly the outbound-webhook HMAC `secret`) into agent/LLM context. This is the top finding and the core rationale for PR-A + the read-only default in PR-D.
- **Already-secure (keep, do not regress):** constant-time API-key hashing/verification with real revocation + `api_audit_log`; inbound-webhook constant-time compare + TTL/event-id replay protection + canonical body; unconditional `mask_email_profile`; outbound HMAC-SHA256 signing; local fix-agent credential scrubbing; `chaos://` resource projection redaction (#296); no real secret ever committed (the one tracked plaintext in `.cursor/mcp.json` is issue #292's subject, addressed by the Keychain foundation). PR #328 added honest diff-presentation only.
- **Audit-reported gaps to re-verify:** no secret-scanning in CI; `api_audit_log` has no production reader; only `0o755` `set_permissions` (no `0600`/`secure_delete`/backup-exclusion); `inbound_webhook_secret` has no setter (the `/dispatch` path is bearer-token-only, not the HMAC the docs imply); outbound-webhook URLs are never redacted and there is no dedicated auth-header field (credentials embedded in a URL persist in `action_dead_letters` and error text); `run_tasks.details` for the `cursor_agent` operator persists unredacted stderr readable via REST/IPC/MCP; `fix_agent_dispatches.detail` stores capped raw stdout despite a comment claiming otherwise (internal-only today, no read path).
- **Anchor:** security issue #292 (rotate the exposed scheduler API credential + make the tracked MCP config secret-free). The **managed-key → Keychain foundation is LANDED** (this plan's base): the managed key now lives in the macOS Keychain resolved by an app-owned launcher, the tracked `.cursor/mcp.json` is untracked + git-ignored with a secret-free example and a `ci-required` credential guard, offboarding is Keychain-aware, and two PR-D offboarding follow-ups shipped alongside it — the `AtomicUsize` refcount/generation offboarding guard and the unparseable-`~/.cursor/mcp.json` backup-and-replace (see ADR 0010). Live-key rotation + Git-history cleanup remain the operator's separate incident-response work.

## 4. Approach

- **Slice by concern, not by file.** PR-A…PR-E are independently reviewable and revertible; each ships a failing-first test. PR-A (off-device egress) is the highest risk-reduction-per-effort and goes first; PR-B and PR-C are independent; PR-D depends on the Keychain foundation (#292); PR-E is optional and staged after it.
- **Redaction as an enforced invariant.** A single coverage-matrix test enumerates every read surface (REST, IPC, MCP tools, MCP resources) and asserts none emits a raw secret for a fixture workflow, so a future read path cannot silently regress.
- **Least privilege for the everyday session.** Combined with PR-A, a read-only default managed key makes the routine managed MCP session both minimal-scope and redacted; write is elevated only during an explicit authoring action, and secret-preserving edits continue to route through the existing `patch_workflow_spec` merge-preserve-sentinel mechanism (which fetches the real secret server-side, never exposing it to the caller).
- **At rest: FileVault + hardening by default (Option A, DECIDED); envelope encryption only if Affirm IT mandates it (Option B, DEFERRED).** See §5 (PR-E).
- **Delivery:** git-data-API single-commit PRs, one concern per PR, merged via the automerge App; `SECURITY.md` updated in lockstep so its claims match the code.

## 5. Work items

### PR-A — Redact MCP tool reads (redaction invariant)

- **Objective:** Apply the `chaos://` resource projection redaction to the `list_workflows` and `get_workflow` MCP **tool** outputs, regardless of key scope, and lock it with a redaction-coverage test.
- **Scope:** `packages/mcp-server/src/server.ts` (wrap the two tool handlers with `projectWorkflowsForResource` / `projectWorkflowForResource`); a new coverage-matrix test asserting no read surface emits a raw secret; `SECURITY.md` wording corrected so "MCP tools are always redacted" is true. `patch_workflow_spec`'s internal server-side fetch stays unaffected so secret-preserving edits still work.
- **Prerequisites:** re-confirm the tool handlers and REST redaction semantics on the dispatch-time `main`.
- **DoD:** a write-scoped key can no longer read a raw workflow secret through `list_workflows`/`get_workflow`; the coverage test fails first, then passes; edit round-trips via `patch_workflow_spec` are unchanged.

### PR-B — Secret-scan CI + pre-commit gate

- **Objective:** Prevent any secret from reaching git and prove the built artifact carries none.
- **Scope:** a CI secret-scan job (e.g. gitleaks) + a pre-commit hook (via the existing lefthook config); an e2e/artifact scan asserting no known test secret appears in the built app bundle, running process env/argv, or config files.
- **DoD:** a planted test secret fails CI and the pre-commit hook; the artifact scan is green on a clean build.

### PR-C — At-rest hardening (FileVault-stack)

- **Objective:** Harden secret-bearing files above FileVault.
- **Scope:** `src-tauri/src/db.rs` (+ callers) — `0600` on the SQLite DB and `.bak` backups, `0700` on app-data; enable `secure_delete` and VACUUM-on-delete; exclude the DB/backups from Time Machine and document cloud-sync exclusion; extend `.gitignore` to cover `.bak` sidecars.
- **DoD:** new DB/backup files are `0600`; deletes are secure; backups carry the exclusion attribute; a test asserts the permission/pragma settings.

### PR-D — Audit + offboarding + least-privilege

- **Objective:** Give IT-grade visibility and a clean revoke/offboard path, and shrink the managed key's blast radius.
- **Scope:** a read-only `api_audit_log` access view (new append-only migration + a read path/command); a one-action revoke-all-keys / purge-secrets offboarding flow; default the managed Cursor MCP key to **read-only** (`src-tauri/src/mcp.rs`), elevating to write only during an explicit authoring action.
- **Prerequisites:** the managed-key → Keychain foundation (#292).
- **DoD:** the audit log is readable through the product; one action revokes all keys and purges secrets/Keychain items; the everyday managed session is read-only + redacted, with write elevated only on authoring.
- **Offboarding follow-ups — COMPLETED (landed with #292).** Two hardening fixes on the one-action offboard path shipped alongside the Keychain foundation: (a) the offboarding minting-disabled gate is now an `AtomicUsize` refcount/generation counter (not a boolean), so two overlapping offboards keep key minting disabled until every in-flight offboard completes; and (b) an unparseable `~/.cursor/mcp.json` is now backed up to a `.bak` sidecar and replaced with a scrubbed, valid, token-free config, so the managed token is scrubbed even when the config can't be parse-merged, and the offboarding tri-state additionally requires the managed Keychain item proven-absent. Each carries a failing-first test.

### PR-E — Optional envelope encryption (DEFERRED)

- **DECIDED 2026-08-23 — Option A (FileVault-stack) is the chosen at-rest posture.** PR-C implements this baseline. PR-E / Option B is deferred as optional and should only be revisited if Affirm IT/security ever mandates application-level at-rest encryption beyond FileVault.
  - **Option A — FileVault-stack (PR-C; chosen).** Covers powered-off device loss, other-local-user reads, forensic residue, and backup/sync exfil at near-zero cost and with no data migration; HMAC secrets remain runtime-usable. Does not defend against malware already running as the same user on an unlocked Mac — a narrow threat that falls outside the practical scope of a personal-use tool.
  - **Option B — envelope encryption (this PR-E; DEFERRED / optional).** Secrets would be AEAD-encrypted at rest under a Keychain-held data key (same trust boundary established by the Keychain foundation); a copied/backed-up DB becomes ciphertext and same-user malware must also pull the ACL-bound Keychain key. The only added protection over Option A is against malware already running as the same user on an unlocked Mac. Cost: a one-time reversible migration + secure-wipe of pre-migration `.bak`s, a startup key fetch, and ongoing AEAD/rotation/failure-mode complexity. **Revisit trigger:** Affirm IT/security mandates application-level at-rest encryption beyond FileVault.
- **DoD (only if Option B is later adopted):** secrets are AEAD-encrypted at rest under a Keychain data key; the migration is transactional and reversible; pre-migration backups are securely wiped; startup fetch and rotation are tested.

### Documentation / caller-responsibility fixes (fold into the nearest slice)

- Correct `SECURITY.md`/SDK-README on MCP-tool-vs-resource redaction (PR-A) and that the SDK/remote-MCP env-var key is the embedding app's responsibility (Chaos Scheduler cannot secure the caller's environment).
- Resolve the `inbound_webhook_secret` gap: add a setter (and real HMAC on `/dispatch`) **or** correct the docs to state `/dispatch` is bearer-token-authenticated.
- Document outbound-webhook URLs as caller-owned (do not embed credentials); file a follow-up for a dedicated auth-header/bearer field or URL-query redaction, plus an `action_dead_letters` redaction guard/test.
- File the `run_tasks.details` operator-output redaction gap as its own security issue (broader than credentials).
- Fix the `fix_agent_dispatches.detail` comment to match reality (stores capped raw stdout in local mode; internal-only).
- Remove the "Add to Cursor" flow that passes the API key as a base64 argv to `open` (reinforces the Keychain-only decision).

## 6. Acceptance criteria

- **PR-A:** no MCP tool read returns a raw workflow secret for any key scope; redaction-coverage test green; edit round-trips unaffected.
- **PR-B:** a planted secret fails CI + pre-commit; artifact scan green.
- **PR-C:** DB/backups `0600`, app-data `0700`, `secure_delete` on, backups Time-Machine-excluded, `.bak` gitignored; asserted by test.
- **PR-D:** `api_audit_log` readable in-product; one-action revoke/purge works; managed key read-only-by-default with elevate-on-authoring.
- **PR-E (deferred / optional — revisit only if Affirm IT mandates app-level encryption beyond FileVault):** secrets AEAD-encrypted at rest under a Keychain key; migration reversible; pre-migration backups wiped; rotation tested.
- **Program:** `SECURITY.md` claims match the code; issue #292's Keychain foundation landed; ADR-0009 fix-agent invariants intact; at-rest-encryption posture recorded as Option A (FileVault-stack, decided 2026-08-23).

## 7. Rollback

- **Per slice:** each ships as one concern per PR — revert the offending PR. PR-A/PR-B/PR-C are pure additions (redaction, a CI gate, file-permission/pragma settings) and revert cleanly.
- **PR-D:** the `api_audit_log` read view is an append-only migration (never rolled back — a forward migration is used if a change is needed); the revoke/purge action and the read-only-key default are behavioral and revertible.
- **PR-E:** the envelope-encryption migration is transactional and reversible (repo/DB remains the source of truth); pre-migration backups are securely wiped only after a verified migration.
- **This plan:** DRAFT and reversible until ACCEPTED; supersede via a new version rather than editing in place once accepted.
