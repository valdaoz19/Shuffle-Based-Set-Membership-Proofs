use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use aml_core::Transaction;

/// Authorized future action for sliding-window expiry (Verified To-Do List).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoAction {
    pub action_id: Uuid,
    pub transaction_data_id: String,
    pub scheduled_at: DateTime<Utc>,
    pub action_type: TodoActionType,
    pub authorized: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoActionType {
    ExpireTransaction,
    RemoveFromWindow,
}

/// Verified log of authorized sliding-window maintenance actions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifiedTodoList {
    pub actions: Vec<TodoAction>,
}

impl VerifiedTodoList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule_expiry(
        &mut self,
        transaction: &Transaction,
        scheduled_at: DateTime<Utc>,
    ) -> Uuid {
        let action_id = Uuid::new_v4();
        self.actions.push(TodoAction {
            action_id,
            transaction_data_id: transaction.data_id.clone(),
            scheduled_at,
            action_type: TodoActionType::ExpireTransaction,
            authorized: true,
        });
        action_id
    }

    pub fn pending_actions(&self, now: DateTime<Utc>) -> Vec<&TodoAction> {
        self.actions
            .iter()
            .filter(|a| a.authorized && a.scheduled_at <= now)
            .collect()
    }

    pub fn to_json(&self) -> aml_core::AmlResult<String> {
        serde_json::to_string(self).map_err(|e| aml_core::AmlError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_core::CommitmentBytes;

    #[test]
    fn schedules_and_lists_pending() {
        let mut list = VerifiedTodoList::new();
        let tx = Transaction {
            data_id: "tx1".into(),
            group_id: "u1".into(),
            amount_cents: 100,
            timestamp: Utc::now(),
            commitment: CommitmentBytes::new("c"),
        };
        let id = list.schedule_expiry(&tx, Utc::now());
        assert_eq!(list.pending_actions(Utc::now()).len(), 1);
        assert_eq!(list.actions[0].action_id, id);
    }
}
