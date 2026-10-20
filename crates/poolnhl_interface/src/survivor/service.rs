use std::sync::Arc;

use async_trait::async_trait;

use crate::errors::Result;
use crate::survivor::model::{SurvivorPool, SurvivorPoolShort, SurvivorStandings};
use crate::survivor::picks::{SurvivorPickOptions, SurvivorPickView};
use crate::survivor::requests::{
    AddSurvivorParticipantRequest, JoinSurvivorRequest, LeaveSurvivorRequest, MakePickRequest,
    ParticipantLinkRequest, RenameSurvivorParticipantRequest, RequestParticipantLinkRequest,
    SettleWeekRequest, SurvivorCreationRequest, SurvivorDeletionRequest,
    UpdateSurvivorSettingsRequest,
};

#[async_trait]
pub trait SurvivorService {
    async fn init_indexes(&self) -> Result<()>;

    async fn get_pool_by_name(&self, name: &str) -> Result<SurvivorPool>;
    async fn list_pools(&self, season: u32) -> Result<Vec<SurvivorPoolShort>>;

    async fn create_pool(
        &self,
        user_id: &str,
        req: SurvivorCreationRequest,
    ) -> Result<SurvivorPool>;
    async fn delete_pool(&self, user_id: &str, req: SurvivorDeletionRequest) -> Result<()>;
    async fn update_settings(
        &self,
        user_id: &str,
        req: UpdateSurvivorSettingsRequest,
    ) -> Result<SurvivorPool>;

    async fn join_pool(&self, user_id: &str, req: JoinSurvivorRequest) -> Result<SurvivorPool>;

    /// Add a spot the organiser keeps on somebody's behalf, for the pool of a
    /// few friends where one person enters everybody.
    async fn add_participant(
        &self,
        user_id: &str,
        req: AddSurvivorParticipantRequest,
    ) -> Result<SurvivorPool>;
    async fn leave_pool(&self, user_id: &str, req: LeaveSurvivorRequest) -> Result<SurvivorPool>;

    /// Rename a participant, as the owner.
    ///
    /// Display name only — nothing in either collection is keyed by it — so it
    /// is allowed at any point of the season, settled dates included.
    async fn rename_participant(
        &self,
        user_id: &str,
        req: RenameSurvivorParticipantRequest,
    ) -> Result<SurvivorPool>;

    /// Offer one of the pool's spots to whoever signs in with an address.
    ///
    /// The four below are the survivor pool's half of the roster pool's
    /// pooler-link flow. A spot the organiser entered has no account behind it,
    /// so that person cannot pick for themselves; the owner files an invitation
    /// and the invitee turns it into the link, which moves the spot's id to
    /// their account and takes its picks with it.
    async fn request_participant_link(
        &self,
        user_id: &str,
        req: RequestParticipantLinkRequest,
    ) -> Result<SurvivorPool>;

    /// Withdraw a pending invitation. The owner's.
    async fn cancel_participant_link(
        &self,
        user_id: &str,
        req: ParticipantLinkRequest,
    ) -> Result<SurvivorPool>;

    /// Take a spot over, as the account the invitation was filed against.
    ///
    /// `email` is the address Hanko verified and put in the caller's JWT: it is
    /// what the invitation is matched against, so a caller cannot accept one
    /// filed for somebody else.
    async fn accept_participant_link(
        &self,
        user_id: &str,
        email: &str,
        req: ParticipantLinkRequest,
    ) -> Result<SurvivorPool>;

    /// Turn down an invitation filed against the caller's own address.
    async fn decline_participant_link(
        &self,
        email: &str,
        req: ParticipantLinkRequest,
    ) -> Result<SurvivorPool>;

    /// What a participant needs to pick for a date: the teams playing, the ones
    /// still open to them, and the pick they already have in.
    /// `participant_id` is `None` for the caller's own screen. The owner and
    /// the assistants may name somebody else, which is how they see what is
    /// still open to a spot they keep on that person's behalf — and nobody
    /// else may, because an open date's pick is on this screen.
    async fn get_pick_options(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
        participant_id: Option<&str>,
    ) -> Result<SurvivorPickOptions>;

    async fn make_pick(&self, user_id: &str, req: MakePickRequest) -> Result<SurvivorPickOptions>;

    /// Every pick of a date, for the reveal.
    ///
    /// Answers with nothing but the caller's own pick while the date is still
    /// taking them: seeing the field before you pick is the one thing that
    /// would break the game. The reveal comes with the date's last puck drop,
    /// after which nobody's pick can change.
    async fn get_week_picks(
        &self,
        user_id: &str,
        pool_name: &str,
        week: u16,
    ) -> Result<Vec<SurvivorPickView>>;

    /// A participant's own picks across the season, which is where the teams
    /// they have spent comes from.
    async fn get_my_picks(&self, user_id: &str, pool_name: &str) -> Result<Vec<SurvivorPickView>>;

    /// Close a date and apply what the games did. Idempotent.
    ///
    /// The one date-level action left. Closing a date to picks used to be a
    /// second one, pressed at the first puck drop to reveal the picks; the
    /// deadline is read from the games themselves now, so what is left here is
    /// the half no clock can derive — reading the results and eliminating
    /// whoever ran out of strikes. It refuses a date with a game still to
    /// finish, and running it twice changes nothing, so it would suit a
    /// scheduled job as well as the owner's button.
    async fn settle_week(&self, user_id: &str, req: SettleWeekRequest) -> Result<SurvivorPool>;

    /// The standings, aggregated server-side so a pool of hundreds does not ship
    /// every pick to the browser to be counted there.
    async fn get_standings(&self, pool_name: &str) -> Result<SurvivorStandings>;
}

pub type SurvivorServiceHandle = Arc<dyn SurvivorService + Send + Sync>;
