//! Request payloads accepted by the survivor endpoints.
//!
//! Kept apart from the domain types in [`crate::survivor::model`] the same way
//! the roster pool's are, so a change to the wire shape does not reach into the
//! model.

use serde::Deserialize;

use crate::survivor::model::SurvivorSettings;

#[derive(Debug, Deserialize, Clone)]
pub struct SurvivorCreationRequest {
    pub pool_name: String,
    pub settings: SurvivorSettings,
    /// The name the owner plays under. They join their own pool on creation —
    /// an owner who is not in it would have nothing to pick.
    pub participant_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SurvivorDeletionRequest {
    pub pool_name: String,
}

/// Signing yourself up. The participant is whoever the JWT names, never a name
/// in the body: a caller who could name somebody else could sign up the whole
/// pool.
#[derive(Debug, Deserialize, Clone)]
pub struct JoinSurvivorRequest {
    pub pool_name: String,
    pub participant_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LeaveSurvivorRequest {
    pub pool_name: String,
    /// Whose spot to give up. A participant sends their own id; the owner may
    /// send anyone's.
    pub participant_id: String,
}

/// Naming the team you are backing for a pick date.
#[derive(Debug, Deserialize, Clone)]
pub struct MakePickRequest {
    pub pool_name: String,
    pub week: u16,
    pub team_id: u32,
}

/// Closing a pick date and applying what the games did.
///
/// Idempotent on purpose: it is the owner's button today and would be a
/// scheduled job's call tomorrow, and neither should care whether the other
/// already ran.
#[derive(Debug, Deserialize, Clone)]
pub struct SettleWeekRequest {
    pub pool_name: String,
    pub week: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateSurvivorSettingsRequest {
    pub pool_name: String,
    pub settings: SurvivorSettings,
}
