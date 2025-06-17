use lazy_static::lazy_static;
use tera::Tera;

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("templates/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                std::process::exit(1);
            }
        }
    };
}

pub struct Templates;

impl Templates {
    pub fn render_welcome(
        subscriber_name: &str,
        confirmation_link: &str,
    ) -> Result<String, anyhow::Error> {
        let mut context = tera::Context::new();
        context.insert("subscriber_name", subscriber_name);
        context.insert("confirmation_link", confirmation_link);
        TEMPLATES
            .render("welcome.html", &context)
            .map_err(|e| anyhow::anyhow!("Could not render welcome template: {e}"))
    }

    pub fn render_publish_newsletter(flash_messages: &str) -> Result<String, anyhow::Error> {
        let mut context = tera::Context::new();
        context.insert("flash_messages", &flash_messages);
        TEMPLATES
            .render("publish_newsletter.html", &context)
            .map_err(|e| anyhow::anyhow!("Could not render send newsletter template: {e}"))
    }
}
