use std::{net::SocketAddr, fs::{File, OpenOptions}, io::{BufRead, BufReader, Read}};

use axum::{response::IntoResponse, routing::get, Json, Router};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use walkdir::WalkDir;

#[derive(Deserialize, Serialize, Debug)]
pub struct RouteNames {
    name: String,
    route: String,
}


const CACHE_PATH: &str = "./routes/routes.json";
lazy_static::lazy_static! {
    static ref HTML_TITLE: Regex = Regex::new(r".*<title>(.*)</title>$").unwrap();
}

pub fn get_page_title(file: &File) -> Option<String> {
    let lines = BufReader::new(file).lines().filter_map(|l| l.ok());
    for line in lines {
        match HTML_TITLE.captures(&line) {
            Some(c) => return Some(c[1].to_owned()),
            None => continue,
        }
    }
    None
}

#[derive(Serialize, Debug, Default, Deserialize)]
pub struct RouteCache {
    #[serde(rename = "length")]
    len: usize,
    routes: Vec<RouteNames>,
}

pub fn route_cache_len() -> usize {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .append(false)
        .open(CACHE_PATH)
        .unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    serde_json::from_str::<RouteCache>(&buf)
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
            let d = d.path();
            if d.file_name()?.to_str()? != "index.html" {
                return None;
            }
            let f = File::open(d).ok()?;

            let route = d
                .parent()?
                .iter()
                .skip(1)
                .filter_map(|c| Some(c.to_str()?.to_owned() + "/"))
                .collect::<String>();
            let name = get_page_title(&f)?;

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
