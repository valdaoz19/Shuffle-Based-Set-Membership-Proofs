use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use aml_core::{Transaction, WindowConfig};

/// Filter transactions within a sliding window S^T = {v : v.t > T}.
pub struct SlidingWindow;

impl SlidingWindow {
    pub fn window_start(config: &WindowConfig, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::days(i64::from(config.window_days))
    }

    pub fn filter(transactions: &[Transaction], window_start: DateTime<Utc>) -> Vec<Transaction> {
        transactions
            .iter()
            .filter(|tx| tx.timestamp > window_start)
            .cloned()
            .collect()
    }

    pub fn filter_with_new(
        transactions: &[Transaction],
        window_start: DateTime<Utc>,
        new_amount_cents: u64,
        new_timestamp: DateTime<Utc>,
    ) -> WindowFilterResult {
        let in_window = Self::filter(transactions, window_start);
        let total_cents: u64 = in_window.iter().map(|tx| tx.amount_cents).sum();
        let includes_new = new_timestamp > window_start;
        let total_with_new = if includes_new {
            total_cents.saturating_add(new_amount_cents)
        } else {
            total_cents
        };

        WindowFilterResult {
            transactions: in_window,
            total_cents,
            total_with_new_cents: total_with_new,
            includes_new,
        }
    }

    pub fn expiring_transactions(
        transactions: &[Transaction],
        window_start: DateTime<Utc>,
        grace_days: u32,
    ) -> Vec<Transaction> {
        let expiry_cutoff = window_start + Duration::days(i64::from(grace_days));
        transactions
            .iter()
            .filter(|tx| tx.timestamp <= expiry_cutoff)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFilterResult {
    pub transactions: Vec<Transaction>,
    pub total_cents: u64,
    pub total_with_new_cents: u64,
    pub includes_new: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_core::CommitmentBytes;
    use uuid::Uuid;

    fn tx(amount: u64, days_ago: i64) -> Transaction {
        Transaction {
            data_id: Uuid::new_v4().to_string(),
            group_id: "user_42".into(),
            amount_cents: amount,
            timestamp: Utc::now() - Duration::days(days_ago),
            commitment: CommitmentBytes::new("dummy"),
        }
    }

    #[test]
    fn filters_one_year_window() {
        let config = WindowConfig::default();
        let now = Utc::now();
        let start = SlidingWindow::window_start(&config, now);
        let txs = vec![tx(100, 30), tx(200, 400), tx(300, 10)];
        let filtered = SlidingWindow::filter(&txs, start);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered.iter().map(|t| t.amount_cents).sum::<u64>(), 400);
    }
}
