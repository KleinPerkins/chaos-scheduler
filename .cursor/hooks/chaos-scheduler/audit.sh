#!/bin/bash
#
# afterMCPExecution audit for the Chaos MCP server.
#
# Appends a one-line JSONL audit record for every chaos-scheduler MCP call to
# .cursor/hooks/chaos-scheduler/audit.log (git-ignored). Read-only side effect;
# always allows. Requires `jq`; if missing, it no-ops.

set -euo pipefail

input=$(cat)
log_dir="$(dirname "$0")"
log_file="$log_dir/audit.log"

if command -v jq >/dev/null 2>&1; then
  tool=$(printf '%s' "$input" | jq -r '(.tool_name // .tool // .name // "unknown")' 2>/dev/null || echo "unknown")
  ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  printf '{"ts":"%s","tool":"%s"}\n' "$ts" "$tool" >> "$log_file" 2>/dev/null || true
fi

# afterMCPExecution has no permission gate; emit nothing / empty object.
echo '{}'
exit 0
