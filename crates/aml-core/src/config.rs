use serde::{Deserialize, Serialize};

/// Singapore AML sliding-window configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowConfig {
    /// Window length in days (default: 365 for 1-year sliding window).
    pub window_days: u32,
    /// Public threshold τ in SGD cents (e.g. 2_000_000 = SGD 20,000).
    pub threshold_cents: u64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_days: 365,
            threshold_cents: 2_000_000,
        }
    }
}

/// Top-level deployment configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AmlConfig {
    pub window: WindowConfig,
    /// Maximum transaction amount in cents for demo validation.
    pub max_tx_cents: u64,
}

impl Default for AmlConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            max_tx_cents: 5_000_000,
        }
    }
}
