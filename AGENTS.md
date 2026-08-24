## Continual-learning memory

Machine-mined preferences and workspace facts for this workspace now live in
`agent-learnings.md` (machine-local, git-ignored, never committed). Agents load it as
ancestor context when present; this file holds only hand-authored, human-curated guidance.

<!-- doc-warden:begin -->

## Documentation governance

This repo's documentation structure is governed by `doc-warden`.

| Artifact type | Location                                                               |
| ------------- | ---------------------------------------------------------------------- |
| Plans         | `docs/plans/` — index at `docs/plans/README.md`                        |
| ADRs          | `docs/adr/` — index at `docs/adr/README.md`                            |
| Runbooks      | `docs/runbooks/`                                                       |
| Glossary      | `docs/GLOSSARY.md` (created when genuine terminology ambiguity arises) |

**Policy:** `~/dev/_ops/doc-warden/POLICY.md`
**Engine:** `~/dev/_ops/bin/doc-warden.sh check chaos-scheduler` (read-only health check)
<!-- doc-warden:end -->
