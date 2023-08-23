use axum::extract::{Json, State};
use axum::routing::post;
use axum::Router;

use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft::model::{
    SelectPlayerRequest, StartDraftRequest, UndoSelectionRequest,
};
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::Result;
<<<<<<< Updated upstream
use poolnhl_interface::pool::model::Pool;
=======
use poolnhl_interface::pool::model::{Player, Pool, PoolSettings};
use poolnhl_interface::{draft, pool};
use serde::Deserialize;
use std::net::SocketAddr;
>>>>>>> Stashed changes

use poolnhl_infrastructure::jwt::UserToken;

use tokio::sync::broadcast;
pub struct DraftRouter;

impl DraftRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/start-draft", post(DraftRouter::start_draft))
            .route("/draft-player", post(DraftRouter::draft_player))
            .route("/undo-draft-player", post(DraftRouter::undo_draft_player))
            .with_state(service_registry)
    }

    // Initiate a draft.
    async fn start_draft(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(mut body): Json<StartDraftRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .start_draft(&token._id.to_string(), &mut body)
            .await
            .map(Json)
    }

    // Draft a player.
    async fn draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<SelectPlayerRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .draft_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }

    // Undo the last draft player action.
    async fn undo_draft_player(
        token: UserToken,
        State(draft_service): State<DraftServiceHandle>,
        Json(body): Json<UndoSelectionRequest>,
    ) -> Result<Json<Pool>> {
        draft_service
            .undo_draft_player(&token._id.to_string(), body)
            .await
            .map(Json)
    }
<<<<<<< Updated upstream
=======

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
        let mut tx = None::<broadcast::Sender<String>>;
        let mut pool_name = None::<String>;

        // Thread used to handle when the socket send commands
        let mut send_messages = {
            tokio::spawn(async move {
                while let Some(Ok(msg)) = receiver.next().await {
                    // Handle the message received.
                    if let Message::Text(command) = msg {
                        println!("{}", command);
                        if let Ok(command) = serde_json::from_str::<Command>(&command) {
                            println!("type");
                            match command {
                                Command::JoinRoom { pool_name } => {
                                    let tx = draft_service.join_room(&pool_name, addr);
                                }
                                Command::LeaveRoom { pool_name } => {
                                    draft_service.leave_room(&pool_name, addr)
                                }
                                _ => todo!("Need to implement the rest of the Socket Commands"),
                            }
                        }
                    }
                }
            })
        };

        // Thread used to handle when the socket receive commands
        let mut recv_messages = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if sender.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        tokio::select! {
            _ = (&mut send_messages) => recv_messages.abort(),
            _ = (&mut recv_messages) => send_messages.abort(),
        };

        match (tx, pool_name) {
            (Some(tx), Some(pool_name)) => {
                // If a tx and a pool_name, this means the user is in the room,
                // let leave the room.
                let _ = tx.send(format!("{} left the chat!", addr));
                draft_service.leave_room(&pool_name, addr);
            }
            _ => {}
        }
    }
}

#[derive(Deserialize)]
enum Command {
    JoinRoom {
        pool_name: String,
    },
    LeaveRoom {
        pool_name: String,
    },

    OnReady {
        pool_name: String,
    },
    OnPoolSettingChanges {
        pool_name: String,
        pool_settings: PoolSettings,
    },
    UndoDraftPlayer {
        pool_name: String,
    },
    DraftPlayer {
        pool_name: String,
        player: Player,
    },
>>>>>>> Stashed changes
}
