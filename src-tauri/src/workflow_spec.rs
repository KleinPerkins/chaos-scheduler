//! Workflow specification model (generic step-flow vs typed operator) plus
//! registration-time validation. Specs are persisted in `workflows.spec_json`
//! (consistent with the existing `trigger_config`/`queue_config` blobs) and the
//! generic step-flow is executed in-scheduler by [`crate::steps`].

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Which execution model a workflow uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    /// User-defined multi-step DAG of arbitrary commands/scripts.
    Generic,
    /// A single built-in operator (git_pull, cursor_agent, …).
    Typed,
}

impl WorkflowKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowKind::Generic => "generic",
            WorkflowKind::Typed => "typed",
        }
    }
}

/// Per-step retry policy.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryPolicy {
    #[serde(default)]
    pub max_retries: u32,
    #[serde(default)]
    pub backoff_seconds: u64,
}

/// A single step in a generic workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSpec {
    pub id: String,
    /// A full shell command line (mutually exclusive with `script`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// A script path (resolved against `working_dir`/workspace root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// If true, a failure of this step does not fail the run (continue vs
    /// fail-fast).
    #[serde(default)]
    pub continue_on_error: bool,
}

/// The generic step-flow body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericSpec {
    pub steps: Vec<StepSpec>,
}

