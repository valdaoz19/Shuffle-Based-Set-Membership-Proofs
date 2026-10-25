use std::{fmt, str::FromStr, time::Instant};

use aml_core::{AmlError, AmlResult};
use aml_crypto_ibc::{IbcCommitment, IbcScheme, PublicParams};
use aml_crypto_shuffle::{RerandomizationProof, ShuffleProof, VerifiableShuffle};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::seq::SliceRandom;
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

pub const DEFAULT_MEMBERSHIP_WARN_TIMEOUT_MS: u64 = 1_000;
pub const DEFAULT_MEMBERSHIP_HARD_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MembershipMode {
    #[default]
    Explicit,
    Implicit,
    Hybrid,
}

impl MembershipMode {
    pub const ALL: [MembershipMode; 3] = [
        MembershipMode::Explicit,
        MembershipMode::Implicit,
        MembershipMode::Hybrid,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            MembershipMode::Explicit => "explicit",
            MembershipMode::Implicit => "implicit",
            MembershipMode::Hybrid => "hybrid",
        }
    }
}

impl fmt::Display for MembershipMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MembershipMode {
    type Err = AmlError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "explicit" | "shuffle" | "explicit_shuffle" => Ok(Self::Explicit),
            "implicit" | "vector" | "vc" | "vector_commitment" => Ok(Self::Implicit),
            "hybrid" | "bucket" | "bucketed" => Ok(Self::Hybrid),
            other => Err(AmlError::InvalidInput(format!(
                "unknown membership mode: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipMetricThresholds {
    pub proof_generation_ms: u64,
    pub verification_ms: u64,
    pub proof_size_bytes: usize,
    pub warn_timeout_ms: u64,
    pub hard_timeout_ms: u64,
}

impl MembershipMetricThresholds {
    pub fn for_mode(mode: MembershipMode) -> Self {
        let (proof_generation_ms, verification_ms, proof_size_bytes) = match mode {
            MembershipMode::Explicit => (120, 500, 64 * 1024),
            MembershipMode::Implicit => (20, 10, 2 * 1024),
            MembershipMode::Hybrid => (60, 80, 16 * 1024),
        };
        Self {
            proof_generation_ms,
            verification_ms,
            proof_size_bytes,
            warn_timeout_ms: env_u64(
                "AML_MEMBERSHIP_WARN_TIMEOUT_MS",
                DEFAULT_MEMBERSHIP_WARN_TIMEOUT_MS,
            ),
            hard_timeout_ms: env_u64(
                "AML_MEMBERSHIP_HARD_TIMEOUT_MS",
                DEFAULT_MEMBERSHIP_HARD_TIMEOUT_MS,
            ),
        }
    }

    pub fn verdict(&self, generation_ms: u64, verification_ms: u64, proof_size: usize) -> String {
        if generation_ms > self.hard_timeout_ms || verification_ms > self.hard_timeout_ms {
            "fail_timeout".into()
        } else if generation_ms > self.proof_generation_ms
            || verification_ms > self.verification_ms
            || proof_size > self.proof_size_bytes
        {
            "fail".into()
        } else if generation_ms > self.warn_timeout_ms || verification_ms > self.warn_timeout_ms {
            "pass_warn".into()
        } else {
            "pass".into()
        }
    }
}

#[derive(Debug, Clone)]
pub struct MembershipStatement {
    pub set_commitments: Vec<IbcCommitment>,
    pub target: IbcCommitment,
    pub set_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipProofEnvelope {
    pub mode: MembershipMode,
    pub proof_json: String,
    pub proof_size_bytes: usize,
    pub generation_time_ms: u64,
    pub verification_time_ms: u64,
    pub threshold_gen_ms: u64,
    pub threshold_verify_ms: u64,
    pub threshold_size_bytes: usize,
    pub warn_timeout_ms: u64,
    pub hard_timeout_ms: u64,
    pub verdict: String,
    pub implementation: String,
    pub current_primitive: String,
    pub formal_target: String,
    pub formal_gap: String,
    pub trust_anchor_current: String,
    pub trust_anchor_ideal: String,
    pub claim_level: String,
}

pub trait MembershipScheme {
    fn mode(&self) -> MembershipMode;

    fn prove<R: RngCore + CryptoRng>(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        rng: &mut R,
    ) -> AmlResult<MembershipProofEnvelope>;

    fn verify(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        envelope: &MembershipProofEnvelope,
    ) -> AmlResult<bool>;
}

pub enum MembershipSchemeImpl {
    Explicit(ExplicitMembershipScheme),
    Implicit(ImplicitMembershipScheme),
    Hybrid(HybridMembershipScheme),
}

impl MembershipSchemeImpl {
    pub fn for_mode(mode: MembershipMode) -> Self {
        match mode {
            MembershipMode::Explicit => Self::Explicit(ExplicitMembershipScheme),
            MembershipMode::Implicit => Self::Implicit(ImplicitMembershipScheme),
            MembershipMode::Hybrid => Self::Hybrid(HybridMembershipScheme::default()),
        }
    }

    pub fn mode(&self) -> MembershipMode {
        match self {
            Self::Explicit(scheme) => scheme.mode(),
            Self::Implicit(scheme) => scheme.mode(),
            Self::Hybrid(scheme) => scheme.mode(),
        }
    }

    pub fn prove<R: RngCore + CryptoRng>(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        rng: &mut R,
    ) -> AmlResult<MembershipProofEnvelope> {
        match self {
            Self::Explicit(scheme) => scheme.prove(pp, statement, rng),
            Self::Implicit(scheme) => scheme.prove(pp, statement, rng),
            Self::Hybrid(scheme) => scheme.prove(pp, statement, rng),
        }
    }

    pub fn verify(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        envelope: &MembershipProofEnvelope,
    ) -> AmlResult<bool> {
        match self {
            Self::Explicit(scheme) => scheme.verify(pp, statement, envelope),
            Self::Implicit(scheme) => scheme.verify(pp, statement, envelope),
            Self::Hybrid(scheme) => scheme.verify(pp, statement, envelope),
        }
    }
}

pub struct ExplicitMembershipScheme;

impl MembershipScheme for ExplicitMembershipScheme {
    fn mode(&self) -> MembershipMode {
        MembershipMode::Explicit
    }

    fn prove<R: RngCore + CryptoRng>(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        rng: &mut R,
    ) -> AmlResult<MembershipProofEnvelope> {
        let started = Instant::now();
        let proof =
            SetMembershipProver::prove(pp, &statement.set_commitments, &statement.target, rng)?;
        let generation_time_ms = started.elapsed().as_millis() as u64;
        let proof_json = proof.to_json()?;
        let mut envelope = envelope(
            self.mode(),
            proof_json,
            generation_time_ms,
            "explicit_shuffle_sigma_membership",
        );
        apply_thresholds(&mut envelope);
        Ok(envelope)
    }

    fn verify(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        envelope: &MembershipProofEnvelope,
    ) -> AmlResult<bool> {
        if envelope.mode != self.mode() {
            return Ok(false);
        }
        let proof = SetMembershipProof::from_json(&envelope.proof_json)?;
        Ok(SetMembershipVerifier::verify(
            pp,
            &statement.set_commitments,
            &statement.target,
            &proof,
        ))
    }
}

pub struct ImplicitMembershipScheme;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ImplicitMembershipProof {
    root: String,
    target_leaf: String,
    index: usize,
    set_size: usize,
    path: Vec<MerkleSibling>,
    poc_implementation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MerkleSibling {
    hash: String,
    sibling_on_left: bool,
}

impl MembershipScheme for ImplicitMembershipScheme {
    fn mode(&self) -> MembershipMode {
        MembershipMode::Implicit
    }

    fn prove<R: RngCore + CryptoRng>(
        &self,
        _pp: &PublicParams,
        statement: &MembershipStatement,
        _rng: &mut R,
    ) -> AmlResult<MembershipProofEnvelope> {
        let started = Instant::now();
        let proof = build_merkle_proof(statement)?;
        let proof_json =
            serde_json::to_string(&proof).map_err(|e| AmlError::Serialization(e.to_string()))?;
        let generation_time_ms = started.elapsed().as_millis() as u64;
        let mut envelope = envelope(
            self.mode(),
            proof_json,
            generation_time_ms,
            "vector_commitment_like_merkle_poc",
        );
        apply_thresholds(&mut envelope);
        Ok(envelope)
    }

    fn verify(
        &self,
        _pp: &PublicParams,
        statement: &MembershipStatement,
        envelope: &MembershipProofEnvelope,
    ) -> AmlResult<bool> {
        if envelope.mode != self.mode() {
            return Ok(false);
        }
        let proof: ImplicitMembershipProof = serde_json::from_str(&envelope.proof_json)
            .map_err(|e| AmlError::Serialization(e.to_string()))?;
        if proof.set_size != statement.set_commitments.len() {
            return Ok(false);
        }
        let target_leaf = leaf_hash(&statement.target);
        if proof.target_leaf != encode_hash(&target_leaf) {
            return Ok(false);
        }
        if statement
            .set_commitments
            .get(proof.index)
            .is_none_or(|candidate| candidate != &statement.target)
        {
            return Ok(false);
        }
        let root = merkle_root(&statement.set_commitments);
        let computed = verify_merkle_path(target_leaf, &proof.path)?;
        Ok(proof.root == encode_hash(&root) && proof.root == encode_hash(&computed))
    }
}

#[derive(Debug, Clone)]
pub struct HybridMembershipScheme {
    bucket_count: usize,
}

impl Default for HybridMembershipScheme {
    fn default() -> Self {
        Self { bucket_count: 8 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HybridMembershipProof {
    bucket_index: usize,
    bucket_count: usize,
    bucket_digest: String,
    subset_commitments: Vec<String>,
    subset_explicit_proof: SetMembershipProof,
    poc_implementation: String,
}

impl MembershipScheme for HybridMembershipScheme {
    fn mode(&self) -> MembershipMode {
        MembershipMode::Hybrid
    }

    fn prove<R: RngCore + CryptoRng>(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        rng: &mut R,
    ) -> AmlResult<MembershipProofEnvelope> {
        let started = Instant::now();
        let target_bucket = bucket_for(&statement.target, self.bucket_count);
        let subset: Vec<IbcCommitment> = statement
            .set_commitments
            .iter()
            .copied()
            .filter(|commitment| bucket_for(commitment, self.bucket_count) == target_bucket)
            .collect();
        if subset.is_empty() {
            return Err(AmlError::InvalidInput(
                "hybrid membership bucket is empty".into(),
            ));
        }
        let explicit =
            SetMembershipProver::prove(pp, &subset, &statement.target, rng).map_err(|_| {
                AmlError::InvalidInput("target commitment is not in hybrid bucket".into())
            })?;
        let subset_commitments = subset
            .iter()
            .map(IbcScheme::encode)
            .collect::<AmlResult<Vec<_>>>()?;
        let proof = HybridMembershipProof {
            bucket_index: target_bucket,
            bucket_count: self.bucket_count,
            bucket_digest: encode_hash(&bucket_digest(&subset)),
            subset_commitments,
            subset_explicit_proof: explicit,
            poc_implementation: "bucketed_subset_plus_explicit_sigma_membership".into(),
        };
        let proof_json =
            serde_json::to_string(&proof).map_err(|e| AmlError::Serialization(e.to_string()))?;
        let generation_time_ms = started.elapsed().as_millis() as u64;
        let mut envelope = envelope(
            self.mode(),
            proof_json,
            generation_time_ms,
            "bucketed_subset_plus_explicit_sigma_membership",
        );
        apply_thresholds(&mut envelope);
        Ok(envelope)
    }

    fn verify(
        &self,
        pp: &PublicParams,
        statement: &MembershipStatement,
        envelope: &MembershipProofEnvelope,
    ) -> AmlResult<bool> {
        if envelope.mode != self.mode() {
            return Ok(false);
        }
        let proof: HybridMembershipProof = serde_json::from_str(&envelope.proof_json)
            .map_err(|e| AmlError::Serialization(e.to_string()))?;
        if proof.bucket_count == 0 || proof.bucket_count != self.bucket_count {
            return Ok(false);
        }
        let expected_bucket = bucket_for(&statement.target, proof.bucket_count);
        if proof.bucket_index != expected_bucket {
            return Ok(false);
        }
        let expected_subset: Vec<IbcCommitment> = statement
            .set_commitments
            .iter()
            .copied()
            .filter(|commitment| bucket_for(commitment, proof.bucket_count) == proof.bucket_index)
            .collect();
        let proof_subset = proof
            .subset_commitments
            .iter()
            .map(|encoded| IbcScheme::decode(encoded))
            .collect::<AmlResult<Vec<_>>>()?;
        if expected_subset != proof_subset {
            return Ok(false);
        }
        if proof.bucket_digest != encode_hash(&bucket_digest(&expected_subset)) {
            return Ok(false);
        }
        Ok(SetMembershipVerifier::verify(
            pp,
            &expected_subset,
            &statement.target,
            &proof.subset_explicit_proof,
        ))
    }
}

/// Explicit shuffle-based set membership proof for PoC.
///
/// The prover rerandomizes and shuffles a commitment set so the target member is
/// placed at output index 0, then proves the target commitment rerandomizes to
/// that same first output. This matches the stakeholder's explicit/shuffle-based
/// direction while staying consistent with the existing sigma shuffle PoC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMembershipProof {
    pub shuffled_set: ShuffleProof,
    pub target_link: RerandomizationProof,
    pub set_size: usize,
    pub poc_implementation: String,
}

pub struct SetMembershipProver;

impl SetMembershipProver {
    pub fn prove<R: RngCore + CryptoRng>(
        pp: &PublicParams,
        set: &[IbcCommitment],
        target: &IbcCommitment,
        rng: &mut R,
    ) -> AmlResult<SetMembershipProof> {
        if set.is_empty() {
            return Err(AmlError::InvalidInput("membership set is empty".into()));
        }

        let witness_index = set
            .iter()
            .position(|candidate| candidate == target)
            .ok_or_else(|| AmlError::InvalidInput("target commitment is not in set".into()))?;

        let mut tail: Vec<usize> = (0..set.len())
            .filter(|index| *index != witness_index)
            .collect();
        tail.shuffle(rng);

        let mut permutation = Vec::with_capacity(set.len());
        permutation.push(witness_index);
        permutation.extend(tail);

        let mut outputs = Vec::with_capacity(set.len());
        let mut deltas = Vec::with_capacity(set.len());
        for &input_index in &permutation {
            let (output, delta) = IbcScheme::rerandomize(pp, &set[input_index], rng);
            outputs.push(output);
            deltas.push(delta);
        }

        let shuffled_set = VerifiableShuffle::prove(pp, set, &outputs, &permutation, &deltas, rng);
        let target_link = RerandomizationProof::prove(pp, target, &outputs[0], &deltas[0], rng);

        Ok(SetMembershipProof {
            shuffled_set,
            target_link,
            set_size: set.len(),
            poc_implementation: "explicit_shuffle_sigma_membership".into(),
        })
    }
}

pub struct SetMembershipVerifier;

impl SetMembershipVerifier {
    pub fn verify(
        pp: &PublicParams,
        set: &[IbcCommitment],
        target: &IbcCommitment,
        proof: &SetMembershipProof,
    ) -> bool {
        if set.is_empty() || proof.set_size != set.len() {
            return false;
        }
        let Some(first_output) = proof.shuffled_set.outputs.first() else {
            return false;
        };
        VerifiableShuffle::verify(pp, set, &proof.shuffled_set)
            && proof.target_link.verify(pp, target, first_output)
    }
}

impl SetMembershipProof {
    pub fn to_json(&self) -> AmlResult<String> {
        serde_json::to_string(self).map_err(|e| AmlError::Serialization(e.to_string()))
    }

    pub fn from_json(json: &str) -> AmlResult<Self> {
        serde_json::from_str(json).map_err(|e| AmlError::Serialization(e.to_string()))
    }
}

pub fn membership_mode_from_env() -> MembershipMode {
    std::env::var("AML_MEMBERSHIP_MODE")
        .ok()
        .and_then(|value| MembershipMode::from_str(&value).ok())
        .unwrap_or_default()
}

pub fn parse_membership_modes(value: &str) -> AmlResult<Vec<MembershipMode>> {
    let mut modes = Vec::new();
    for raw in value.split(',') {
        let mode = MembershipMode::from_str(raw)?;
        if !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    if modes.is_empty() {
        return Err(AmlError::InvalidInput(
            "membership mode list is empty".into(),
        ));
    }
    Ok(modes)
}

pub fn finalize_membership_envelope(
    envelope: &mut MembershipProofEnvelope,
    verification_time_ms: u64,
) {
    envelope.verification_time_ms = verification_time_ms;
    apply_thresholds(envelope);
}

fn envelope(
    mode: MembershipMode,
    proof_json: String,
    generation_time_ms: u64,
    implementation: &str,
) -> MembershipProofEnvelope {
    let thresholds = MembershipMetricThresholds::for_mode(mode);
    let proof_size_bytes = proof_json.len();
    let profile = security_profile_for_mode(mode);
    MembershipProofEnvelope {
        mode,
        proof_json,
        proof_size_bytes,
        generation_time_ms,
        verification_time_ms: 0,
        threshold_gen_ms: thresholds.proof_generation_ms,
        threshold_verify_ms: thresholds.verification_ms,
        threshold_size_bytes: thresholds.proof_size_bytes,
        warn_timeout_ms: thresholds.warn_timeout_ms,
        hard_timeout_ms: thresholds.hard_timeout_ms,
        verdict: "measured".into(),
        implementation: implementation.into(),
        current_primitive: profile.current_primitive.into(),
        formal_target: profile.formal_target.into(),
        formal_gap: profile.formal_gap.into(),
        trust_anchor_current: profile.trust_anchor_current.into(),
        trust_anchor_ideal: profile.trust_anchor_ideal.into(),
        claim_level: profile.claim_level.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MembershipSecurityProfile {
    pub current_primitive: &'static str,
    pub formal_target: &'static str,
    pub formal_gap: &'static str,
    pub trust_anchor_current: &'static str,
    pub trust_anchor_ideal: &'static str,
    pub claim_level: &'static str,
}

pub fn security_profile_for_mode(mode: MembershipMode) -> MembershipSecurityProfile {
    match mode {
        MembershipMode::Explicit => MembershipSecurityProfile {
            current_primitive: "sigma rerandomization membership over a shuffled set",
            formal_target: "formal verifiable shuffle proof, e.g. Bayer-Groth-style shuffle argument",
            formal_gap: "current proof verifies rerandomization links and target linkage, but does not implement a formal shuffle proof with the same security claim as Bayer-Groth",
            trust_anchor_current: "Alipay+ receives the full PayPay commitment set and verifies PoC rerandomization links against it",
            trust_anchor_ideal: "Alipay+ verifies a formal shuffle argument against an agreed set commitment/transcript without trusting PayPay's ordering claim",
            claim_level: "poc_baseline",
        },
        MembershipMode::Implicit => MembershipSecurityProfile {
            current_primitive: "Merkle membership proof over H_leaf(c_tid)",
            formal_target: "vector commitment opening proof, e.g. KZG/vector-commitment-style opening",
            formal_gap: "current proof is a hash-tree path, not a production vector commitment proof with constant-size algebraic opening",
            trust_anchor_current: "Alipay+ recomputes the Merkle root from the PayPay-provided commitment set",
            trust_anchor_ideal: "Alipay+ holds or obtains a trusted global vector commitment root R_S and verifies an opening proof for c_tid",
            claim_level: "poc_baseline",
        },
        MembershipMode::Hybrid => MembershipSecurityProfile {
            current_primitive: "bucketed subset selection plus explicit sigma rerandomization proof on the subset",
            formal_target: "global commitment proof for bucket/subset consistency plus subset membership proof",
            formal_gap: "current proof checks a bucket digest and recomputed subset, but has no pi_global proving the bucket comes from a trusted global commitment R_S",
            trust_anchor_current: "Alipay+ recomputes expected_subset from the PayPay-provided full set_commitments",
            trust_anchor_ideal: "Alipay+ verifies VerifyGlobal(R_S,j,D_j,pi_global)=1 before accepting subset membership",
            claim_level: "poc_baseline",
        },
    }
}

fn apply_thresholds(envelope: &mut MembershipProofEnvelope) {
    let thresholds = MembershipMetricThresholds::for_mode(envelope.mode);
    envelope.verdict = thresholds.verdict(
        envelope.generation_time_ms,
        envelope.verification_time_ms,
        envelope.proof_size_bytes,
    );
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn build_merkle_proof(statement: &MembershipStatement) -> AmlResult<ImplicitMembershipProof> {
    let index = statement
        .set_commitments
        .iter()
        .position(|candidate| candidate == &statement.target)
        .ok_or_else(|| AmlError::InvalidInput("target commitment is not in set".into()))?;
    let leaves = statement
        .set_commitments
        .iter()
        .map(leaf_hash)
        .collect::<Vec<_>>();
    let root = root_from_leaves(leaves.clone());
    let path = merkle_path(leaves, index);
    Ok(ImplicitMembershipProof {
        root: encode_hash(&root),
        target_leaf: encode_hash(&leaf_hash(&statement.target)),
        index,
        set_size: statement.set_commitments.len(),
        path,
        poc_implementation: "vector_commitment_like_merkle_poc".into(),
    })
}

fn leaf_hash(commitment: &IbcCommitment) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(b"aml-membership-leaf-v1");
    hasher.update(commitment.0.compress().as_bytes());
    hasher.finalize().into()
}

fn merkle_root(commitments: &[IbcCommitment]) -> [u8; 32] {
    root_from_leaves(commitments.iter().map(leaf_hash).collect())
}

fn root_from_leaves(mut level: Vec<[u8; 32]>) -> [u8; 32] {
    if level.is_empty() {
        return hash_pair(&[0; 32], &[0; 32]);
    }
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().expect("level is not empty");
            level.push(last);
        }
        level = level
            .chunks(2)
            .map(|pair| hash_pair(&pair[0], &pair[1]))
            .collect();
    }
    level[0]
}

fn merkle_path(mut level: Vec<[u8; 32]>, mut index: usize) -> Vec<MerkleSibling> {
    let mut path = Vec::new();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().expect("level is not empty");
            level.push(last);
        }
        let sibling_index = if index.is_multiple_of(2) {
            index + 1
        } else {
            index - 1
        };
        path.push(MerkleSibling {
            hash: encode_hash(&level[sibling_index]),
            sibling_on_left: !index.is_multiple_of(2),
        });
        index /= 2;
        level = level
            .chunks(2)
            .map(|pair| hash_pair(&pair[0], &pair[1]))
            .collect();
    }
    path
}

fn verify_merkle_path(mut current: [u8; 32], path: &[MerkleSibling]) -> AmlResult<[u8; 32]> {
    for sibling in path {
        let sibling_hash = decode_hash(&sibling.hash)?;
        current = if sibling.sibling_on_left {
            hash_pair(&sibling_hash, &current)
        } else {
            hash_pair(&current, &sibling_hash)
        };
    }
    Ok(current)
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(b"aml-membership-node-v1");
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn bucket_for(commitment: &IbcCommitment, bucket_count: usize) -> usize {
    let leaf = leaf_hash(commitment);
    usize::from(leaf[0]) % bucket_count.max(1)
}

fn bucket_digest(subset: &[IbcCommitment]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(b"aml-membership-bucket-v1");
    for commitment in subset {
        hasher.update(commitment.0.compress().as_bytes());
    }
    hasher.finalize().into()
}

fn encode_hash(hash: &[u8; 32]) -> String {
    URL_SAFE_NO_PAD.encode(hash)
}

fn decode_hash(encoded: &str) -> AmlResult<[u8; 32]> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| AmlError::Serialization(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(AmlError::Serialization("invalid hash length".into()));
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aml_crypto_ibc::IbcScheme;
    use rand::rngs::OsRng;

    fn fixture() -> (PublicParams, Vec<IbcCommitment>, IbcCommitment) {
        let pp = PublicParams::setup();
        let mut rng = OsRng;
        let mut set = Vec::new();
        for id in 0..16 {
            let (commitment, _) = IbcScheme::commit(&pp, id, 0, &mut rng);
            set.push(commitment);
        }
        let target = set[7];
        (pp, set, target)
    }

    #[test]
    fn proves_target_is_member_of_set() {
        let (pp, set, target) = fixture();
        let mut rng = OsRng;

        let proof = SetMembershipProver::prove(&pp, &set, &target, &mut rng).unwrap();
        assert!(SetMembershipVerifier::verify(&pp, &set, &target, &proof));
    }

    #[test]
    fn rejects_non_member_target() {
        let (pp, set, _) = fixture();
        let mut rng = OsRng;
        let (target, _) = IbcScheme::commit(&pp, 99, 0, &mut rng);

        assert!(SetMembershipProver::prove(&pp, &set, &target, &mut rng).is_err());
    }

    #[test]
    fn all_membership_schemes_verify() {
        let (pp, set, target) = fixture();
        let mut rng = OsRng;
        let statement = MembershipStatement {
            set_commitments: set,
            target,
            set_id: "test".into(),
        };

        for mode in MembershipMode::ALL {
            let scheme = MembershipSchemeImpl::for_mode(mode);
            let envelope = scheme.prove(&pp, &statement, &mut rng).unwrap();
            assert_eq!(envelope.mode, mode);
            assert!(scheme.verify(&pp, &statement, &envelope).unwrap());
        }
    }

    #[test]
    fn parses_membership_modes() {
        let modes = parse_membership_modes("explicit,implicit,hybrid").unwrap();
        assert_eq!(modes, MembershipMode::ALL);
    }
}
