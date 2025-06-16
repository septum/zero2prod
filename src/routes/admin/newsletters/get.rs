use actix_web::HttpResponse;
use actix_web::http::header::ContentType;

pub async fn send_newsletter_issue() -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("form.html"))
}
