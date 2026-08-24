//! Metric names, shared so there is exactly one spelling of each.
//!
//! They live here rather than next to the recorder in `poolnhl_routing`
//! because the draft room state in `poolnhl_infrastructure` also emits, and
//! infrastructure cannot depend on routing. A metric renamed on one side of
//! that boundary and not the other is a silently empty panel, so both sides
//! reference these constants.
//!
//! No dependency on the `metrics` crate here on purpose — these are just names.

// Draft socket lifecycle.
pub const WS_CONNECTIONS: &str = "draft_ws_connections";
pub const WS_CONNECTED_TOTAL: &str = "draft_ws_connected_total";
pub const WS_CLOSED_TOTAL: &str = "draft_ws_closed_total";
pub const ROOMS_ACTIVE: &str = "draft_rooms_active";

// Backpressure: both mean this instance cannot keep up with the fan-out.
pub const WS_LAGGED_TOTAL: &str = "draft_ws_lagged_total";
pub const WS_LAGGED_MESSAGES_TOTAL: &str = "draft_ws_lagged_messages_total";
pub const WS_SEND_BLOCKED_SECONDS: &str = "draft_ws_send_blocked_seconds";

// Command handling.
pub const COMMAND_TOTAL: &str = "draft_command_total";
pub const COMMAND_DURATION: &str = "draft_command_duration_seconds";

// REST side.
pub const HTTP_REQUESTS_TOTAL: &str = "http_requests_total";
pub const HTTP_REQUEST_DURATION: &str = "http_request_duration_seconds";
