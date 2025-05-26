use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::SubscriptionToken;

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
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    let incoming_token: SubscriptionToken = match parameters.0.subscription_token.try_into() {
        Ok(token) => token,
        Err(err) => {
            tracing::error!(err);
            return HttpResponse::BadRequest().finish();
        }
    };
    let id = match get_subscriber_id_from_token(&pool, incoming_token.as_ref()).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match id {
        // Non-existing token!
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            match check_subscriber_is_confirmed(&pool, subscriber_id).await {
                Ok(false) => {
                    if confirm_subscriber(&pool, subscriber_id).await.is_err() {
                        return HttpResponse::InternalServerError().finish();
                    }
                }
                Err(_) => {
                    return HttpResponse::InternalServerError().finish();
                }
                Ok(true) => {}
            }

            HttpResponse::Ok().finish()
        }
    }
}
#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(record.status == "confirmed")
}
