//! RAG Performance Metrics
//!
//! This module provides metrics tracking for the RAG system to monitor:
//! - Retrieval latency and quality
//! - Embedding generation performance
//! - LLM inference latency
//! - Historical pattern match quality and outcomes distribution

use std::time::{Duration, Instant};

/// Metrics for RAG retrieval and LLM inference performance
#[derive(Debug, Clone, Default)]
pub struct RagMetrics {
    /// Time taken to retrieve similar patterns from Qdrant (milliseconds)
    pub retrieval_latency_ms: u64,

    /// Time taken to generate embeddings (milliseconds)
    pub embedding_latency_ms: u64,

    /// Time taken for LLM to generate response (milliseconds)
    pub llm_latency_ms: u64,

    /// Similarity scores for all retrieved matches
    pub similarity_scores: Vec<f32>,

    /// Minimum similarity score among matches
    pub similarity_min: Option<f32>,

    /// Maximum similarity score among matches
    pub similarity_max: Option<f32>,

    /// Number of historical matches found
    pub num_matches: usize,

    /// Distribution of 4-hour outcomes from retrieved patterns
    pub outcomes_distribution: Vec<f64>,

    /// Median 4-hour outcome from historical matches
    pub outcome_median_4h: Option<f64>,

    /// 10th percentile (P10) of 4-hour outcomes
    pub outcome_p10_4h: Option<f64>,

    /// 90th percentile (P90) of 4-hour outcomes
    pub outcome_p90_4h: Option<f64>,
}

impl RagMetrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Set retrieval latency from a duration
    pub fn set_retrieval_latency(&mut self, duration: Duration) {
        self.retrieval_latency_ms = duration.as_millis() as u64;
    }

    /// Set embedding latency from a duration
    pub fn set_embedding_latency(&mut self, duration: Duration) {
        self.embedding_latency_ms = duration.as_millis() as u64;
    }

    /// Set LLM latency from a duration
    pub fn set_llm_latency(&mut self, duration: Duration) {
        self.llm_latency_ms = duration.as_millis() as u64;
    }

    /// Add similarity scores and compute statistics
    pub fn set_similarity_scores(&mut self, scores: Vec<f32>) {
        if scores.is_empty() {
            self.similarity_min = None;
            self.similarity_max = None;
            self.num_matches = 0;
        } else {
            self.similarity_min = scores.iter().copied().min_by(|a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.similarity_max = scores.iter().copied().max_by(|a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.num_matches = scores.len();
        }
        self.similarity_scores = scores;
    }

    /// Set outcome distribution and compute statistics
    pub fn set_outcomes(&mut self, outcomes: Vec<f64>) {
        if outcomes.is_empty() {
            self.outcome_median_4h = None;
            self.outcome_p10_4h = None;
            self.outcome_p90_4h = None;
            self.outcomes_distribution = Vec::new();
            return;
        }

        let mut sorted = outcomes.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate percentiles
        self.outcome_median_4h = Some(percentile(&sorted, 50.0));
        self.outcome_p10_4h = Some(percentile(&sorted, 10.0));
        self.outcome_p90_4h = Some(percentile(&sorted, 90.0));
        self.outcomes_distribution = sorted;
    }

    /// Calculate average similarity score
    pub fn avg_similarity(&self) -> f32 {
        if self.similarity_scores.is_empty() {
            0.0
        } else {
            self.similarity_scores.iter().sum::<f32>() / self.similarity_scores.len() as f32
        }
    }

    /// Calculate total latency (retrieval + embedding + LLM)
    pub fn total_latency_ms(&self) -> u64 {
        self.retrieval_latency_ms + self.embedding_latency_ms + self.llm_latency_ms
    }

    /// Report metrics to tracing logs
    pub fn report(&self) {
        let avg_sim = self.avg_similarity();

        tracing::info!(
            "RAG Metrics: retrieval={}ms, embedding={}ms, llm={}ms, total={}ms, avg_sim={:.2}, matches={}, sim_range=[{:?},{:?}], median_4h={:?}, p10_4h={:?}, p90_4h={:?}",
            self.retrieval_latency_ms,
            self.embedding_latency_ms,
            self.llm_latency_ms,
            self.total_latency_ms(),
            avg_sim,
            self.num_matches,
            self.similarity_min,
            self.similarity_max,
            self.outcome_median_4h,
            self.outcome_p10_4h,
            self.outcome_p90_4h,
        );
    }

    /// Report detailed metrics with additional information
    pub fn report_detailed(&self) {
        self.report();

        if !self.outcomes_distribution.is_empty() {
            let positive_count = self.outcomes_distribution.iter().filter(|&&x| x > 0.0).count();
            let negative_count = self.outcomes_distribution.iter().filter(|&&x| x < 0.0).count();
            let neutral_count = self.outcomes_distribution.len() - positive_count - negative_count;

            tracing::info!(
                "Outcome distribution: positive={}({:.1}%), negative={}({:.1}%), neutral={}({:.1}%)",
                positive_count,
                (positive_count as f64 / self.outcomes_distribution.len() as f64) * 100.0,
                negative_count,
                (negative_count as f64 / self.outcomes_distribution.len() as f64) * 100.0,
                neutral_count,
                (neutral_count as f64 / self.outcomes_distribution.len() as f64) * 100.0,
            );
        }
    }
}

/// Timer helper for measuring operation latency
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Stop the timer and return elapsed duration
    pub fn stop(self) -> Duration {
        self.start.elapsed()
    }
}

