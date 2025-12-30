// LLM Provider Module
// Provides a unified interface for multiple LLM providers

pub mod helpers;
pub mod provider;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "gemini")]
pub mod gemini;

#[cfg(test)]
mod tests;

// Re-export main types
pub use helpers::*;
pub use provider::*;

#[cfg(feature = "openai")]
pub use openai::OpenAIProvider;

#[cfg(feature = "gemini")]
pub use gemini::GeminiProvider;
