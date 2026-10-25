use std::{path::Path, str::FromStr};

use aml_core::{CommitmentBytes, CryptoProofMetadata, VerificationStatus, VerifyApiResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

#[derive(Debug, Clone)]
pub struct AuditStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct AuditInsert {
    pub user_key: String,
    pub scenario: Option<String>,
    pub amount_cents: u64,
    pub tx_id_commitment: CommitmentBytes,
    pub threshold_cents: u64,
    pub window_days: u32,
    pub response: VerifyApiResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProofAuditRecord {
    pub request_id: String,
    pub created_at: String,
    pub user_key: String,
    pub scenario: Option<String>,
    pub amount_cents: u64,
    pub tx_id_commitment: CommitmentBytes,
    pub status: VerificationStatus,
    pub threshold_compliant: bool,
    pub threshold_cents: u64,
    pub window_days: u32,
    pub proof_metadata: CryptoProofMetadata,
    pub proof_audit_hash: String,
    pub replay_warning: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProofIntegrityResponse {
    pub request_id: String,
    pub valid: bool,
    pub stored_hash: String,
    pub recomputed_hash: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitmentLedgerEntry {
    pub id: i64,
    pub request_id: String,
    pub created_at: String,
    pub commitment_handle: CommitmentBytes,
    pub status: VerificationStatus,
    pub record_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitmentLedgerResponse {
    pub entries: Vec<CommitmentLedgerEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThresholdConfigSnapshot {
    threshold_cents: u64,
    window_days: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuditHashPayload {
    request_id: String,
    created_at: String,
    user_key: String,
    scenario: Option<String>,
    amount_cents: u64,
    tx_id_commitment: String,
    status: VerificationStatus,
    threshold_compliant: bool,
    threshold_config: ThresholdConfigSnapshot,
    proof_metadata: CryptoProofMetadata,
    replay_warning: bool,
    message: String,
}

impl AuditStore {
    pub async fn connect_from_env() -> anyhow::Result<Self> {
        let url = std::env::var("AML_AUDIT_DB_URL")
            .unwrap_or_else(|_| "sqlite:data/aml_audit.sqlite".to_string());
        Self::connect(&url).await
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        create_parent_dir(database_url)?;

        let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_records (
                request_id TEXT PRIMARY KEY NOT NULL,
                created_at TEXT NOT NULL,
                user_key TEXT NOT NULL,
                scenario TEXT,
                amount_cents INTEGER NOT NULL,
                tx_id_commitment TEXT NOT NULL,
                status TEXT NOT NULL,
                threshold_compliant INTEGER NOT NULL,
                threshold_cents INTEGER NOT NULL,
                window_days INTEGER NOT NULL,
                threshold_config_json TEXT NOT NULL,
                proof_metadata_json TEXT NOT NULL,
                proof_audit_hash TEXT NOT NULL,
                replay_warning INTEGER NOT NULL,
                message TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS commitment_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                commitment_handle TEXT NOT NULL,
                status TEXT NOT NULL,
                record_hash TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_records_tx_id ON audit_records(tx_id_commitment)",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_commitment_ledger_request ON commitment_ledger(request_id)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn has_seen_tx_commitment(
        &self,
        tx_id_commitment: &CommitmentBytes,
    ) -> anyhow::Result<bool> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(1) FROM audit_records WHERE tx_id_commitment = ?")
                .bind(&tx_id_commitment.0)
                .fetch_one(&self.pool)
                .await?;
        Ok(count > 0)
    }

    pub async fn record_verification(&self, insert: AuditInsert) -> anyhow::Result<String> {
        let created_at = Utc::now().to_rfc3339();
        let request_id = insert.response.request_id.to_string();
        let status = insert.response.status;
        let threshold_config = ThresholdConfigSnapshot {
            threshold_cents: insert.threshold_cents,
            window_days: insert.window_days,
        };
        let proof_metadata = insert.response.crypto_proof_metadata.clone();
        let message = insert.response.message.clone();
        let replay_warning = insert.response.replay_warning;
        let threshold_compliant = insert.response.threshold_compliant;
        let proof_audit_hash = hash_payload(&AuditHashPayload {
            request_id: request_id.clone(),
            created_at: created_at.clone(),
            user_key: insert.user_key.clone(),
            scenario: insert.scenario.clone(),
            amount_cents: insert.amount_cents,
            tx_id_commitment: insert.tx_id_commitment.0.clone(),
            status,
            threshold_compliant,
            threshold_config,
            proof_metadata: proof_metadata.clone(),
            replay_warning,
            message: message.clone(),
        })?;

        let threshold_config_json = serde_json::to_string(&ThresholdConfigSnapshot {
            threshold_cents: insert.threshold_cents,
            window_days: insert.window_days,
        })?;
        let proof_metadata_json = serde_json::to_string(&proof_metadata)?;
        let status_text = status_text(status);

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO audit_records (
                request_id, created_at, user_key, scenario, amount_cents, tx_id_commitment,
                status, threshold_compliant, threshold_cents, window_days, threshold_config_json,
                proof_metadata_json, proof_audit_hash, replay_warning, message
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&request_id)
        .bind(&created_at)
        .bind(&insert.user_key)
        .bind(insert.scenario.as_deref())
        .bind(u64_to_i64(insert.amount_cents)?)
        .bind(&insert.tx_id_commitment.0)
        .bind(status_text)
        .bind(bool_to_i64(threshold_compliant))
        .bind(u64_to_i64(insert.threshold_cents)?)
        .bind(i64::from(insert.window_days))
        .bind(&threshold_config_json)
        .bind(&proof_metadata_json)
        .bind(&proof_audit_hash)
        .bind(bool_to_i64(replay_warning))
        .bind(&message)
        .execute(&mut *tx)
        .await?;

        for commitment in &proof_metadata.commitment_handles {
            sqlx::query(
                r#"
                INSERT INTO commitment_ledger (
                    request_id, created_at, commitment_handle, status, record_hash
                )
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(&request_id)
            .bind(&created_at)
            .bind(&commitment.0)
            .bind(status_text)
            .bind(&proof_audit_hash)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(proof_audit_hash)
    }

    pub async fn get_proof(&self, request_id: &str) -> anyhow::Result<Option<ProofAuditRecord>> {
        let Some(row) = sqlx::query(
            r#"
            SELECT request_id, created_at, user_key, scenario, amount_cents, tx_id_commitment,
                   status, threshold_compliant, threshold_cents, window_days, proof_metadata_json,
                   proof_audit_hash, replay_warning, message
            FROM audit_records
            WHERE request_id = ?
            "#,
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(record_from_row(row)?))
    }

    pub async fn verify_proof_hash(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<ProofIntegrityResponse>> {
        let Some(record) = self.get_proof(request_id).await? else {
            return Ok(None);
        };
        let recomputed_hash = hash_payload(&AuditHashPayload {
            request_id: record.request_id.clone(),
            created_at: record.created_at.clone(),
            user_key: record.user_key.clone(),
            scenario: record.scenario.clone(),
            amount_cents: record.amount_cents,
            tx_id_commitment: record.tx_id_commitment.0.clone(),
            status: record.status,
            threshold_compliant: record.threshold_compliant,
            threshold_config: ThresholdConfigSnapshot {
                threshold_cents: record.threshold_cents,
                window_days: record.window_days,
            },
            proof_metadata: record.proof_metadata.clone(),
            replay_warning: record.replay_warning,
            message: record.message.clone(),
        })?;
        let valid = recomputed_hash == record.proof_audit_hash;
        Ok(Some(ProofIntegrityResponse {
            request_id: record.request_id,
            valid,
            stored_hash: record.proof_audit_hash,
            recomputed_hash,
            message: if valid {
                "Audit payload hash is valid.".to_string()
            } else {
                "Audit payload hash mismatch.".to_string()
            },
        }))
    }

    pub async fn list_commitments(&self) -> anyhow::Result<CommitmentLedgerResponse> {
        let rows = sqlx::query(
            r#"
            SELECT id, request_id, created_at, commitment_handle, status, record_hash
            FROM commitment_ledger
            ORDER BY id DESC
            LIMIT 100
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let entries = rows
            .into_iter()
            .map(|row| {
                Ok(CommitmentLedgerEntry {
                    id: row.try_get("id")?,
                    request_id: row.try_get("request_id")?,
                    created_at: row.try_get("created_at")?,
                    commitment_handle: CommitmentBytes(row.try_get("commitment_handle")?),
                    status: parse_status(row.try_get::<String, _>("status")?)?,
                    record_hash: row.try_get("record_hash")?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(CommitmentLedgerResponse { entries })
    }
}

fn create_parent_dir(database_url: &str) -> anyhow::Result<()> {
    if database_url.contains(":memory:") {
        return Ok(());
    }
    let Some(path) = database_url.strip_prefix("sqlite:") else {
        return Ok(());
    };
    let path = path.strip_prefix("//").unwrap_or(path);
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn record_from_row(row: sqlx::sqlite::SqliteRow) -> anyhow::Result<ProofAuditRecord> {
    Ok(ProofAuditRecord {
        request_id: row.try_get("request_id")?,
        created_at: row.try_get("created_at")?,
        user_key: row.try_get("user_key")?,
        scenario: row.try_get("scenario")?,
        amount_cents: i64_to_u64(row.try_get("amount_cents")?)?,
        tx_id_commitment: CommitmentBytes(row.try_get("tx_id_commitment")?),
        status: parse_status(row.try_get::<String, _>("status")?)?,
        threshold_compliant: i64_to_bool(row.try_get("threshold_compliant")?),
        threshold_cents: i64_to_u64(row.try_get("threshold_cents")?)?,
        window_days: i64_to_u32(row.try_get("window_days")?)?,
        proof_metadata: serde_json::from_str(&row.try_get::<String, _>("proof_metadata_json")?)?,
        proof_audit_hash: row.try_get("proof_audit_hash")?,
        replay_warning: i64_to_bool(row.try_get("replay_warning")?),
        message: row.try_get("message")?,
    })
}

fn hash_payload(payload: &AuditHashPayload) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(payload)?;
    let digest = Sha3_256::digest(bytes);
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn status_text(status: VerificationStatus) -> &'static str {
    match status {
        VerificationStatus::Approved => "APPROVED",
        VerificationStatus::Rejected => "REJECTED",
    }
}

fn parse_status(status: String) -> anyhow::Result<VerificationStatus> {
    match status.as_str() {
        "APPROVED" => Ok(VerificationStatus::Approved),
        "REJECTED" => Ok(VerificationStatus::Rejected),
        other => anyhow::bail!("unknown verification status in audit store: {other}"),
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn i64_to_bool(value: i64) -> bool {
    value != 0
}

fn u64_to_i64(value: u64) -> anyhow::Result<i64> {
    i64::try_from(value).map_err(|_| anyhow::anyhow!("value is too large for SQLite integer"))
}

fn i64_to_u64(value: i64) -> anyhow::Result<u64> {
    u64::try_from(value).map_err(|_| anyhow::anyhow!("negative integer in audit store"))
}

fn i64_to_u32(value: i64) -> anyhow::Result<u32> {
    u32::try_from(value).map_err(|_| anyhow::anyhow!("invalid u32 in audit store"))
}
