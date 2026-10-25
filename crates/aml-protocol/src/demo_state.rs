use std::collections::HashMap;

use chrono::Utc;
use rand::rngs::OsRng;
use rand::seq::SliceRandom;

use aml_core::{AmlConfig, AmlResult, CommitmentBytes, Transaction};
use aml_crypto_ibc::{IbcCommitment, IbcOpening, IbcScheme, PublicParams};
use aml_persephone::{CommitmentState, PersephoneVerifier, ValueVerifier};

/// In-memory demo server state simulating PayPay transaction history.
#[derive(Debug, Clone)]
pub struct DemoServerState {
    pub pp: PublicParams,
    pub users: HashMap<String, DemoUserRecord>,
}

#[derive(Debug, Clone)]
pub struct DemoUserRecord {
    pub user_id_hash: u64,
    pub tx_id_commitment: IbcCommitment,
    pub tx_id_opening: IbcOpening,
    pub membership_set_commitments: Vec<IbcCommitment>,
    pub transactions: Vec<Transaction>,
    pub openings: Vec<IbcOpening>,
    pub commitments: Vec<IbcCommitment>,
    pub nnp_state: CommitmentState,
}

impl Default for DemoServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoServerState {
    pub fn new() -> Self {
        Self {
            pp: PublicParams::setup(),
            users: HashMap::new(),
        }
    }

    pub fn setup_user(
        &mut self,
        user_key: &str,
        user_id_hash: u64,
        historical_amounts: &[(u64, i64)],
    ) -> AmlResult<CommitmentBytes> {
        let mut rng = OsRng;
        let verifier = PersephoneVerifier::new();
        let config = AmlConfig::default();
        let threshold = config.window.threshold_cents;

        let (tx_c, tx_o) = IbcScheme::commit(&self.pp, user_id_hash, 0, &mut rng);
        let tx_id_encoded = CommitmentBytes::new(IbcScheme::encode(&tx_c)?);
        let mut membership_set_commitments = vec![tx_c];
        for offset in 1..8 {
            let (decoy, _) = IbcScheme::commit(&self.pp, user_id_hash + offset, 0, &mut rng);
            membership_set_commitments.push(decoy);
        }
        membership_set_commitments.shuffle(&mut rng);

        let mut transactions = Vec::new();
        let mut openings = Vec::new();
        let mut commitments = Vec::new();

        for (amount, days_ago) in historical_amounts {
            let (c, o) = IbcScheme::commit(&self.pp, user_id_hash, *amount, &mut rng);
            let ts = Utc::now() - chrono::Duration::days(*days_ago);
            transactions.push(Transaction {
                data_id: uuid::Uuid::new_v4().to_string(),
                group_id: user_key.to_string(),
                amount_cents: *amount,
                timestamp: ts,
                commitment: CommitmentBytes::new(IbcScheme::encode(&c)?),
            });
            openings.push(o);
            commitments.push(c);
        }

        let nnp_state = verifier
            .init_bounds(0, threshold)
            .map_err(|e| aml_core::AmlError::Internal(e.to_string()))?;

        self.users.insert(
            user_key.to_string(),
            DemoUserRecord {
                user_id_hash,
                tx_id_commitment: tx_c,
                tx_id_opening: tx_o,
                membership_set_commitments,
                transactions,
                openings,
                commitments,
                nnp_state,
            },
        );

        Ok(tx_id_encoded)
    }
}