/// Calculate percentile from sorted data
fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let len = sorted_data.len();
    let idx = (p / 100.0 * (len - 1) as f64).round() as usize;
    sorted_data[idx.min(len - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = RagMetrics::new();
        assert_eq!(metrics.num_matches, 0);
        assert_eq!(metrics.retrieval_latency_ms, 0);
        assert_eq!(metrics.embedding_latency_ms, 0);
        assert_eq!(metrics.llm_latency_ms, 0);
    }

    #[test]
    fn test_similarity_scores() {
        let mut metrics = RagMetrics::new();
        let scores = vec![0.9, 0.85, 0.75, 0.95, 0.8];
        metrics.set_similarity_scores(scores);

        assert_eq!(metrics.num_matches, 5);
        assert_eq!(metrics.similarity_min, Some(0.75));
        assert_eq!(metrics.similarity_max, Some(0.95));
        assert_eq!(metrics.avg_similarity(), 0.85);
    }

    #[test]
    fn test_outcomes_distribution() {
        let mut metrics = RagMetrics::new();
        let outcomes = vec![-2.3, 1.1, -1.8, -0.5, 0.9];
        metrics.set_outcomes(outcomes);

        assert_eq!(metrics.outcome_median_4h, Some(-0.5));
        assert!(metrics.outcome_p10_4h.is_some());
        assert!(metrics.outcome_p90_4h.is_some());
    }

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 50.0), 3.0);
        assert_eq!(percentile(&data, 100.0), 5.0);
    }

    #[test]
    fn test_timer() {
        let timer = MetricsTimer::start();
        std::thread::sleep(Duration::from_millis(10));
        let duration = timer.stop();
        assert!(duration.as_millis() >= 10);
    }

    #[test]
    fn test_empty_outcomes() {
        let mut metrics = RagMetrics::new();
        metrics.set_outcomes(vec![]);

        assert_eq!(metrics.outcome_median_4h, None);
        assert_eq!(metrics.outcome_p10_4h, None);
        assert_eq!(metrics.outcome_p90_4h, None);
    }

    #[test]
    fn test_latency_setters() {
        let mut metrics = RagMetrics::new();
        metrics.set_retrieval_latency(Duration::from_millis(50));
        metrics.set_embedding_latency(Duration::from_millis(30));
        metrics.set_llm_latency(Duration::from_millis(200));

        assert_eq!(metrics.retrieval_latency_ms, 50);
        assert_eq!(metrics.embedding_latency_ms, 30);
        assert_eq!(metrics.llm_latency_ms, 200);
        assert_eq!(metrics.total_latency_ms(), 280);
    }
}
