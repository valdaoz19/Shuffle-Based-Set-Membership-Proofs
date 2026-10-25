use curve25519_dalek::RistrettoPoint;
use serde::{Deserialize, Serialize};

use crate::generator::generator_from_label;

/// Public parameters for the IBC scheme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicParams {
    pub g_iden: RistrettoPoint,
    pub g_val: RistrettoPoint,
    pub g_ran: RistrettoPoint,
}

impl PublicParams {
    pub fn setup() -> Self {
        Self {
            g_iden: generator_from_label(b"iden"),
            g_val: generator_from_label(b"val"),
            g_ran: generator_from_label(b"ran"),
        }
    }
}
