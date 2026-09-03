use axum::{
    Router,
    extract::{
        Json, Path, State,
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft::model::{Command, RoomUser};
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::users::model::UserEmailJwtPayload;

use crate::metrics as app_metrics;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

// A socket that connects and never sends JoinRoom would otherwise sit there
// indefinitely, holding a task without ever having authenticated.
const JOIN_ROOM_TIMEOUT: Duration = Duration::from_secs(10);

// Keepalive period. Without pings, a half-open connection (mobile handover,
// NAT timeout, laptop lid) is indistinguishable from an idle one: `recv()`
// simply never returns, so `leave_room` never runs and the member stays in the
// room forever — the room heartbeat keeps re-arming its redis TTL because this
// instance still believes it owns a live socket for it.
const PING_PERIOD: Duration = Duration::from_secs(20);

// How long the closing frame is given to reach the client before the writer is
// torn down with the socket.
const CLOSE_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

// Stable label for a command. A &'static str rather than the serde tag so the
// label set is closed by construction — an unbounded `command` label would let
// a client mint new series at will.
fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Auth { .. } => "auth",
        Command::JoinRoom { .. } => "join_room",
        Command::LeaveRoom => "leave_room",
        Command::OnPoolSettingChanges { .. } => "on_pool_setting_changes",
        Command::OnReady => "on_ready",
        Command::AddUser { .. } => "add_user",
        Command::RemoveUser { .. } => "remove_user",
        Command::StartDraft { .. } => "start_draft",
        Command::DraftPlayer { .. } => "draft_player",
        Command::UndoDraftPlayer => "undo_draft_player",
        Command::ModifyRoster(_) => "modify_roster",
    }
}

// Records a command's duration and outcome when it goes out of scope.
//
// Drop rather than an explicit call at the end because `Command::LeaveRoom`
// returns straight out of the read task — an explicit record would be skipped
// on exactly the path worth measuring.
struct CommandTimer {
    command: &'static str,
    started: Instant,
    outcome: &'static str,
}

impl CommandTimer {
    fn start(command: &'static str) -> Self {
        Self {
            command,
            started: Instant::now(),
            outcome: "ok",
        }
    }

    fn failed(&mut self) {
        self.outcome = "error";
    }
}

impl Drop for CommandTimer {
    fn drop(&mut self) {
        let labels = [("command", self.command), ("outcome", self.outcome)];
        metrics::counter!(app_metrics::COMMAND_TOTAL, &labels).increment(1);
        metrics::histogram!(app_metrics::COMMAND_DURATION, &[("command", self.command)])
            .record(self.started.elapsed().as_secs_f64());
    }
}

// Keeps the open-socket gauge honest. The socket task has several exit paths
// (join timeout, client close, keepalive failure) and can also be aborted, so
// decrementing by hand at each one is a leak waiting to happen: a gauge that
// only ever goes up is worse than no gauge, because it reads as load.
struct ConnectionGuard {
    reason: &'static str,
}

impl ConnectionGuard {
    fn new() -> Self {
        metrics::counter!(app_metrics::WS_CONNECTED_TOTAL).increment(1);
        metrics::gauge!(app_metrics::WS_CONNECTIONS).increment(1.0);
        Self { reason: "unknown" }
    }

    fn closed_because(&mut self, reason: &'static str) {
        self.reason = reason;
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        metrics::gauge!(app_metrics::WS_CONNECTIONS).decrement(1.0);
        metrics::counter!(app_metrics::WS_CLOSED_TOTAL, &[("reason", self.reason)]).increment(1);
    }
}

pub struct DraftRouter;

impl DraftRouter {
    pub fn router(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/ws", get(Self::ws_handler))
            .route("/rooms", get(Self::list_rooms))
            .route("/room-users/:room", get(Self::list_room_users))
            .with_state(service_registry)
    }

    async fn list_rooms(
        State(draft_service): State<DraftServiceHandle>,
    ) -> Result<Json<Vec<String>>> {
        draft_service.list_rooms().await.map(Json)
    }

