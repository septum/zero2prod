use actix_web::{HttpResponse, error::UrlencodedError, web};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;
use sqlx::{Executor, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    authentication::UserId,
    idempotency::{IdempotencyKey, NextAction, save_response, try_processing},
    utils::{e400, e500, see_other},
};

#[derive(serde::Deserialize)]
pub struct NewsletterData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip_all,
    fields(user_id=%&*user_id)
)]
pub async fn publish_newsletter(
    form: Result<web::Form<NewsletterData>, actix_web::Error>,
    pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let NewsletterData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = match form {
        Ok(form) => form.0,
        Err(error) => {
            let message = match error.as_error::<UrlencodedError>() {
                Some(_) => "The form fields are incorrect, incomplete or badly formatted.",
                _ => "Something unexpected happened, please report it to the web admin.",
            };
            FlashMessage::error(message).send();
            return Ok(see_other("/admin/newsletters"));
        }
    };

    if title.is_empty() {
        FlashMessage::error("The title cannot be empty.").send();
        return Ok(see_other("/admin/newsletters"));
    }

    if html_content.is_empty() || text_content.trim().is_empty() {
        FlashMessage::error("The content cannot be empty.").send();
        return Ok(see_other("/admin/newsletters"));
    }

    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let mut transaction = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };

    let at_least_one_confirmed_subscriber =
        check_confirmed_subscribers(&pool).await.map_err(e500)?;
    if !at_least_one_confirmed_subscriber {
        FlashMessage::warning("The newsletter has no confirmed subscribers or their stored contact details are invalid.").send();
        return Ok(see_other("/admin/newsletters"));
    }

    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    success_message().send();
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info(
        "The newsletter issue has been accepted - \
        emails will go out shortly.",
    )
}

#[tracing::instrument(name = "Check confirmed subscribers", skip(pool))]
async fn check_confirmed_subscribers(pool: &PgPool) -> Result<bool, anyhow::Error> {
    let confirmed_subscribers_check = sqlx::query!(
        r#"
        SELECT count(id) as "count!"
        FROM subscriptions
        WHERE status = 'confirmed'
        LIMIT 1;
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(confirmed_subscribers_check.count > 0)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    );
    transaction.execute(query).await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    );
    transaction.execute(query).await?;
    Ok(())
}
