use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Json, Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use poolnhl_infrastructure::services::ServiceRegistry;
use poolnhl_interface::draft::model::Command;
use poolnhl_interface::draft::service::DraftServiceHandle;
use poolnhl_interface::errors::{AppError, Result};
use poolnhl_interface::users::model::UserEmailJwtPayload;

use std::net::SocketAddr;
use tokio::sync::{broadcast, mpsc};

pub struct DraftRouter;

impl DraftRouter {
    pub fn new(service_registry: ServiceRegistry) -> Router {
        Router::new()
            .route("/ws/:token", get(Self::ws_handler))
            .route("/rooms", get(Self::get_rooms))
            .with_state(service_registry)
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
        let user = draft_service.authenticate_web_socket(&token, addr).await;
        ws.on_upgrade(move |socket| Self::handle_socket(socket, user, addr, draft_service))
    }

    // The initial socket state.
    // Waits for the client socket to send the JoinRoom command.
    // before leaving the state. It returns the the receiver and the room name.
    async fn waiting_join_room_command(
        socket: &mut WebSocket,
        addr: &SocketAddr,
        draft_service: &DraftServiceHandle,
    ) -> Result<(broadcast::Receiver<String>, String)> {
        while let Some(Ok(msg)) = socket.recv().await {
            if let Message::Text(command) = msg {
                if let Ok(command) = serde_json::from_str::<Command>(&command) {
                    match command {
                        Command::JoinRoom { pool_name } => {
                            // join the requested room.
                            let rx = draft_service.join_room(&pool_name, *addr).await?;

                            // TODO: Make sure this line can be removed..
                            //let _ = socket.send(Message::Text(users)).await;

                            return Ok((rx, pool_name));
                        }
                        _ => continue,
                    }
                }
            }
        }
        Err(AppError::CustomError {
            msg: "Could not join a room.".to_string(),
        })
    }

    async fn handle_socket(
        mut socket: WebSocket,
        user: Option<UserEmailJwtPayload>,
        addr: SocketAddr,
        draft_service: DraftServiceHandle,
    ) {
        // At the beginning there is a state where the user needs to join a room
        // before leaving the initial socket state.

        match DraftRouter::waiting_join_room_command(&mut socket, &addr, &draft_service).await {
            Err(_) => (), // An error occured during the initial waiting to join room function. Close the socket connection.
            Ok((mut rx, current_pool_name)) => {
                println!("upgraded socket");
                // Actual websocket statemachine (one will be spawned per connection)
                let (mut sender, mut receiver) = socket.split();

                // create an mpsc so we can send messages to the socket from multiple threads
                let (agg_sender, mut agg_receiver) = mpsc::channel::<String>(100);

                // spawn a task that forwards messages from the mpsc to the sender
                // This is a way to share the sender between 2 different threads.
                tokio::spawn(async move {
                    while let Some(message) = agg_receiver.recv().await {
                        if sender.send(message.into()).await.is_err() {
                            break;
                        }
                    }
                });

                // Spawn the socket to handle commands received from the socket user.
                let mut send_messages = {
                    let send_task_sender = agg_sender.clone();
                    let current_pool_name = current_pool_name.clone();
                    let draft_service = draft_service.clone();
                    tokio::spawn(async move {
                        while let Some(Ok(msg)) = receiver.next().await {
                            // Handle the message received.
                            if let Message::Text(command) = msg {
                                println!("{}", command);
                                if let Ok(command) = serde_json::from_str::<Command>(&command) {
                                    match command {
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
                                                    let _ =
                                                        send_task_sender.send(e.to_string()).await;
                                                }
                                            }
                                        }
                                        Command::OnReady => {
                                            if let Err(e) = draft_service
                                                .on_ready(&current_pool_name, addr)
                                                .await
                                            {
                                                let _ = send_task_sender.send(e.to_string()).await;
                                            }
                                        }
                                        Command::StartDraft => {
                                            if let Some(user) = &user {
                                                if let Err(e) = draft_service
                                                    .start_draft(&current_pool_name, &user.sub)
                                                    .await
                                                {
                                                    let _ =
                                                        send_task_sender.send(e.to_string()).await;
                                                }
                                            }
                                        }
                                        Command::DraftPlayer { player } => {
                                            if let Some(user) = &user {
                                                if let Err(e) = draft_service
                                                    .draft_player(
                                                        &current_pool_name,
                                                        &user.sub,
                                                        player,
                                                    )
                                                    .await
                                                {
                                                    let _ =
                                                        send_task_sender.send(e.to_string()).await;
                                                }
                                            }
                                        }
                                        Command::UndoDraftPlayer => {
                                            if let Some(user) = &user {
                                                if let Err(e) = draft_service
                                                    .undo_draft_player(
                                                        &current_pool_name,
                                                        &user.sub,
                                                    )
                                                    .await
                                                {
                                                    let _ =
                                                        send_task_sender.send(e.to_string()).await;
                                                }
                                            }
                                        }
                                        Command::JoinRoom { pool_name: _ } => {}
                                    }
                                } else {
                                    let _ = send_task_sender
                                        .send(
                                            "could not deserialize the command received."
                                                .to_string(),
                                        )
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
                    tokio::spawn(async move {
                        while let Ok(msg) = rx.recv().await {
                            if recv_sender.send(msg).await.is_err() {
                                break;
                            }
                        }
                    })
                };

                // Tome make sure that if the receiver/sender thread complete, the other one get cleared.
                tokio::select! {
                    _ = (&mut send_messages) => recv_messages.abort(),
                    _ = (&mut recv_messages) => send_messages.abort(),
                };

                // Make sure that if we lose the socket communication we force the user to leave the room.
                let _ = draft_service.leave_room(&current_pool_name, addr);
            }
        }
    }
}