    async fn list_room_users(
        State(draft_service): State<DraftServiceHandle>,
        Path(pool_name): Path<String>,
    ) -> Result<Json<HashMap<String, RoomUser>>> {
        draft_service.list_room_users(&pool_name).await.map(Json)
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        State(draft_service): State<DraftServiceHandle>,
    ) -> impl IntoResponse {
        // One id per connection. Not derived from the peer address: see the
        // note on the DraftService room commands.
        let socket_id = Uuid::new_v4().to_string();

        ws.on_upgrade(move |socket| {
            Self::handle_socket(socket, None, socket_id, addr, draft_service)
        })
    }

    // The initial socket state.
    // Waits for the client socket to send the JoinRoom command.
    // before leaving the state. It returns the the receiver and the room name.
    async fn waiting_join_room_command(
        socket: &mut WebSocket,
        socket_id: &str,
        draft_service: &DraftServiceHandle,
        user: &mut Option<UserEmailJwtPayload>,
    ) -> Result<(broadcast::Receiver<String>, String)> {
        while let Some(Ok(msg)) = socket.recv().await {
            if let Message::Text(command) = msg {
                tracing::debug!(%command, "draft socket command received");
                if let Ok(command) = serde_json::from_str::<Command>(&command) {
                    match command {
                        Command::Auth { token } => {
                            // A failed token leaves the socket unauthenticated
                            // rather than closing it: the draft board is
                            // readable without an account, and this is the same
                            // outcome the path-token route produced for a bad
                            // token.
                            *user = draft_service
                                .authenticate_web_socket(&token, socket_id)
                                .await;
                            match user {
                                Some(u) => tracing::debug!(
                                    user = %u.sub,
                                    %socket_id,
                                    "draft socket authenticated from first frame"
                                ),
                                None => tracing::debug!(
                                    %socket_id,
                                    "draft socket auth frame rejected"
                                ),
                            }
                            continue;
                        }
                        Command::JoinRoom {
                            pool_name,
                            number_poolers,
                        } => {
                            // join the requested room.
                            let rx = draft_service
                                .join_room(&pool_name, number_poolers, socket_id)
                                .await?;
                            tracing::debug!(%pool_name, "draft room joined");

                            return Ok((rx, pool_name));
                        }
                        _ => continue,
                    }
                }
                tracing::debug!("could not deserialize the draft socket command");
            } else {
                tracing::debug!("non-text frame on the draft socket, ignored");
            }
        }
        Err(AppError::CustomError {
            msg: "Could not join a room.".to_string(),
        })
    }