/// The typed-operator body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedSpec {
    pub operator_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Full workflow spec stored in `spec_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub kind: WorkflowKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic: Option<GenericSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed: Option<TypedSpec>,
    /// On-success / on-failure actions (Phase 5). Kept here so the spec is the
    /// single registration payload.
    #[serde(default)]
    pub on_success: Vec<crate::actions::ActionSpec>,
    #[serde(default)]
    pub on_failure: Vec<crate::actions::ActionSpec>,
}

impl WorkflowSpec {
    /// Parse a spec from its JSON blob.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid workflow spec JSON: {e}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Validate the spec structurally. For generic specs this checks step id
    /// uniqueness, that each step has exactly one of command/script, that
    /// `depends_on` references exist, and that the dependency graph is acyclic.
    pub fn validate(&self) -> Result<(), String> {
        match self.kind {
            WorkflowKind::Generic => {
                let generic = self
                    .generic
                    .as_ref()
                    .ok_or("generic workflow requires a `generic` body")?;
                validate_generic(generic)?;
            }
            WorkflowKind::Typed => {
                let typed = self
                    .typed
                    .as_ref()
                    .ok_or("typed workflow requires a `typed` body")?;
                if typed.operator_type.trim().is_empty() {
                    return Err("typed workflow requires a non-empty operator_type".into());
                }
            }
        }
        for action in self.on_success.iter().chain(self.on_failure.iter()) {
            action.validate()?;
        }
        Ok(())
    }
}

fn validate_generic(generic: &GenericSpec) -> Result<(), String> {
    if generic.steps.is_empty() {
        return Err("generic workflow requires at least one step".into());
    }
    let mut ids = HashSet::new();
    for step in &generic.steps {
        if step.id.trim().is_empty() {
            return Err("every step requires a non-empty id".into());
        }
        if !ids.insert(step.id.clone()) {
            return Err(format!("duplicate step id: {}", step.id));
        }
        let has_command = step.command.as_ref().is_some_and(|c| !c.trim().is_empty());
        let has_script = step.script.as_ref().is_some_and(|s| !s.trim().is_empty());
        if has_command == has_script {
            return Err(format!(
                "step '{}' must specify exactly one of `command` or `script`",
                step.id
            ));
        }
    }
    // Referential integrity of depends_on.
    for step in &generic.steps {
        for dep in &step.depends_on {
            if !ids.contains(dep) {
                return Err(format!(
                    "step '{}' depends on unknown step '{}'",
                    step.id, dep
                ));
            }
            if dep == &step.id {
                return Err(format!("step '{}' cannot depend on itself", step.id));
            }
        }
    }
    // Acyclicity via topological sort.
    topological_order(&generic.steps).map(|_| ())
}

/// Return a valid execution order (steps before their dependents), or an error
/// if the dependency graph contains a cycle. Deterministic: ties broken by the
/// original step order.
pub fn topological_order(steps: &[StepSpec]) -> Result<Vec<String>, String> {
    let mut indegree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    let order: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
    for step in steps {
        indegree.entry(step.id.as_str()).or_insert(0);
        for dep in &step.depends_on {
            *indegree.entry(step.id.as_str()).or_insert(0) += 1;
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(step.id.as_str());
        }
    }
    // Seed with zero-indegree nodes in original order (stable).
    let mut ready: Vec<&str> = order
        .iter()
        .copied()
        .filter(|id| indegree.get(id).copied().unwrap_or(0) == 0)
        .collect();
    let mut result = Vec::with_capacity(steps.len());
    while let Some(node) = ready.first().copied() {
        ready.remove(0);
        result.push(node.to_string());
        if let Some(children) = dependents.get(node) {
            let mut newly_ready: Vec<&str> = Vec::new();
            for &child in children {
                let entry = indegree.get_mut(child).unwrap();
                *entry -= 1;
                if *entry == 0 {
                    newly_ready.push(child);
                }
            }
            // Preserve original order among newly-ready nodes.
            for id in order.iter() {
                if newly_ready.contains(id) {
                    ready.push(id);
                }
            }
        }
    }
    if result.len() != steps.len() {
        return Err("workflow step graph contains a cycle".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str, deps: &[&str]) -> StepSpec {
        StepSpec {
            id: id.to_string(),
            command: Some("echo hi".to_string()),
            script: None,
            args: vec![],
            working_dir: None,
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            retry: None,
            timeout_seconds: None,
            continue_on_error: false,
        }
    }

    #[test]
    fn topological_order_respects_dependencies() {
        let steps = vec![step("c", &["b"]), step("b", &["a"]), step("a", &[])];
        let order = topological_order(&steps).unwrap();
        let pos = |id: &str| order.iter().position(|x| x == id).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("b") < pos("c"));
    }

    #[test]
    fn cycle_is_detected() {
        let steps = vec![step("a", &["b"]), step("b", &["a"])];
        assert!(topological_order(&steps).is_err());
    }

    #[test]
    fn generic_spec_requires_command_xor_script() {
        let spec = WorkflowSpec {
            kind: WorkflowKind::Generic,
            environment: None,
            generic: Some(GenericSpec {
                steps: vec![StepSpec {
                    id: "s1".into(),
                    command: Some("echo".into()),
                    script: Some("s.py".into()),
                    args: vec![],
                    working_dir: None,
                    depends_on: vec![],
                    retry: None,
                    timeout_seconds: None,
                    continue_on_error: false,
                }],
            }),
            typed: None,
            on_success: vec![],
            on_failure: vec![],
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn unknown_dependency_is_rejected() {
        let spec = WorkflowSpec {
            kind: WorkflowKind::Generic,
            environment: None,
            generic: Some(GenericSpec {
                steps: vec![step("a", &["missing"])],
            }),
            typed: None,
            on_success: vec![],
            on_failure: vec![],
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn typed_spec_requires_operator_type() {
        let spec = WorkflowSpec {
            kind: WorkflowKind::Typed,
            environment: None,
            generic: None,
            typed: Some(TypedSpec {
                operator_type: "  ".into(),
                config: serde_json::json!({}),
            }),
            on_success: vec![],
            on_failure: vec![],
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn valid_generic_spec_round_trips() {
        let spec = WorkflowSpec {
            kind: WorkflowKind::Generic,
            environment: Some("production".into()),
            generic: Some(GenericSpec {
                steps: vec![step("build", &[]), step("test", &["build"])],
            }),
            typed: None,
            on_success: vec![],
            on_failure: vec![],
        };
        spec.validate().unwrap();
        let json = spec.to_json();
        let parsed = WorkflowSpec::from_json(&json).unwrap();
        parsed.validate().unwrap();
    }
}
