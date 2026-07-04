//! On-success / on-failure action framework.
//!
//! Actions are declared in a workflow's spec (`on_success` / `on_failure`) and
//! dispatched from run-completion paths. `email` is always available (the
//! required capability); `webhook` posts the run result to a source system with
//! an HMAC signature, bounded retries, and dead-letter capture; `run_workflow`
//! chains another workflow; `desktop_notification` surfaces locally.

use crate::db::{Database, EmailConfig};
use crate::service::Notifier;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;

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
            ActionSpec::Webhook { url, .. } => {
                if url.trim().is_empty() {
                    return Err("webhook action requires a url".into());
                }
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err("webhook url must be http(s)".into());
                }
                Ok(())
            }
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
/// failing action is recorded and does not abort the others.
pub fn dispatch_actions(actions: &[ActionSpec], ctx: &ActionContext) -> Vec<ActionResult> {
    actions
        .iter()
        .map(|action| dispatch_one(action, ctx))
        .collect()
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub kind: &'static str,
    pub success: bool,
    pub message: String,
}

fn dispatch_one(action: &ActionSpec, ctx: &ActionContext) -> ActionResult {
    let kind = action.kind();
    let outcome = match action {
        ActionSpec::Email { to } => dispatch_email(to.as_deref(), ctx),
        ActionSpec::Webhook {
            url,
            secret,
            max_retries,
        } => dispatch_webhook(url, secret.as_deref(), *max_retries, ctx),
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
) -> Result<String, String> {
    let body = serde_json::to_vec(&ctx.result_payload).map_err(|e| e.to_string())?;
    let signature = secret.map(|s| sign_payload(s, &body));

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let attempts = max_retries.saturating_add(1);
    let mut last_err = String::new();
    for attempt in 0..attempts {
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
            std::thread::sleep(std::time::Duration::from_millis(
                200 * (1 << attempt.min(5)) as u64,
            ));
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
    fn webhook_requires_http_url() {
        let a = ActionSpec::Webhook {
            url: "ftp://x".into(),
            secret: None,
            max_retries: 0,
        };
        assert!(a.validate().is_err());
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
    fn outbound_webhook_delivers_signed_payload() {
        use crate::service::NoopNotifier;
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 8192];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            request
        });

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
            url: format!("http://{addr}/hook"),
            secret: Some("s3cr3t".into()),
            max_retries: 0,
        };
        let results = dispatch_actions(std::slice::from_ref(&action), &ctx);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].success,
            "delivery should succeed: {}",
            results[0].message
        );

        let request = server.join().unwrap();
        let lower = request.to_lowercase();
        assert!(lower.contains("x-chaos-signature: sha256="));
        assert!(lower.contains("x-chaos-event: run.failed"));
        // The signature must match the HMAC of the exact JSON body sent.
        let body = serde_json::to_vec(&ctx.result_payload).unwrap();
        let expected = sign_payload("s3cr3t", &body);
        assert!(
            request.contains(&expected),
            "signature header must match body HMAC"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
