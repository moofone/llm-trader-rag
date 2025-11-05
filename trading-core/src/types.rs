pub mod market_snapshot;

// Re-export common types
pub use market_snapshot::MarketStateSnapshot;

/// Timestamp in milliseconds since Unix epoch
pub type TimestampMS = u64;

/// Crypto futures symbol (e.g., "BTCUSDT", "ETHUSDT")
pub type CryptoFuturesSymbol = String;
