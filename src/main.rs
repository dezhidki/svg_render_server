use actix_multipart::Multipart;
use actix_web::{get, post, HttpResponse, HttpServer, App, web, Result};
use serde::Deserialize;
use chromiumoxide::{Browser, BrowserConfig};
use futures::{StreamExt, TryStreamExt};
use std::sync::Arc;
use chromiumoxide_cdp::cdp::browser_protocol::emulation::{SetEmulatedMediaParams};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;

#[derive(Deserialize)]
struct VPSize {
    width: f32,
    height: f32,
}

struct AppState {
    browser: Arc<Browser>,
}

async fn render_pdf(input: &str, data: &web::Data<AppState>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let browser = &data.browser;
    let page = browser.new_page("").await?;

    page.execute(SetEmulatedMediaParams::builder().media(String::from("screen")).build()).await?;
    page.set_content(format!(r#"
        <!DOCTYPE html>
        <html>
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
            </head>
            <body style="margin: 0;">
                <div>{}</div>
            </body>
        </html>
    "#, input)).await?;

    let size: VPSize = page.evaluate_function(r#"() => {
            let size = document.getElementsByTagName("svg")[0];
            return {
                width: size.clientWidth,
                height: size.clientHeight,
            };
    }"#).await?.into_value()?;

    println!("Printing with size {}, {}", size.width, size.height);

    let pdf_data = page.pdf(PrintToPdfParams::builder()
        .paper_width(size.width / 96.0)
        .paper_height(size.height / 96.0)
        .page_ranges("1")
        .margin_top(0)
        .margin_left(0)
        .margin_bottom(0)
        .margin_right(0)
        .prefer_css_page_size(false)
        .build()).await?;

    Ok(pdf_data)
}

#[post("/render")]
async fn convert_file(mut payload: Multipart, data: web::Data<AppState>) -> HttpResponse {
    let field = match payload.try_next().await {
        Ok(Some(f)) => f,
        _ => return HttpResponse::BadRequest().body("Invalid file")
    };

    let chunks = field.map(|x| x.unwrap()).collect::<Vec<_>>().await;
    let mut input = String::new();
    for r in chunks {
        input.push_str(std::str::from_utf8(r.as_ref()).unwrap());
    }

    let result = render_pdf(&input, &data).await;

    match result {
        Ok(data) => HttpResponse::Ok().content_type("application/pdf").body(data),
        Err(err) => HttpResponse::InternalServerError().body(err.to_string())
    }
}

#[get("/test")]
async fn test_page() -> HttpResponse {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form action="/render" method="post" enctype="multipart/form-data">
                <input type="file" name="file"/>
                <button type="submit">Submit</button>
            </form>
            <p>Max size: 2MB</p>
        </body>
    </html>
    "#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cfg = BrowserConfig::builder()
        .arg("--disable-gpu")
        .arg("--no-sandbox") // TODO: Run with seccop instead
        .arg("--disable-setuid-sandbox")
        .build()
        .unwrap();
    println!("Starting Chrome");
    let (browser, mut handler) = Browser::launch(cfg).await.unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                let _ = handler.next().await.unwrap();
            }
        });
    });

    let ver = browser.version().await.unwrap();
    println!("Done, got version {}", ver.product);
    let browser_arc = Arc::new(browser);

    HttpServer::new(move || {
        App::new()
            .app_data(
                web::FormConfig::default()
                    .limit(2 * 1024 * 1024)
            )
            .app_data(web::Data::new(AppState {
                browser: browser_arc.clone()
            }))
            .service(convert_file)
            .service(test_page)
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
