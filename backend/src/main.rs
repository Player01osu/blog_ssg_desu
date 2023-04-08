use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    net::SocketAddr,
};

use axum::{response::IntoResponse, routing::get, Json, Router};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use walkdir::WalkDir;

const CACHE_PATH: &str = "./routes/routes.json";

lazy_static::lazy_static! {
    static ref HTML_TITLE: Regex = Regex::new(r".*<title>(.*)</title>$").unwrap();
}

#[derive(Serialize, Debug, Default, Deserialize)]
pub struct RouteCache {
    #[serde(rename = "length")]
    len: usize,
    routes: Vec<RouteNames>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RouteNames {
    name: String,
    route: String,
}

pub fn page_title(file: &File) -> Option<String> {
    BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .find_map(|l| {
            HTML_TITLE
                .captures(&l)
                .and_then(|c| c.get(1).and_then(|m| Some(m.as_str().to_owned())))
        })
}

pub fn route_cache_len() -> usize {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .append(false)
        .open(CACHE_PATH)
        .unwrap();

    serde_json::from_reader::<_, RouteCache>(file)
        .unwrap_or_default()
        .len
}

pub fn route_len() -> usize {
    WalkDir::new("routes")
        .into_iter()
        .filter_map(|d| d.ok())
        .filter(|d| d.file_name().eq("index.html"))
        .count()
}

pub fn add_cache() -> anyhow::Result<()> {
    let file = OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .append(false)
        .open(CACHE_PATH)
        .unwrap();

    let routes = WalkDir::new("routes")
        .into_iter()
        .filter_map(|d| d.ok())
        .filter_map(|d| {
            let d = d.path().file_name()?.eq("index.html").then_some(d.path())?;
            let name = page_title(&File::open(d).ok()?)?;
            let route = d
                .parent()?
                .iter()
                .skip(1)
                .filter_map(|c| Some(c.to_str()?.to_owned() + "/"))
                .collect::<String>();

            Some(RouteNames { name, route })
        })
        .collect::<Vec<RouteNames>>();

    let route_cache = RouteCache {
        len: route_len(),
        routes,
    };

    serde_json::to_writer(file, &route_cache)?;
    Ok(())
}

pub async fn show_paths() -> impl IntoResponse {
    // TODO Don't want to keep file open, but using same reference doesn't exactly work as intended
    // because reading from it changes its position.
    (route_len() != route_cache_len()).then(|| add_cache().unwrap());

    let file = File::open(CACHE_PATH).unwrap();
    let routes = serde_json::from_reader::<_, RouteCache>(file).unwrap();
    Json(routes)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    dbg!(route_len());
    let app = Router::new()
        .nest_service(
            "/",
            ServeDir::new("routes").fallback(ServeFile::new("routes/not_found.html")),
        )
        .route_service("/api/v0/routes", get(show_paths));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.layer(TraceLayer::new_for_http()).into_make_service())
        .await
        .unwrap();
}
