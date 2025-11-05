pub mod rag_retriever;
pub mod prompt_formatter;
pub mod llm_client;

// Re-export commonly used items
pub use rag_retriever::{HistoricalMatch, RagRetriever};
pub use prompt_formatter::LlmPromptFormatter;
pub use llm_client::{
    LlmClient, LlmConfig, LlmProvider, LlmResponse, SignalAction, TradingDecision,
};
