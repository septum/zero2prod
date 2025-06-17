use actix_web::{HttpResponse, web};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

use crate::{
    authentication::UserId,
    domain::SubscriberEmail,
    email_client::EmailClient,
    utils::{e500, see_other},
};

#[derive(serde::Deserialize)]
pub struct NewsletterData {
    title: String,
    html_content: String,
    text_content: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<NewsletterData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    if form.title.is_empty() {
        FlashMessage::error("The title cannot be empty.").send();
        return Ok(see_other("/admin/newsletters"));
    }

    if form.html_content.is_empty() || form.text_content.trim().is_empty() {
        FlashMessage::error("The content cannot be empty.").send();
        return Ok(see_other("/admin/newsletters"));
    }

    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    let mut at_least_one_confirmed_subscriber = false;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                at_least_one_confirmed_subscriber = true;
                email_client
                    .send_email(
                        &subscriber.email,
                        &form.title,
                        &form.html_content,
                        &form.text_content,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                error.cause_chain = ?error,
                error.message = %error,
                "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }

    if at_least_one_confirmed_subscriber {
        FlashMessage::info("The newsletter issue has been published!").send();
    } else {
        FlashMessage::warning("The newsletter has no confirmed subscribers or their stored contact details are invalid.").send();
    }
    Ok(see_other("/admin/newsletters"))
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();

    Ok(confirmed_subscribers)
}
