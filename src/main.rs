use actix_web::{Responder, get, post, HttpResponse, HttpServer, App, web, Result};
use serde::Deserialize;
use chromiumoxide::{Browser, BrowserConfig};
use futures::{StreamExt, TryFutureExt};
use futures::future::select;
use futures::future::{Abortable, AbortHandle, Aborted};
use std::sync::Arc;
use chromiumoxide_cdp::cdp::browser_protocol::emulation::{SetEmulatedMediaParams, SetDeviceMetricsOverrideParams};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use std::path::Path;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello, world!")
}

#[derive(Deserialize)]
struct RenderOptions {
    to: String,
    input: String,
}

#[derive(Deserialize)]
struct VPSize {
    width: f32,
    height: f32,
}

struct AppState {
    browser: Arc<Browser>,
}

async fn get_html(input: &str, data: &web::Data<AppState>) -> Result<String, Box<dyn std::error::Error>> {
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

    page.save_pdf(PrintToPdfParams::builder()
        .paper_width(size.width / 96.0)
        .paper_height(size.height / 96.0)
        .page_ranges("1")
        .margin_top(0)
        .margin_left(0)
        .margin_bottom(0)
        .margin_right(0)
        .prefer_css_page_size(false)
        .build(),
                  Path::new("./test.pdf")).await?;

    println!("Done!");

    let html = page.wait_for_navigation().await?.content().await?;

    println!("Got HTML");

    page.close().await?;
    println!("Loop done");

    Ok(html)
}

#[post("/render")]
async fn convert_file(opts: web::Json<RenderOptions>, data: web::Data<AppState>) -> Result<String> {
    let result = get_html(&opts.input, &data).await;

    match result {
        Ok(html) => Ok(html),
        Err(err) => Ok(err.to_string())
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cfg = BrowserConfig::builder().build().unwrap();
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
                web::JsonConfig::default()
                    .limit(1024 * 1024)
            )
            .data(AppState {
                browser: browser_arc.clone()
            })
            .service(hello)
            .service(convert_file)
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
