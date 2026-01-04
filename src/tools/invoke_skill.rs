//! Invoke Skill Tool - allows LLM to load skill instructions on demand.

use crate::skills::{SkillInjection, SkillsManager};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Tool for invoking skills by name.
///
/// This tool allows the LLM to load the full instructions of a skill
/// when it determines a skill is applicable to the current task.
#[derive(Clone)]
pub struct InvokeSkillTool {
    skills_manager: Arc<SkillsManager>,
    cwd: std::path::PathBuf,
}

/// Arguments for the invoke_skill tool
#[derive(Debug, Deserialize)]
pub struct InvokeSkillArgs {
    /// Name of the skill to invoke
    pub skill_name: String,
}

impl InvokeSkillTool {
    /// Create a new InvokeSkillTool
    pub fn new(skills_manager: Arc<SkillsManager>, cwd: std::path::PathBuf) -> Self {
        Self {
            skills_manager,
            cwd,
        }
    }

    /// Get the tool definition for LLM
    pub fn definition() -> crate::llm::Tool {
        crate::llm::Tool {
            name: "invoke_skill".to_string(),
            description: "Load the full instructions of a skill. Use this when you determine \
                a skill is applicable to the current task based on its description. \
                The skill's instructions will guide you on how to perform the task."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "The name of the skill to invoke (e.g., 'code-review', 'rust-expert')"
                    }
                },
                "required": ["skill_name"]
            }),
            full_description: Some(
                "Load the full instructions of a skill by name. \
                 Skills provide specialized knowledge and workflows for specific tasks. \
                 When you invoke a skill, you will receive its complete instructions \
                 which you should follow to complete the task. \
                 \n\n\
                 Example: invoke_skill({\"skill_name\": \"code-review\"}) \
                 will load the code review skill's instructions."
                    .to_string(),
            ),
        }
    }

    /// Execute the tool - load skill instructions
    pub async fn execute(&self, args: &InvokeSkillArgs) -> Result<String, String> {
        let outcome = self.skills_manager.skills_for_cwd(&self.cwd);

        // Find the skill by name
        let skill = outcome.find_by_name(&args.skill_name).ok_or_else(|| {
            format!(
                "Skill '{}' not found. Available skills: {}",
                args.skill_name,
                outcome
                    .skills
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        // Read the skill file
        let contents = std::fs::read_to_string(&skill.path)
            .map_err(|e| format!("Failed to read skill file: {}", e))?;

        // Format as skill injection
        let injection = SkillInjection {
            name: skill.name.clone(),
            path: skill.path.to_string_lossy().into_owned(),
            contents,
        };

        Ok(injection.to_xml())
    }

    /// Execute from a ToolCall
    pub async fn execute_tool_call(&self, call: &crate::llm::ToolCall) -> Result<String, String> {
        let args: InvokeSkillArgs = serde_json::from_value(call.arguments.clone())
            .map_err(|e| format!("Invalid arguments: {}", e))?;
        self.execute(&args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition() {
        let def = InvokeSkillTool::definition();
        assert_eq!(def.name, "invoke_skill");
        assert!(def.description.contains("Load the full instructions"));
    }
}