    async fn handle_socket(
        mut socket: WebSocket,
        user: Option<UserEmailJwtPayload>,
        socket_id: String,
        addr: SocketAddr,
        draft_service: DraftServiceHandle,
    ) {
        // Counts the socket as open for as long as this function runs; the
        // Drop impl is what releases it on every exit path.
        let mut connection = ConnectionGuard::new();

        // At the beginning there is a state where the user needs to join a room
        // before leaving the initial socket state.
        let mut user = user;
        tracing::debug!(%socket_id, %addr, "draft socket connecting");

        // Joining is bounded: a socket does not get to stall in the initial
        // state and keep the task alive for free. The same bound now covers
        // authentication, which happens inside this wait.
        let joined = tokio::time::timeout(
            JOIN_ROOM_TIMEOUT,
            Self::waiting_join_room_command(&mut socket, &socket_id, &draft_service, &mut user),
        )
        .await;

        match joined {
            Err(_elapsed) => {
                connection.closed_because("no_join_timeout");
                tracing::debug!(%socket_id, "draft socket closed: no JoinRoom before the timeout");
                let _ = socket.send(Message::Close(None)).await;
            }
            // The socket never joined a room; nothing to clean up, just close it.
            Ok(Err(e)) => {
                connection.closed_because("join_failed");
                tracing::debug!(error = %e, %socket_id, "draft socket closed before joining a room")
            }
            Ok(Ok((mut rx, current_pool_name))) => {
                // Read here rather than before the join: on the tokenless route
                // the identity is only known once the `Auth` frame has been
                // seen, and `user` is moved into the read task just below.
                let is_authenticated_users = user.is_some();
                if let Some(u) = &user {
                    tracing::debug!(user = %u.sub, %socket_id, "authenticated socket joined");
                }

                // Actual websocket statemachine (one will be spawned per connection)
                let (mut sender, mut receiver) = socket.split();

                // create an mpsc so we can send messages to the socket from multiple threads
                let (agg_sender, mut agg_receiver) = mpsc::channel::<Message>(100);

                // spawn a task that forwards messages from the mpsc to the sender
                // This is a way to share the sender between 2 different threads.
                let mut write_task = tokio::spawn(async move {
                    while let Some(message) = agg_receiver.recv().await {
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                });

                // Set by the read task on every frame the client sends (pongs
                // included), cleared by the keepalive task on every tick. Two
                // consecutive silent periods mean the peer is gone.
                let alive = Arc::new(AtomicBool::new(true));

                // Spawn the socket to handle commands received from the socket user.
                let mut send_messages = {
                    let send_task_sender = agg_sender.clone();
                    let current_pool_name = current_pool_name.clone();
                    let draft_service = draft_service.clone();
                    let socket_id = socket_id.clone();
                    let alive = alive.clone();
                    tokio::spawn(async move {
                        while let Some(Ok(msg)) = receiver.next().await {
                            // Any frame at all proves the peer is still there.
                            alive.store(true, Ordering::Relaxed);

                            // Handle the message received.
                            if let Message::Text(command) = msg {
                                tracing::debug!(%command, "draft socket command received");
                                if let Ok(command) = serde_json::from_str::<Command>(&command) {
                                    // Dropped at the end of this block, so the early `return` on
                                    // LeaveRoom is still recorded.
                                    let mut timer = CommandTimer::start(command_name(&command));
                                    match command {
                                        Command::Auth { .. } => {
                                            tracing::debug!(
                                                %socket_id,
                                                "auth frame after join, ignored"
                                            );
                                        }
                                        Command::LeaveRoom => {
                                            // The socket needs to be killed when the user leave a room.
                                            // The leave room commands will be called once the socket is killed.
                                            return;
                                        }
                                        Command::OnPoolSettingChanges { pool_settings } => {
                                            if let Some(user) = &user {
                                                // If the pool settings update was a success.
                                                if let Err(e) = draft_service
                                                    .update_pool_settings(
                                                        &user.sub,
                                                        &current_pool_name,
                                                        &pool_settings,
                                                    )
                                                    .await
                                                {
                                                    timer.failed();
                                                    let _ = send_task_sender
                                                        .send(Message::Text(e.to_string()))
                                                        .await;
                                                }
                                            }
                                        }
                                        Command::OnReady => {
                                            if let Err(e) = draft_service
                                                .on_ready(&current_pool_name, &socket_id)
                                                .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::AddUser { user_name } => {
                                            if let Err(e) = draft_service
                                                .add_user(
                                                    &current_pool_name,
                                                    &user_name,
                                                    &socket_id,
                                                )
                                                .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::RemoveUser { user_id } => {
                                            if let Err(e) = draft_service
                                                .remove_user(
                                                    &current_pool_name,
                                                    &user_id,
                                                    &socket_id,
                                                )
                                                .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::StartDraft { draft_order } => {
                                            if let Some(user) = &user
                                                && let Err(e) = draft_service
                                                    .start_draft(
                                                        &current_pool_name,
                                                        &user.sub,
                                                        &draft_order,
                                                    )
                                                    .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::DraftPlayer { player_id } => {
                                            if let Some(user) = &user
                                                && let Err(e) = draft_service
                                                    .draft_player(
                                                        &current_pool_name,
                                                        &user.sub,
                                                        player_id,
                                                    )
                                                    .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::UndoDraftPlayer => {
                                            if let Some(user) = &user
                                                && let Err(e) = draft_service
                                                    .undo_draft_player(
                                                        &current_pool_name,
                                                        &user.sub,
                                                    )
                                                    .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::ModifyRoster(modification) => {
                                            if let Some(user) = &user
                                                && let Err(e) = draft_service
                                                    .modify_roster(
                                                        &current_pool_name,
                                                        &user.sub,
                                                        &modification,
                                                    )
                                                    .await
                                            {
                                                timer.failed();
                                                let _ = send_task_sender
                                                    .send(Message::Text(e.to_string()))
                                                    .await;
                                            }
                                        }
                                        Command::JoinRoom {
                                            pool_name: _,
                                            number_poolers: _,
                                        } => {}
                                    }
                                } else {
                                    let _ = send_task_sender
                                        .send(Message::Text(
                                            "could not deserialize the command received."
                                                .to_string(),
                                        ))
                                        .await;
                                }
                            }
                        }
                    })
                };

                // Spawn the socket to handle sending messages to the socket user.
                // When a socket in the room send a messages that needs to be communicated to every one in the room.
                let mut recv_messages = {
                    let recv_sender = agg_sender.clone();
                    let socket_id = socket_id.clone();
                    tokio::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(msg) => {
                                    // The socket's outbound queue is bounded at
                                    // 100 and this send blocks rather than
                                    // dropping, so a client that cannot drain as
                                    // fast as the room produces stalls *here*,
                                    // silently. Timing the send is what makes
                                    // that visible before it turns into a lag.
                                    let send_started = Instant::now();
                                    let send_result = recv_sender.send(Message::Text(msg)).await;
                                    metrics::histogram!(app_metrics::WS_SEND_BLOCKED_SECONDS)
                                        .record(send_started.elapsed().as_secs_f64());
                                    if send_result.is_err() {
                                        break;
                                    }
                                }
                                // This socket fell behind the room's fan-out
                                // buffer. Dropping it from the room over that
                                // would be worse than the gap itself: the draft
                                // deltas carry a pick_count, so the next one to
                                // arrive tells the client it missed updates and
                                // it refetches the pool on its own.
                                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                    metrics::counter!(app_metrics::WS_LAGGED_TOTAL).increment(1);
                                    metrics::counter!(app_metrics::WS_LAGGED_MESSAGES_TOTAL)
                                        .increment(skipped);
                                    tracing::warn!(
                                        %socket_id,
                                        skipped,
                                        "draft socket lagged behind the room broadcast"
                                    );
                                }
                                Err(broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    })
                };

                // Ping the client on a fixed period and close the socket when a
                // whole period goes by without a single frame coming back.
                let mut keepalive = {
                    let ping_sender = agg_sender.clone();
                    let socket_id = socket_id.clone();
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(PING_PERIOD);
                        // The first tick of an interval completes immediately.
                        interval.tick().await;
                        loop {
                            interval.tick().await;
                            if !alive.swap(false, Ordering::Relaxed) {
                                tracing::debug!(
                                    %socket_id,
                                    "draft socket closed: silent for a whole keepalive period"
                                );
                                return;
                            }
                            if ping_sender.send(Message::Ping(Vec::new())).await.is_err() {
                                return;
                            }
                        }
                    })
                };

                // Tome make sure that if the receiver/sender thread complete, the other one get cleared.
                tokio::select! {
                    // The read task ended: the client closed, or sent LeaveRoom.
                    _ = (&mut send_messages) => {
                        connection.closed_because("client");
                        recv_messages.abort();
                        keepalive.abort();
                    }
                    // The room's broadcast channel closed under us.
                    _ = (&mut recv_messages) => {
                        connection.closed_because("broadcast_closed");
                        send_messages.abort();
                        keepalive.abort();
                    }
                    // A whole keepalive period with no frame at all — the peer
                    // is gone without having closed. This is the half-open case
                    // that used to leave members stuck in the room.
                    _ = (&mut keepalive) => {
                        connection.closed_because("keepalive_timeout");
                        send_messages.abort();
                        recv_messages.abort();
                    }
                };

                // Close deliberately, so the client can tell the end of a
                // session from a connection that just dropped. The writer stops
                // on its own once every sender is gone; give the frame a bounded
                // moment to get out, then tear the writer down regardless.
                let _ = agg_sender.send(Message::Close(None)).await;
                drop(agg_sender);
                let _ = tokio::time::timeout(CLOSE_FLUSH_TIMEOUT, &mut write_task).await;
                write_task.abort();

                // Make sure that if we lose the socket communication we force the user to leave the room and unauthenticate.
                // leave_room is called for every socket (authenticated or not) so the
                // instance can release its local room bookkeeping.
                let _ = draft_service
                    .leave_room(&current_pool_name, &socket_id)
                    .await;
                if is_authenticated_users {
                    let _ = draft_service.unauthenticate_web_socket(&socket_id).await;
                }
            }
        }
    }
}
