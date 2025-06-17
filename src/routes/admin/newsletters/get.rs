use std::fmt::Write;

use actix_web::HttpResponse;
use actix_web::http::header::ContentType;
use actix_web_flash_messages::IncomingFlashMessages;

use crate::html_templates::Templates;
use crate::utils::e500;

pub async fn publish_newsletter_form(
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut messages = String::new();
    for m in flash_messages.iter() {
        writeln!(messages, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    let html_body = Templates::render_publish_newsletter(&messages).map_err(e500)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(html_body))
}
