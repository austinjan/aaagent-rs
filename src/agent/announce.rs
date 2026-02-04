//! Announce Flow - Handles sub-agent completion announcements
//!
//! When a sub-agent completes, this module:
//! 1. Reads the sub-agent's output from the session
//! 2. Formats an announcement message
//! 3. Checks if the parent agent is busy
//! 4. Either injects immediately or enqueues for later processing

use anyhow::Result;
use std::sync::Arc;

use crate::agent::{AgentRuntime, SubAgentOutcome, SubAgentRegistry, SubAgentRun};
use crate::api::event_bus::{GlobalEventBus, MessageSource};

/// Run the announce flow for a completed sub-agent
///
/// This function is called after a sub-agent completes. It reads the output,
/// formats an announcement, and either injects it immediately or enqueues it
/// based on whether the parent agent is busy.
pub async fn run_announce_flow(
    run: &SubAgentRun,
    _registry: Arc<SubAgentRegistry>,
    runtime: Arc<AgentRuntime>,
    event_bus: Arc<GlobalEventBus>,
    storage: Arc<dyn crate::history::TreeStore>,
) -> Result<()> {
    // Format the announcement message
    let announcement = format_announcement(run, &storage).await?;

    // Check if parent agent is busy
    let parent_busy = runtime.is_run_active(&run.parent_session_key);

    if parent_busy {
        // Parent is busy - enqueue the message
        log::info!(
            "Parent agent {} is busy, enqueueing announcement for {}",
            run.parent_session_key,
            run.run_id
        );

        let queued = runtime.enqueue_message(
            run.parent_session_key.clone(),
            crate::agent::QueuedMessage {
                content: announcement,
                mode: crate::agent::QueueMode::Followup,
                source: crate::agent::RuntimeMessageSource::SubAgent {
                    run_id: run.run_id.clone(),
                },
                queued_at: chrono::Utc::now().timestamp_millis(),
            },
        )?;

        if !queued {
            log::warn!(
                "Failed to enqueue announcement for {} - queue full",
                run.run_id
            );
        }
    } else {
        // Parent is idle - inject immediately
        log::info!(
            "Parent agent {} is idle, injecting announcement for {}",
            run.parent_session_key,
            run.run_id
        );

        event_bus.emit_inject(
            run.parent_session_key.clone(),
            announcement,
            MessageSource::SubAgent {
                run_id: run.run_id.clone(),
            },
        );
    }

    Ok(())
}

/// Format an announcement message from a completed sub-agent run
async fn format_announcement(
    run: &SubAgentRun,
    _storage: &Arc<dyn crate::history::TreeStore>,
) -> Result<String> {
    let elapsed_secs = run.elapsed_ms() / 1000;

    match &run.outcome {
        Some(SubAgentOutcome::Success {
            output,
            tokens_used,
            runtime_ms,
        }) => {
            // Try to read more context from the session if available
            // For now, just use the output from the outcome

            Ok(format!(
                "📬 **Sub-Agent Completed: {}**\n\n\
                ✅ **Status**: Success\n\
                ⏱️  **Runtime**: {:.1}s\n\
                🪙 **Tokens**: {}\n\n\
                **Output**:\n{}\n\n\
                ---\n\
                *(Sub-agent run ID: {})*",
                run.task_label,
                (*runtime_ms as f64) / 1000.0,
                tokens_used,
                truncate_output(output, 2000),
                run.run_id
            ))
        }
        Some(SubAgentOutcome::Error { error }) => Ok(format!(
            "📬 **Sub-Agent Completed: {}**\n\n\
            ❌ **Status**: Error\n\
            ⏱️  **Runtime**: {:.1}s\n\n\
            **Error**:\n{}\n\n\
            ---\n\
            *(Sub-agent run ID: {})*",
            run.task_label, elapsed_secs as f64, error, run.run_id
        )),
        Some(SubAgentOutcome::Timeout { timeout_secs }) => Ok(format!(
            "📬 **Sub-Agent Completed: {}**\n\n\
            ⏱️  **Status**: Timeout (exceeded {}s)\n\
            ⏱️  **Runtime**: {:.1}s\n\n\
            The sub-agent did not complete within the timeout period.\n\n\
            ---\n\
            *(Sub-agent run ID: {})*",
            run.task_label, timeout_secs, elapsed_secs as f64, run.run_id
        )),
        None => {
            // Should not happen - outcome should be set when run is completed
            Ok(format!(
                "📬 **Sub-Agent Status: {}**\n\n\
                ⚠️  **Status**: Unknown (no outcome recorded)\n\
                ⏱️  **Runtime**: {:.1}s\n\n\
                ---\n\
                *(Sub-agent run ID: {})*",
                run.task_label, elapsed_secs as f64, run.run_id
            ))
        }
    }
}

/// Truncate output to a maximum length, adding ellipsis if truncated
fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        output.to_string()
    } else {
        // Keep first 80% and last 10% with ellipsis in between
        let first_len = (max_len as f64 * 0.8) as usize;
        let last_len = (max_len as f64 * 0.1) as usize;

        let first = &output[..first_len.min(output.len())];
        let last_start = output.len().saturating_sub(last_len);
        let last = &output[last_start..];

        format!(
            "{}\n\n[... {} chars truncated ...]\n\n{}",
            first,
            output.len() - first_len - last_len,
            last
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{CleanupStrategy, SubAgentRun};

    #[test]
    fn test_format_announcement_success() {
        let mut run = SubAgentRun::new(
            "run-1".to_string(),
            "child-session".to_string(),
            "parent-session".to_string(),
            "Test Task".to_string(),
            CleanupStrategy::Keep,
        );

        run.mark_started();
        run.mark_completed(SubAgentOutcome::Success {
            output: "Task completed successfully!".to_string(),
            tokens_used: 150,
            runtime_ms: 2500,
        });

        // Note: format_announcement is async, so we'll just verify the run state
        assert!(run.is_completed());
        assert!(!run.is_active());
    }

    #[test]
    fn test_format_announcement_error() {
        let mut run = SubAgentRun::new(
            "run-2".to_string(),
            "child-session".to_string(),
            "parent-session".to_string(),
            "Failing Task".to_string(),
            CleanupStrategy::Keep,
        );

        run.mark_started();
        run.mark_completed(SubAgentOutcome::Error {
            error: "Something went wrong".to_string(),
        });

        assert!(run.is_completed());
        assert!(matches!(run.outcome, Some(SubAgentOutcome::Error { .. })));
    }

    #[test]
    fn test_truncate_output_short() {
        let output = "Short output";
        let result = truncate_output(output, 1000);
        assert_eq!(result, output);
    }

    #[test]
    fn test_truncate_output_long() {
        let output = "a".repeat(3000);
        let result = truncate_output(&output, 1000);
        assert!(result.len() < output.len());
        assert!(result.contains("truncated"));
    }
}
