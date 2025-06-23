use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::helpers::{ConfirmationLinks, TestApp, assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn you_must_be_logged_in_to_publish_a_newsletter() {
    // Arrange
    let app = spawn_app().await;

    // Act - No Login
    let response = app.post_newsletters("").await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_publish_newsletter_form() {
    // Arrange
    let app = spawn_app().await;

    // Act - No Login
    let response = app.get_newsletters().await;

    // Assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn newsletters_returns_shows_error_on_invalid_form_data() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        "html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
        "title=Hello!&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
        "title=Hello!&html_content=<p>Hello!</p>&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
        "title=Hello!&text_content=Hello!&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
        "title=Hello!&html_content=<p>Hello!</p>&text_content=Hello!",
    ];

    // Act - Login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    // Assert
    for invalid_body in test_cases {
        let response = app.post_newsletters(invalid_body).await;
        assert_is_redirect_to(&response, "/admin/newsletters");

        let html_page = app.get_newsletters_html().await;
        assert!(html_page.contains(
            "<p><i>The form fields are incorrect, incomplete or badly formatted.</i></p>"
        ));
    }
}

#[tokio::test]
async fn newsletters_returns_400_on_invalid_idempotency_key() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            "title=Hello!&html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key=",
            "Empty",
        ),
        (
            "title=Hello!&html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key=\
            aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "Too long",
        ),
    ];

    // Act - Login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    // Assert
    for (invalid_body, case) in test_cases {
        let response = app.post_newsletters(invalid_body).await;
        assert_eq!(400, response.status().as_u16(), "{case}");
    }
}

#[tokio::test]
async fn publish_newsletter_form_shows_error_on_empty_parameters() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            "title=&html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
            "The title cannot be empty.",
        ),
        (
            "title=Hello!&html_content=&text_content=&idempotency_key=1816bc9c-4bdd-43b3-be5b-5f72f4d00869",
            "The content cannot be empty.",
        ),
    ];

    // Act - Login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body).await;
        assert_is_redirect_to(&response, "/admin/newsletters");

        let html_page = app.get_newsletters_html().await;
        assert!(html_page.contains(&format!("<p><i>{error_message}</i></p>")));
    }
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        // We assert that no request is fired at Postmark!
        .expect(0)
        .mount(&app.email_server)
        .await;

    // Act - Login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    // Act
    let response = app
        .post_newsletters(&format!(
            "title=Hello!&html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key={}",
            uuid::Uuid::new_v4().to_string()
        ))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletters_html().await;
    assert!(html_page.contains(
        "<p><i>The newsletter has no confirmed subscribers \
        or their stored contact details are invalid.</i></p>"
    ));
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password
    }))
    .await;

    // Act
    let response = app
        .post_newsletters(&format!(
            "title=Hello!&html_content=<p>Hello!</p>&text_content=Hello!&idempotency_key={}",
            uuid::Uuid::new_v4().to_string()
        ))
        .await;

    // Assert
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletters_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

/// Use the public API of the application under test to create
/// an unconfirmed subscriber.
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();
    // We now inspect the requests received by the mock Postmark server
    // to retrieve the confirmation link and return it
    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    // We can then reuse the same helper and just add
    // an extra step to actually call the confirmation link!
    let confirmation_link = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    // Arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act - Part 1 - Submit newsletter form
    let newsletter_request_body = format!(
        "title=Newsletter title\
        &text_content=Newsletter body as plain text\
        &html_content=<p>Newsletter body as HTML</p>\
        &idempotency_key={}",
        uuid::Uuid::new_v4().to_string()
    );
    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 2 - Follow the redirect
    let html_page = app.get_newsletters_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));

    // Act - Part 3 - Submit newsletter form **again**
    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Act - Part 4 - Follow the redirect
    let html_page = app.get_newsletters_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));

    // Mock verifies on Drop that we have sent the newsletter email **once**
}
