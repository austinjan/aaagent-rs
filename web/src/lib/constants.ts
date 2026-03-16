// Shared constants for the application

/**
 * Provider types that map to backend provider names
 */
export type ProviderType = "openai" | "anthropic" | "google";

/**
 * Special value to indicate custom model input
 */
export const CUSTOM_MODEL_VALUE = "__custom__";

/**
 * Get provider type from model name
 */
export function getProviderForModel(model: string): ProviderType {
  if (model.startsWith("gpt-") || model.startsWith("o1-") || model.startsWith("o3-")) return "openai";
  if (model.startsWith("claude-")) return "anthropic";
  if (model.startsWith("gemini-")) return "google";
  return "openai"; // default
}
