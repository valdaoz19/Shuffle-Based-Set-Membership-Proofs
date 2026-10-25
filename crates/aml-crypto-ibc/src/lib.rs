//! Identity Binding Commitment (IBC) on Ristretto255.
//!
//! Commitment: c = g_iden^id · g_val^v · g_ran^r

mod commitment;
mod generator;
mod ideq;
mod params;
mod vver;

pub use commitment::{IbcCommitment, IbcOpening, IbcScheme};
pub use ideq::{IdEqProof, IdEqProver, IdEqVerifier};
pub use params::PublicParams;
pub use vver::{VVerProof, VVerProver, VVerVerifier};
