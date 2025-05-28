use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::SubscriptionToken;

use crate::routes::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> StatusCode {
        match self {
            ConfirmError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

impl TryFrom<String> for SubscriptionToken {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SubscriptionToken::parse(value)
    }
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmError> {
    let incoming_token: SubscriptionToken = parameters
        .0
        .subscription_token
        .try_into()
        .map_err(ConfirmError::ValidationError)?;
    let id = get_subscriber_id_from_token(&pool, incoming_token.as_ref())
        .await
        .context("Failed to get a subcriber id from the provided token")?;

    Ok(match id {
        None => HttpResponse::Unauthorized().finish(), // Non-existing token!
        Some(subscriber_id) => {
            if check_subscriber_is_confirmed(&pool, subscriber_id)
                .await
                .context("Failed to check if subcriber is already confirmed")?
            {
                confirm_subscriber(&pool, subscriber_id)
                    .await
                    .context("Failed to confirm subscriber in the database")?;
            }

            HttpResponse::Ok().finish()
        }
    })
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens \
        WHERE subscription_token = $1",
        subscription_token,
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(
    name = "Checking if subscriber has a confirmed status in the database",
    skip(pool, subscriber_id)
)]
pub async fn check_subscriber_is_confirmed(
    pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        SELECT status FROM subscriptions
        WHERE id = $1
        "#,
        subscriber_id
    )
    .fetch_one(pool)
    .await?;

    Ok(record.status == "confirmed")
}
