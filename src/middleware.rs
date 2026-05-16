use crate::{error::AppError, models::Claims, AppState};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
    RequestPartsExt,
};
use axum_extra::{headers::{authorization::Bearer, Authorization}, TypedHeader};
use jsonwebtoken::{decode, DecodingKey, Validation};
use uuid::Uuid;

/// Extractor: requires a valid JWT. Returns 401 if missing/invalid.
pub struct AuthUser(pub Uuid);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let claims = decode_token(bearer.token(), &state.jwt_secret)?;
        Ok(AuthUser(claims.sub))
    }
}

/// Extractor: optional JWT. Returns None for guests, Some(id) for authed users.
pub struct MaybeUser(pub Option<Uuid>);

#[async_trait]
impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let maybe = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .ok();

        match maybe {
            Some(TypedHeader(Authorization(bearer))) => {
                let claims = decode_token(bearer.token(), &state.jwt_secret)?;
                Ok(MaybeUser(Some(claims.sub)))
            }
            None => Ok(MaybeUser(None)),
        }
    }
}

pub fn decode_token(token: &str, secret: &str) -> crate::error::Result<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

pub fn make_token(user_id: Uuid, username: &str, secret: &str) -> crate::error::Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let exp = (chrono::Utc::now() + chrono::Duration::days(30)).timestamp() as usize;
    let claims = Claims { sub: user_id, username: username.to_string(), exp };
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))?;
    Ok(token)
}
