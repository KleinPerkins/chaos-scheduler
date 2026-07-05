//! On-success / on-failure action framework.
//!
//! Actions are declared in a workflow's spec (`on_success` / `on_failure`) and
//! dispatched from run-completion paths. `email` is always available (the
//! required capability); `webhook` posts the run result to a source system with
//! an HMAC signature, bounded retries, and dead-letter capture; `run_workflow`
//! chains another workflow; `desktop_notification` surfaces locally.

use crate::db::{Database, EmailConfig};
use crate::service::Notifier;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WEBHOOK_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
pub const ACTION_MAX_RETRIES: u32 = 3;
pub const ACTION_DISPATCH_TOTAL_BUDGET: Duration = Duration::from_secs(30);

type HmacSha256 = Hmac<Sha256>;

/// A single declarative action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionSpec {
    /// Send an email. `to` overrides the configured alert recipient.
    Email {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<String>,
    },
    /// POST the run result to an external endpoint, signed with HMAC-SHA256.
    Webhook {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
        #[serde(default)]
        max_retries: u32,
    },
    /// Chain another workflow (enqueued as a downstream run).
    RunWorkflow {
        workflow_id: String,
        #[serde(default)]
        wait: bool,
    },
    /// Local desktop notification.
    DesktopNotification {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
}

impl ActionSpec {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ActionSpec::Webhook { url, .. } => validate_outbound_webhook_url(url),
            ActionSpec::RunWorkflow { workflow_id, .. } => {
                if workflow_id.trim().is_empty() {
                    return Err("run_workflow action requires a workflow_id".into());
                }
                Ok(())
            }
            ActionSpec::Email { .. } | ActionSpec::DesktopNotification { .. } => Ok(()),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            ActionSpec::Email { .. } => "email",
            ActionSpec::Webhook { .. } => "webhook",
            ActionSpec::RunWorkflow { .. } => "run_workflow",
            ActionSpec::DesktopNotification { .. } => "desktop_notification",
        }
    }
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || octets[0] == 0
        || octets[0] == 10
        || octets[0] == 127
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
}

fn is_blocked_outbound_host(host: &str) -> bool {
    let lower = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return true;
    }
    match lower.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => is_blocked_ipv4(ip),
        Ok(IpAddr::V6(ip)) => is_blocked_ipv6(ip),
        Err(_) => false,
    }
}

