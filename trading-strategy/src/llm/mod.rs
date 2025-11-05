pub mod rag_retriever;
pub mod prompt_formatter;
pub mod llm_client;
pub mod metrics;

// Re-export commonly used items
pub use rag_retriever::{HistoricalMatch, RagRetriever};
pub use prompt_formatter::LlmPromptFormatter;
pub use llm_client::{
    LlmClient, LlmConfig, LlmProvider, LlmResponse, SignalAction, TradingDecision,
};
pub use metrics::{RagMetrics, MetricsTimer};
