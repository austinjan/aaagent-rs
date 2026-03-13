// Shared constants for the application

/**
 * Provider types that map to backend provider names
 */
export type ProviderType = "openai" | "anthropic" | "google";

/**
 * Supported models from config.yaml temperature_profiles
 * This list should be kept in sync with the backend configuration
 */
export const SUPPORTED_MODELS = [
  { value: "gpt-5", label: "GPT-5 (Reasoning)", provider: "openai" as ProviderType },
  { value: "gpt-5-mini", label: "GPT-5 Mini (Reasoning)", provider: "openai" as ProviderType },
  { value: "gpt-5-nano", label: "GPT-5 Nano (Reasoning)", provider: "openai" as ProviderType },
  { value: "gpt-5.2", label: "GPT-5.2", provider: "openai" as ProviderType },
  { value: "gemini-3-flash-preview", label: "Gemini 3 Flash Preview", provider: "google" as ProviderType },
  { value: "gemini-3-pro-preview", label: "Gemini 3 Pro Preview", provider: "google" as ProviderType },
] as const;

/**
 * Special value to indicate custom model input
 */
export const CUSTOM_MODEL_VALUE = "__custom__";

/**
 * Type for supported model values
 */
export type SupportedModelValue = (typeof SUPPORTED_MODELS)[number]["value"];

/**
 * Get provider type from model name
 */
export function getProviderForModel(model: string): ProviderType {
  if (model.startsWith("gpt-") || model.startsWith("o1-") || model.startsWith("o3-")) return "openai";
  if (model.startsWith("claude-")) return "anthropic";
  if (model.startsWith("gemini-")) return "google";
  return "openai"; // default
}
