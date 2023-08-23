use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Json, Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{headers, Extension, Router, TypedHeader};
use futures::StreamExt;
use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft;
use poolnhl_interface::draft::model::{
    SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest, UserToken,
};
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::Result;
use poolnhl_interface::pool::model::{Player, Pool, PoolSettings};
use serde::Deserialize;
use std::net::SocketAddr;

use poolnhl_infrastructure::jwt::decode;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

pub struct DraftRouter;

impl DraftRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/start-draft", post(Self::start_draft))
            .route("/draft-player", post(Self::draft_player))
            .route("/undo-draft-player", post(Self::undo_draft_player))
            .route("/ws/:token", get(Self::ws_handler))
            .route("/rooms", get(Self::get_rooms))
            .with_state(service_registry)
    }

    // Initiate a draft.
    async fn start_draft(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(mut body): Json<StartDraftRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .start_draft(&token._id, &mut body)
            .await
            .map(Json)
    }

    // Draft a player.
    async fn draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<SelectPlayerRequest>,
    ) -> Result<Json<Pool>> {
        draft_service.draft_player(&token._id, body).await.map(Json)
    }

    // Undo the last draft player action.
    async fn undo_draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<UndoSelectionRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .undo_draft_player(&token._id, body)
            .await
            .map(Json)
    }

    async fn get_rooms(
        State(draft_service): State<DraftServiceHandle>,
    ) -> Result<Json<Vec<String>>> {
        draft_service.list_rooms().await.map(Json)
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        State(draft_service): State<DraftServiceHandle>,
        Path(token): Path<String>,
    ) -> impl IntoResponse {
        draft_service.authentificate_web_socket(&token, addr);
        ws.on_upgrade(move |socket| Self::handle_socket(socket, addr, draft_service))
    }

    async fn handle_socket(socket: WebSocket, addr: SocketAddr, draft_service: DraftServiceHandle) {
        // Actual websocket statemachine (one will be spawned per connection)
        let (mut sender, mut receiver) = socket.split();
        let mut current_pool_name = None::<String>;

        while let Some(Ok(msg)) = receiver.next().await {
            // Handle the message received.
            if let Message::Text(command) = msg {
                println!("{}", command);
                if let Ok(command) = serde_json::from_str::<Command>(&command) {
                    println!("type");
                    match command {
                        Command::JoinRoom { pool_name } => {
                            if let Some(past_pool_name) = current_pool_name {
                                // Leave the past room before joining the new room.
                                draft_service.leave_room(&past_pool_name, addr);
                            }

                            // join the requested room.
                            draft_service.join_room(&pool_name, addr);
                            current_pool_name = Some(pool_name);
                        }

                        Command::LeaveRoom => {
                            if let Some(pool_name) = &current_pool_name {
                                // Can only leave a room a the user is in a current room.
                                draft_service.leave_room(pool_name, addr);
                            }
                        }
                        _ => todo!("Need to implement the rest of the Socket Commands"),
                    }
                }
            }
        }
    }
}

#[derive(Deserialize)]
enum Command {
    JoinRoom { pool_name: String },
    LeaveRoom,
    OnReady,
    OnPoolSettingChanges { pool_settings: PoolSettings },
    UndoDraftPlayer,
    DraftPlayer { player: Player },
}