fn validate_outbound_webhook_url(url: &str) -> Result<(), String> {
    let parsed =
        reqwest::Url::parse(url.trim()).map_err(|e| format!("invalid webhook url: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("webhook url must be http(s)".into());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "webhook url requires a host".to_string())?;
    if is_blocked_outbound_host(host) {
        return Err("webhook url targets a blocked local/private address".into());
    }
    Ok(())
}

/// Compute the hex HMAC-SHA256 signature of a payload with the given secret.
pub fn sign_payload(secret: &str, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Everything the dispatcher needs to execute actions for a completed run.
pub struct ActionContext {
    pub db: Arc<Database>,
    pub notifier: Arc<dyn Notifier>,
    pub workflow_name: String,
    pub run_id: String,
    pub success: bool,
    /// Serializable run result posted to webhooks / used to compose email.
    pub result_payload: serde_json::Value,
}

/// Dispatch a list of actions, returning per-action outcomes. Never panics; a
/// failing action is recorded and does not abort the others. Convenience
/// wrapper over [`dispatch_actions_with_budget`] using the default total budget;
/// retained for API stability and used by the REST test-dispatch path.
#[allow(dead_code)] // Production paths call `dispatch_actions_with_budget` directly.
pub fn dispatch_actions(actions: &[ActionSpec], ctx: &ActionContext) -> Vec<ActionResult> {
    dispatch_actions_with_budget(actions, ctx, ACTION_DISPATCH_TOTAL_BUDGET)
}

pub fn dispatch_actions_with_budget(
    actions: &[ActionSpec],
    ctx: &ActionContext,
    budget: Duration,
) -> Vec<ActionResult> {
    let started = Instant::now();
    let mut results = Vec::with_capacity(actions.len());
    for action in actions {
        if started.elapsed() >= budget {
            results.push(ActionResult {
                kind: action.kind(),
                success: false,
                message: format!(
                    "action dispatch budget exhausted before '{}' ran",
                    action.kind()
                ),
            });
            continue;
        }
        let remaining = budget.saturating_sub(started.elapsed());
        results.push(dispatch_one(action, ctx, remaining));
    }
    results
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub kind: &'static str,
    pub success: bool,
    pub message: String,
}

fn dispatch_one(action: &ActionSpec, ctx: &ActionContext, budget: Duration) -> ActionResult {
    let kind = action.kind();
    let outcome = match action {
        ActionSpec::Email { to } => dispatch_email(to.as_deref(), ctx),
        ActionSpec::Webhook {
            url,
            secret,
            max_retries,
        } => dispatch_webhook(url, secret.as_deref(), *max_retries, ctx, budget),
        ActionSpec::RunWorkflow { workflow_id, .. } => dispatch_run_workflow(workflow_id, ctx),
        ActionSpec::DesktopNotification { title } => dispatch_desktop(title.as_deref(), ctx),
    };
    match outcome {
        Ok(message) => ActionResult {
            kind,
            success: true,
            message,
        },
        Err(message) => {
            log::warn!("action '{kind}' failed for run {}: {message}", ctx.run_id);
            ActionResult {
                kind,
                success: false,
                message,
            }
        }
    }
}

fn dispatch_email(to_override: Option<&str>, ctx: &ActionContext) -> Result<String, String> {
    let mut config: EmailConfig = ctx.db.get_email_config().map_err(|e| e.to_string())?;
    if let Some(to) = to_override {
        config.alert_email = to.to_string();
    }
    if config.alert_email.trim().is_empty() {
        return Err("no recipient configured for email action".into());
    }
    let (subject, body) = if ctx.success {
        (
            format!("{} succeeded", ctx.workflow_name),
            format!("Run {} completed successfully.", ctx.run_id),
        )
    } else {
        crate::email::compose_failure_alert(&ctx.result_payload)
    };
    crate::email::send_email(&config, &config.alert_email, &subject, &body)?;
    Ok(format!("email sent to {}", config.alert_email))
}

fn dispatch_desktop(title: Option<&str>, ctx: &ActionContext) -> Result<String, String> {
    let title = title.map(|t| t.to_string()).unwrap_or_else(|| {
        format!(
            "{} {}",
            ctx.workflow_name,
            if ctx.success { "succeeded" } else { "failed" }
        )
    });
    let body = format!("Run {} — see the dashboard for details.", ctx.run_id);
    ctx.notifier.notify(&title, &body);
    Ok("desktop notification dispatched".into())
}

fn dispatch_run_workflow(workflow_id: &str, ctx: &ActionContext) -> Result<String, String> {
    // Verify the target exists; actual enqueue is performed by the scheduler's
    // dispatch path when this dispatcher is wired into completion handling.
    ctx.db
        .get_workflow(workflow_id)
        .map_err(|_| format!("chained workflow {workflow_id} not found"))?;
    Ok(format!("chained workflow {workflow_id} requested"))
}

fn dispatch_webhook(
    url: &str,
    secret: Option<&str>,
    max_retries: u32,
    ctx: &ActionContext,
    budget: Duration,
) -> Result<String, String> {
    validate_outbound_webhook_url(url)?;
    let started = Instant::now();
    let body = serde_json::to_vec(&ctx.result_payload).map_err(|e| e.to_string())?;
    let signature = secret.map(|s| sign_payload(s, &body));

    let attempts = webhook_attempts(max_retries);
    let mut last_err = String::new();
    for attempt in 0..attempts {
        if started.elapsed() >= budget {
            last_err = "action dispatch budget exhausted".to_string();
            break;
        }
        let remaining = budget.saturating_sub(started.elapsed());
        let timeout = remaining.min(WEBHOOK_REQUEST_TIMEOUT);
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| e.to_string())?;
        let mut req = client
            .post(url)
            .header("content-type", "application/json")
            .header(
                "x-chaos-event",
                if ctx.success {
                    "run.succeeded"
                } else {
                    "run.failed"
                },
            )
            .body(body.clone());
        if let Some(sig) = &signature {
            req = req.header("x-chaos-signature", format!("sha256={sig}"));
        }
        match req.send() {
            Ok(resp) if resp.status().is_success() => {
                return Ok(format!("webhook delivered (status {})", resp.status()));
            }
            Ok(resp) => {
                last_err = format!("webhook returned status {}", resp.status());
            }
            Err(e) => {
                last_err = format!("webhook request error: {e}");
            }
        }
        if attempt + 1 < attempts {
            let backoff = Duration::from_millis(200 * (1u64 << attempt.min(5)));
            let remaining = budget.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                last_err = "action dispatch budget exhausted".to_string();
                break;
            }
            std::thread::sleep(backoff.min(remaining));
        }
    }
    // Exhausted retries: route to the dead-letter table for later inspection.
    let _ = ctx
        .db
        .record_action_dead_letter(&ctx.run_id, "webhook", url, &last_err);
    Err(format!(
        "webhook delivery failed after {attempts} attempt(s): {last_err}"
    ))
}

pub fn clamp_action_max_retries(max_retries: u32) -> u32 {
    max_retries.min(ACTION_MAX_RETRIES)
}

fn webhook_attempts(max_retries: u32) -> u32 {
    clamp_action_max_retries(max_retries).saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_signature_is_stable_and_hex() {
        let sig = sign_payload("topsecret", b"{\"a\":1}");
        assert_eq!(sig.len(), 64);
        assert_eq!(sig, sign_payload("topsecret", b"{\"a\":1}"));
        assert_ne!(sig, sign_payload("other", b"{\"a\":1}"));
    }

    #[test]
    fn webhook_requires_http_url_and_public_target() {
        for url in [
            "ftp://x",
            "http://127.0.0.1/hook",
            "http://localhost/hook",
            "http://169.254.169.254/latest/meta-data",
            "http://10.0.0.5/hook",
            "http://[::1]/hook",
        ] {
            let action = ActionSpec::Webhook {
                url: url.into(),
                secret: None,
                max_retries: 0,
            };
            assert!(action.validate().is_err(), "{url} should be blocked");
        }
        let ok = ActionSpec::Webhook {
            url: "https://example.com/hook".into(),
            secret: Some("s".into()),
            max_retries: 2,
        };
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn email_and_desktop_actions_validate() {
        assert!(ActionSpec::Email { to: None }.validate().is_ok());
        assert!(ActionSpec::DesktopNotification { title: None }
            .validate()
            .is_ok());
    }

    #[test]
    fn action_retry_count_is_clamped() {
        assert_eq!(clamp_action_max_retries(0), 0);
        assert_eq!(
            clamp_action_max_retries(ACTION_MAX_RETRIES + 100),
            ACTION_MAX_RETRIES
        );
        assert_eq!(webhook_attempts(u32::MAX), ACTION_MAX_RETRIES + 1);
    }

    #[test]
    fn action_dispatch_budget_blocks_late_actions() {
        use crate::service::NoopNotifier;

        let dir = std::env::temp_dir().join(format!("chaos-act-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let ctx = ActionContext {
            db: Arc::new(Database::new(&dir)),
            notifier: Arc::new(NoopNotifier),
            workflow_name: "WF".into(),
            run_id: "run-1".into(),
            success: true,
            result_payload: serde_json::json!({"status":"success","run_id":"run-1"}),
        };
        let action = ActionSpec::DesktopNotification { title: None };

        let results =
            dispatch_actions_with_budget(std::slice::from_ref(&action), &ctx, Duration::ZERO);

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].message.contains("budget exhausted"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn outbound_webhook_blocks_loopback_targets_before_send() {
        use crate::service::NoopNotifier;

        let dir = std::env::temp_dir().join(format!("chaos-act-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Database::new(&dir));
        let ctx = ActionContext {
            db,
            notifier: Arc::new(NoopNotifier),
            workflow_name: "WF".into(),
            run_id: "run-1".into(),
            success: false,
            result_payload: serde_json::json!({"status":"failed","run_id":"run-1"}),
        };
        let action = ActionSpec::Webhook {
            url: "http://127.0.0.1:9/hook".into(),
            secret: Some("s3cr3t".into()),
            max_retries: 0,
        };
        let results = dispatch_actions(std::slice::from_ref(&action), &ctx);
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].message.contains("blocked local/private"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
