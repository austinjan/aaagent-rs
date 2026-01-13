// Shared constants for the application

/**
 * Supported models from config.yaml temperature_profiles
 * This list should be kept in sync with the backend configuration
 */
export const SUPPORTED_MODELS = [
  { value: "gpt-5", label: "GPT-5 (Reasoning)" },
  { value: "gpt-5-mini", label: "GPT-5 Mini (Reasoning)" },
  { value: "gpt-5-nano", label: "GPT-5 Nano (Reasoning)" },
  { value: "gpt-5.2", label: "GPT-5.2" },
  { value: "gemini-3-flash-preview", label: "Gemini 3 Flash Preview" },
  { value: "gemini-3-pro-preview", label: "Gemini 3 Pro Preview" },
] as const;

/**
 * Special value to indicate custom model input
 */
export const CUSTOM_MODEL_VALUE = "__custom__";

/**
 * Type for supported model values
 */
export type SupportedModelValue = (typeof SUPPORTED_MODELS)[number]["value"];
