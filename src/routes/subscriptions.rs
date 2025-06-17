use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError, web};
use anyhow::Context;
use chrono::Utc;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName, SubscriptionToken};
use crate::email_client::EmailClient;
use crate::html_templates::Templates;
use crate::startup::ApplicationBaseUrl;

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    // Transparent delegates both `Display`'s and `source`'s implementation
    // to the type wrapped by `UnexpectedError`.
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database failure was encountered while trying to store a subscription token."
        )
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "\n{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;

        Ok(Self { email, name })
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    );
    transaction.execute(query).await?;

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Fetching the subscription token from the database",
    skip(pool, subscriber_id)
)]
pub async fn fetch_subscription_token(
    pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        SELECT subscription_token FROM subscription_tokens
        WHERE subscriber_id = $1
        "#,
        subscriber_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| r.subscription_token))
}

#[tracing::instrument(
    name = "Checking if subscriber exists in the database",
    skip(pool, email)
)]
pub async fn check_subscriber_exists(
    pool: &PgPool,
    email: &SubscriberEmail,
) -> Result<Option<(Uuid, String)>, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        SELECT id, status FROM subscriptions
        WHERE email = $1
        "#,
        email.as_ref()
    )
    .fetch_optional(pool)
    .await?;

    Ok(record.map(|r| (r.id, r.status)))
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
    subscriber_email = %form.email,
    subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber: NewSubscriber =
        form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let subscriber_details = check_subscriber_exists(&pool, &new_subscriber.email)
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    if let Some((subscriber_id, subscriber_status)) = subscriber_details {
        if subscriber_status == "pending_confirmation" {
            // We are not using `SubscriptionToken` because we trust the data source
            let subscription_token = match fetch_subscription_token(&pool, subscriber_id)
                .await
                .context("Failed to fetch a subscription token from the database")?
            {
                Some(token) => token,
                None => return Ok(HttpResponse::InternalServerError().finish()),
            };
            send_confirmation_email(
                &email_client,
                new_subscriber,
                &base_url.0,
                &subscription_token,
            )
            .await
            .context("Failed to send a confirmation email")?;
        }
    } else {
        let mut transaction = pool
            .begin()
            .await
            .context("Failed to acquire a Postgres connection from the pool")?;
        let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
            .await
            .context("Failed to insert new subcriber in the database")?;
        let subscription_token = SubscriptionToken::new();
        store_token(&mut transaction, subscriber_id, subscription_token.as_ref())
            .await
            .context("Failed to store the confirmation token for a new subscriber")?;
        transaction
            .commit()
            .await
            .context("Failed to commit SQL transaction to store a new subscriber")?;
        send_confirmation_email(
            &email_client,
            new_subscriber,
            &base_url.0,
            subscription_token.as_ref(),
        )
        .await
        .context("Failed to send a confirmation email")?;
    }

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), anyhow::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = Templates::render_welcome(new_subscriber.name.as_ref(), &confirmation_link)?;

    email_client
        .send_email(&new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );
    transaction.execute(query).await.map_err(StoreTokenError)?;

    Ok(())
}
