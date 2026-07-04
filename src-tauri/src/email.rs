//! Native SMTP email via `lettre`, replacing the external `email_alert.py`
//! dependency. All outbound scheduler email (test messages and failure alerts,
//! plus the Phase 5 action framework) goes through [`send_email`].

use crate::db::EmailConfig;
use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};

/// Send a plain-text email using the stored SMTP configuration.
///
/// TLS mode is chosen from the port: 465 uses implicit TLS (`relay`), anything
/// else uses STARTTLS (`starttls_relay`), matching common SMTP providers.
pub fn send_email(
    config: &EmailConfig,
    to: &str,
    subject: &str,
    body_text: &str,
) -> Result<(), String> {
    if config.smtp_host.trim().is_empty() {
        return Err("SMTP host is not configured".to_string());
    }
    let from_address = if config.from_address.trim().is_empty() {
        config.smtp_user.trim()
    } else {
        config.from_address.trim()
    };
    if from_address.is_empty() {
        return Err("email from_address / smtp_user is not configured".to_string());
    }

    let from_mbox: Mailbox = format!("{} <{}>", config.from_name, from_address)
        .parse()
        .map_err(|e| format!("invalid from address: {e}"))?;
    let to_mbox: Mailbox = to
        .trim()
        .parse()
        .map_err(|e| format!("invalid recipient address {to:?}: {e}"))?;

    let message = Message::builder()
        .from(from_mbox)
        .to(to_mbox)
        .subject(subject)
        .body(body_text.to_string())
        .map_err(|e| format!("failed to build email: {e}"))?;

    let mut builder = if config.smtp_port == 465 {
        SmtpTransport::relay(config.smtp_host.trim())
            .map_err(|e| format!("failed to init SMTP relay: {e}"))?
    } else {
        SmtpTransport::starttls_relay(config.smtp_host.trim())
            .map_err(|e| format!("failed to init SMTP STARTTLS relay: {e}"))?
    };
    builder = builder.port(config.smtp_port as u16);
    if !config.smtp_user.trim().is_empty() {
        builder = builder.credentials(Credentials::new(
            config.smtp_user.clone(),
            config.smtp_password.clone(),
        ));
    }
    let transport = builder.build();
    transport
        .send(&message)
        .map(|_| ())
        .map_err(|e| format!("failed to send email: {e}"))
}

/// Compose the subject/body for a workflow failure alert from run context.
pub fn compose_failure_alert(run_context: &serde_json::Value) -> (String, String) {
    let workflow_name = run_context
        .get("workflow_name")
        .and_then(|v| v.as_str())
        .unwrap_or("workflow");
    let exit_code = run_context
        .get("exit_code")
        .and_then(|v| v.as_i64())
        .map(|c| c.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let stderr = run_context
        .get("stderr")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let started_at = run_context
        .get("started_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let run_id = run_context
        .get("run_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let subject = format!("{} run failed", workflow_name);
    let mut body = format!(
        "Workflow '{}' failed.\n\nRun ID: {}\nStarted: {}\nExit code: {}\n",
        workflow_name, run_id, started_at, exit_code
    );
    if !stderr.is_empty() {
        let tail: String = stderr.chars().rev().take(2000).collect::<String>();
        let tail: String = tail.chars().rev().collect();
        body.push_str("\n--- stderr (tail) ---\n");
        body.push_str(&tail);
        body.push('\n');
    }
    body.push_str("\nOpen the Chaos Scheduler dashboard for full details.\n");
    (subject, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_failure_alert_includes_workflow_and_stderr() {
        let ctx = serde_json::json!({
            "workflow_name": "Nightly Digest",
            "exit_code": 2,
            "stderr": "Traceback: boom",
            "run_id": "run-42",
            "started_at": "2026-07-04T00:00:00Z",
        });
        let (subject, body) = compose_failure_alert(&ctx);
        assert!(subject.contains("Nightly Digest"));
        assert!(body.contains("run-42"));
        assert!(body.contains("Exit code: 2"));
        assert!(body.contains("Traceback: boom"));
    }

    #[test]
    fn send_email_requires_host() {
        let cfg = EmailConfig {
            smtp_host: String::new(),
            ..Default::default()
        };
        let err = send_email(&cfg, "a@b.com", "s", "b").unwrap_err();
        assert!(err.contains("SMTP host"));
    }
}
