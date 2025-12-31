// LLM Provider Module
// Provides a unified interface for multiple LLM providers

pub mod helpers;
pub mod loop_detector;
pub mod provider;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(test)]
mod tests;

// Re-export main types
pub use helpers::*;
pub use loop_detector::{LoopAction, LoopDetection, LoopDetector, LoopDetectorConfig, LoopType};
pub use provider::*;

#[cfg(feature = "openai")]
pub use openai::OpenAIProvider;
