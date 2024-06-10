use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmailInfo {
    // The current primary email address of the user.
    pub address: String,

    // A boolean field indicating whether the email address is the primary email.
    // Currently, this field is redundant because only the primary email is included in the JWT.
    pub is_primary: bool,

    // A boolean field indicating whether the email address has been verified.
    pub is_verified: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserEmailJwtPayload {
    // The audience for which the JWT was created.
    // It specifies the intended recipient or system that should accept this JWT.
    // When using Hanko Cloud, the aud will be your app URL.
    pub aud: Vec<String>,

    // Object containing information related to the user email information.
    pub email: EmailInfo,

    // The timestamp indicating when the JWT will expire.
    pub exp: String,

    // The timestamp indicating when the JWT was created.
    pub iat: String,

    // The user ID.
    pub sub: String,
}
