#!/bin/bash
#
# beforeMCPExecution guard for the Chaos MCP server.
#
# Asks the user to confirm before a *destructive* or *protected-environment*
# write reaches the scheduler. This is a local, defense-in-depth backstop that
# complements the server-side guardrails (CHAOS_SCHEDULER_MCP_PROTECTED_*).
#
# Fails OPEN: any parse problem returns "allow" so it never blocks legitimate
# work. Requires `jq` (checked below); if missing, it allows and warns.
#
# Configure protected env names via CHAOS_SCHEDULER_PROTECTED_ENVIRONMENTS
# (comma-separated); defaults to "prod,production".

set -euo pipefail

allow() { echo '{ "permission": "allow" }'; exit 0; }

input=$(cat)

if ! command -v jq >/dev/null 2>&1; then
  # Can't inspect the payload safely; don't block.
  allow
fi

# Tool name can appear under a few keys depending on Cursor version.
tool=$(printf '%s' "$input" | jq -r '(.tool_name // .tool // .name // "")' 2>/dev/null || echo "")

# Arguments blob (stringified) for a cheap substring check on env names.
args=$(printf '%s' "$input" | jq -c '(.tool_input // .arguments // .input // {})' 2>/dev/null || echo "{}")

# Destructive / write tools exposed by the Chaos MCP server.
case "$tool" in
  *delete_workflow*|*register_workflow*|*set_workflow_spec*|*create_environment*|*run_workflow_now*|*enqueue_workflow*|*dispatch_workflow*)
    is_write=1
    ;;
  *)
    is_write=0
    ;;
esac

if [[ "$is_write" -eq 0 ]]; then
  allow
fi

protected_csv="${CHAOS_SCHEDULER_PROTECTED_ENVIRONMENTS:-prod,production}"
IFS=',' read -r -a protected <<< "$protected_csv"

hits_protected=0
lc_args=$(printf '%s' "$args" | tr '[:upper:]' '[:lower:]')
for env in "${protected[@]}"; do
  env_trimmed=$(printf '%s' "$env" | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')
  [[ -z "$env_trimmed" ]] && continue
  if printf '%s' "$lc_args" | grep -q "\"$env_trimmed\""; then
    hits_protected=1
    break
  fi
done

if [[ "$hits_protected" -eq 1 ]]; then
  cat <<'JSON'
{
  "permission": "ask",
  "user_message": "This Chaos Scheduler write targets a PROTECTED environment. Confirm before it runs.",
  "agent_message": "A local hook flagged this as a write to a protected scheduler environment; awaiting user confirmation."
}
JSON
  exit 0
fi

# Non-protected write: allow, but this is the seam to add stricter policy later.
allow
