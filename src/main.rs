#![allow(unused_imports)]

// Hypertext inserts unnecessary parens when using attrib[predicate]
// syntax
#![allow(unused_parens)]

mod api;
mod game;
mod morgue;
mod utils;

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::Mutex;

use hypertext::prelude::*;
use hypertext::Raw;

use anyhow::{Context, Result as AnyResult};
use axum::{
    http::{Uri, StatusCode},
    extract::{Path, State, Request},
    body::Body,
    routing::{post, get}, Router,
    response::{IntoResponse},
    handler::HandlerWithoutStateExt,
};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};
use sqlx::{Row, FromRow, Connection, SqliteConnection};
use tower::util::ServiceExt;
use tower_http::services::ServeDir;

use log::{error, info};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::morgue::GameResult;

static FAVICON: &str = "iVBORw0KGgoAAAANSUhEUgAAAB8AAAAfCAMAAAAocOYLAAAAAXNSR0IArs4c6QAAAGZQTFRFAAAADw4LRSg8ZjkxbmFN33Emn49038+0+/I2meVQar4wN5RuS2kvUkskMjw5Pz90MGCCW27hY5v/X83ky9v8////m623hH6HaWpqWVZSdkKKrDIy2Vdj13u6j5dKim8wOU8kW4A5TRFYOgAAACJ0Uk5TAP///////////////////////////////////////////y79k78AAADHSURBVCiRxdGxCsMwDATQHBh5Mdqyy///kz2dncZQmpR2qCjB4dXS2dlwXRtaa3A+mmtZa4XxUU1L+vp3+vo6PCLcI0tuZqVYlhyIDnj03kObjC2KaULOd3r3dARfLKeX9FyO/tyZM6jqz505A+WcL3c/5svL4T7TzXxlppv56JBnPjnkuoGRDyT3dOXjxbB7us3zpQPP86Xzcsb5Zj5lOPMpAz6635vvc/d9/+C76q1P2688f6vX+pvf9X/1/dKP+ub8a/3qD94aFj7571ZOAAAAAElFTkSuQmCC";
static STYLE: &str = include_str!("../assets/main.css");

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Arc<Mutex<SqliteConnection>>,
    pub pages: Vec<StaticPage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Frontmatter {
    page: Page,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StaticPage {
    meta: Frontmatter,
    name: String, // the filename w/o extension
    html: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Page {
    Home,
    Links,
    About,
    Highscore,
    NotFound, // 404
    Error, // 500
    Other(String),
    External(String, String),
}

impl Page {
    pub fn title(&self) -> &str {
        match self {
            Page::Home => "home",
            Page::Links => "links",
            Page::About => "about",
            Page::Highscore => "score list",
            Page::NotFound => "404",
            Page::Error => "500",
            Page::Other(s) => &s,
            Page::External(t, _) => t,
        }
    }

    pub fn href(&self) -> &str {
        match self {
            Page::Home => "/",
            Page::Links => "/links",
            Page::About => "/about",
            Page::Highscore => "/s/ls",
            Page::External(_, s) => &s,
            _ => unreachable!(),
        }
    }
}

macro_rules! try_or_500 {
    ($ex:expr) => {
        match $ex {
            Ok(value) => value,
            Err(err) => {
                error!("E: {:#}", err.to_string());
                return construct_500_page(err.into());
            },
        }
    }
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    let port = std::env::var("PORT")
        .unwrap_or("3000".to_string())
        .parse::<u16>()
        .unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("loap=info,tower_http=trace"))
                .unwrap(),
        )
        .init();

    let mut pages = vec![];
    for entry in fs::read_dir("content")
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().unwrap().is_file())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
    {
        use gray_matter::Matter;
        use gray_matter::engine::YAML;
        use markdown;

        let matter = Matter::<YAML>::new();

        let path = entry.path();
        let raw = fs::read_to_string(&path).unwrap();
        let fname = path.file_stem().unwrap().to_string_lossy();

        let result = matter.parse_with_struct::<Frontmatter>(&raw).unwrap();
        let html = markdown::to_html_with_options(&result.content, &markdown::Options {
            parse: markdown::ParseOptions::gfm(),
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                allow_dangerous_protocol: true,
                ..markdown::CompileOptions::default()
            },
        }).unwrap();

        pages.push(StaticPage {
            meta: result.data,
            name: fname.to_string(),
            html
        });
    }

    let state = AppState {
        db: Arc::new(Mutex::new(
                SqliteConnection::connect("sqlite:main.sqlite3")
                    .await
                    .with_context(|| "Couldn't open database")?,
        )),
        pages,
    };

    let app = Router::new()
        .route("/s/ls", get(highscore_page))
        .route("/s/{score_id}", get(score_page))
        .route("/api/upload", post(api::upload_morgue))
        .route("/api/upload-form", post(api::upload_morgue_form))
        .fallback(static_page)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
}

async fn score_page(
    State(state): State<AppState>,
    Path(score_id): Path<String>
)
    -> impl IntoResponse
{
    info!("page: score: {}", score_id);

    let mut conn = state.db.lock().await;

    #[derive(FromRow)]
    struct Q { morgue: String }

    let query = sqlx::query_as::<_, Q>(
        "SELECT morgue FROM Scores WHERE id = $1;"
    )
        .bind(score_id)
        .fetch_one(&mut *conn)
        .await;

    let morgue_json = match query {
        Ok(q) => q.morgue,
        Err(sqlx::Error::RowNotFound) => return construct_404_page(),
        Err(e) => return construct_500_page(e.into()),
    };
    let morgue: morgue::Morgue = serde_json::from_str(&morgue_json).unwrap();

    let datetime = format!("{}", morgue.info.end_datetime.to_datetime().format("%Y-%m-%d %H:%M"));

    let page = maud! {
        Doc page=(Page::Other("Score".into())) {
            div .morgue-header {
                @let level_name = game::LEVELS[morgue.info.level as usize];

                span .morgue-name     { (morgue.info.username) " the Oathbreaker" }
                span .morgue-result   { (morgue.info.result) }
                @if morgue.result() != GameResult::Win {
                    span .morgue-death { (morgue.info.slain_str) " by a " (morgue.info.slain_by_name) }
                    @if !morgue.info.slain_by_captain_name.is_empty() {
                        span .morgue-death-captain { "...in service of a " (morgue.info.slain_by_captain_name) }
                    }
                }
                span .morgue-location { "at " (level_name) " after " (morgue.info.turns) " turns" }
            }

            br;

            div .panel-list {
                table {
                    tr { td .sideth { "Completed" } td { (datetime) } }
                }
                table {
                    tr { td .sideth { "Seed" } td { (morgue.info.seed) } }
                }
            }

            br;
            Anchor3 a="state" n="State";

            div .panel-list {
                @if morgue.info.stats != crate::morgue::Stats::placeholder() {
                table {
                    thead { tr { th colspan=3 { "Stats" } } }
                    @let s = &morgue.info.stats;
                    tr { td .sideth { "Melee"       } BarTd p=(s.Melee) show=false;          td { (s.Melee)       "%" } }
                    tr { td .sideth { "Missile"     } BarTd p=(s.Missile) show=false;        td { (s.Missile)     "%" } }
                    tr { td .sideth { "Martial"     } td { }                                 td { (s.Martial)         } }
                    tr { td .sideth { "Evade"       } BarTd p=(s.Evade) show=false;          td { (s.Evade)       "%" } }
                    tr { td .sideth { "Speed"       } td { }                                 td { (s.Speed)           } }
                    tr { td .sideth { "Vision"      } td { }                                 td { (s.Vision)          } }
                    tr { td .sideth { "Willpower"   } BarTd p=(s.Willpower * 10) show=false; td { (s.Willpower)       } }
                    tr { td .sideth { "Spikes"      } td { }                                 td { (s.Spikes)          } }
                    tr { td .sideth { "Conjuration" } td { }                                 td { (s.Conjuration)     } }
                    tr { td .sideth { "Potential"   } BarTd p=(s.Potential) show=false;      td { (s.Potential)   "%" } }
                }
                }

                @if morgue.info.resists != crate::morgue::Resistances::placeholder() {
                    table {
                        thead { tr { th colspan=3 { "Resistances" } } }
                        @let s = &morgue.info.resists;
                        tr { td .sideth { "Armor" } SBarTd p=(s.Armor) show=false; td { (s.Armor)"%" } }
                        tr { td .sideth { "rFire" } SBarTd p=(s.rFire) show=false; td { (s.rFire)"%" } }
                        tr { td .sideth { "rElec" } SBarTd p=(s.rElec) show=false; td { (s.rElec)"%" } }
                        tr { td .sideth { "rFume" } BarTd  p=(s.rFume) show=false; td { (s.rFume)    } }
                        tr { td .sideth { "rAcid" } SBarTd p=(s.rAcid) show=false; td { (s.rAcid)"%" } }
                        tr { td .sideth { "rHoly" } SBarTd p=(s.rHoly) show=false; td { (s.rHoly)"%" } }
                    }
                }

                table {
                    thead { tr { th colspan=2 { "Equipment" } } }
                    tbody {
                        @for eq in &morgue.info.equipment {
                            tr {
                                td .sideth { (eq.slot_name) }
                                td { (eq.name) }
                            }
                        }
                    }
                }

                table .list {
                    thead { tr { th colspan=2 { "Inventory" } } }
                    tbody {
                        @for name in &morgue.info.inventory_names {
                            tr {
                                td { (name) }
                            }
                        }
                    }
                }
            }

            br;
            div .panel-list {
                table {
                    thead { tr { th colspan=2 { "Aptitudes" } } }
                    tbody {
                        @let aptitudes = morgue.info.aptitudes_names
                            .iter()
                            .zip(morgue.info.aptitudes_descs.iter());
                        @for (name, desc) in aptitudes {
                            tr {
                                td .sideth { (name) }
                                td { (desc) }
                            }
                        }
                        @if morgue.info.aptitudes_names.is_empty() {
                            tr { td colspan=2 { "(none)" } }
                        }
                    }
                }

                table {
                    thead { tr { th colspan=2 { "Conjuration Augments" } } }
                    tbody hidden[morgue.info.augments_names.len() == 0] {
                        @let augments = morgue.info.augments_names
                            .iter()
                            .zip(morgue.info.augments_descs.iter());
                        @for (name, desc) in augments {
                            tr {
                                td .sideth { (name) }
                                td { (desc) }
                            }
                        }
                        @if morgue.info.augments_names.is_empty() {
                            tr { td colspan=2 { "(none)" } }
                        }
                    }
                }
            }

            br;
            div .panel-list {
                table {
                    thead { tr { th colspan=999 { "Permanent Status Effects" } } }
                    tbody {
                        @let statuses = ||
                            morgue.info.statuses
                                .iter()
                                .filter(|s| s.duration.duration_type.is_perm());
                        @for status in statuses() {
                            tr {
                                td .sideth { (status.duration.duration_type.to_string()) }
                                td { (status.status) }
                                td #m .shrink hidden[status.power == 0] { "<" (status.power) ">" }
                            }
                        }
                        @if statuses().count() == 0 {
                            tr { td colspan=2 { "(none)" } }
                        }
                    }
                }
            }

            br;
            Anchor3 a="circumstances" n="Circumstances";

            div .panel-list {
                table {
                    thead { tr { th colspan=999 { "Status Effects" } } }
                    tbody {
                        @let statuses = ||
                            morgue.info.statuses
                                .iter()
                                .filter(|s| !s.duration.duration_type.is_perm());
                        @for status in statuses() {
                            tr {
                                td .sideth { (status.duration.duration_type.to_string()) }
                                td { (status.status) }
                                td .shrink { "(" (status.duration.duration_arg) ")" }
                                td #m .shrink hidden[status.power == 0] { "<" (status.power) ">" }
                            }
                        }
                        @if statuses().count() == 0 {
                            tr { td colspan=2 { "(none)" } }
                        }
                    }
                }

                table .list {
                    thead { tr { th colspan=999 { "Visible Creatures" } } }
                    tbody {
                        @let creatures =
                            morgue.info.in_view_names
                                .iter()
                                .fold(
                                    HashMap::<&str, usize>::new(),
                                    |mut counts, name| {
                                        *counts.entry(name).or_insert(0) += 1;
                                        counts
                                    }
                                )
                                .into_iter();
                        @for (name, repeat) in creatures {
                            tr {
                                td .shrink {
                                    @if repeat > 1 {
                                        "×" (repeat)
                                    }
                                }
                                td colspan=9 { (name) }
                            }
                        }
                    }
                }
            }

            br;
            hr;

            pre .map {
                code {
                    ({
                        let mut b = String::new();
                        for row in &morgue.info.surroundings {
                            for &ch in row {
                                b.push(char::from_u32(ch).unwrap_or('¿'));
                            }
                            b += "\n";
                        }
                        Raw(b)
                    })
                }
            }

            br;

            table {
                @for message in &morgue.info.messages {
                    tr {
                        td .message { (Raw(utils::fmt_game_message(&message.text))) }
                        td #m hidden[message.dups == 0] { "×" (message.dups) }
                    }
                }
            }

            br;
            Anchor3 a="records" n="Records";

            Anchor4 a="records-general" n="General";
            table .sheet {
                RecordsHeader s=(&morgue.stats);
                tbody {
                    SingleValueSet v=(&morgue.stats.turns_spent)         n="turns spent"             t=true;
                    BatchValueSet  v=(&morgue.stats.turns_with_statuses) n="turns w/ status effects";
                }
            }

            Anchor4 a="records-general-level" n="Before leaving floor";
            table .sheet {
                RecordsHeader s=(&morgue.stats);
                tbody {
                    SingleValueSet v=(&morgue.stats.health)              n="health"                  t=false;
                    SingleValueSet v=(&morgue.stats.night_reputation)    n="night reputation"        t=false;
                }
            }

            br;
            Anchor4 a="records-combat" n="Combat";

            table .sheet {
                RecordsHeader s=(&morgue.stats);
                tbody {
                    BatchValueSet  v=(&morgue.stats.vanquished_foes)     n="vanquished foes";
                    BatchValueSet  v=(&morgue.stats.stabbed_foes)        n="foes slain by surprise";
                    BatchValueSet  v=(&morgue.stats.inflicted_damage)    n="inflicted damage";
                    BatchValueSet  v=(&morgue.stats.endured_damage)      n="endured damage";
                    BatchValueSet  v=(&morgue.stats.endured_spells)      n="endured spells";
                    BatchValueSet  v=(&morgue.stats.endured_healing)     n="health restored";
                }
            }

            br;
            Anchor4 a="records-items" n="Items";

            table .sheet {
                RecordsHeader s=(&morgue.stats);
                tbody {
                    BatchValueSet  v=(&morgue.stats.items_thrown)        n="items thrown";
                    BatchValueSet  v=(&morgue.stats.items_used)          n="items used";
                    BatchValueSet  v=(&morgue.stats.rings_used)          n="rings used";
                }
            }

            br;
            Anchor4 a="records-misc" n="Misc";

            table .sheet {
                RecordsHeader s=(&morgue.stats);
                tbody {
                    SingleValueSet v=(&morgue.stats.lairs_trespassed)    n="lairs_trespassed"   t=true;
                    SingleValueSet v=(&morgue.stats.candles_destroyed)   n="candles destroyed"  t=true;
                    SingleValueSet v=(&morgue.stats.shrines_drained)     n="shrines drained"    t=true;
                    BatchValueSet  v=(&morgue.stats.times_corrupted)     n="times corrupted";
                    BatchValueSet  v=(&morgue.stats.wizard_keys_used)    n="cheat codes used";
                }
            }
        }
    }.render();

    (StatusCode::OK, page)
}

#[component]
fn anchor_4<'a>(a: &'a str, n: &'a str) -> impl Renderable + use<'a> {
    maud! {
        h4 #(a) { a .anchor href=("#".to_owned() + a) { (n) } }
    }
}

#[component]
fn anchor_3<'a>(a: &'a str, n: &'a str) -> impl Renderable + use<'a> {
    maud! {
        h3 #(a) { a .anchor href=("#".to_owned() + a) { (n) } }
    }
}

#[component]
fn records_header<'a>(s: &'a morgue::MorgueStats) -> impl Renderable + use<'a> {
    maud! {
        thead {
            tr {
                th { }
                th { }
                th { }
                @for v in &s.turns_spent.value.values {
                    th { (v.floor_name.chars().nth(0)) }
                }
            }
            tr {
                th { }
                th { }
                th { "total" }
                @for v in &s.turns_spent.value.values {
                    th { (v.floor_type) }
                }
            }
        }
    }
}

#[component]
fn single_value_set<'a>(v: &'a morgue::SingleValueSet, n: &'a str, t: bool) -> impl Renderable + use<'a> {
    maud! {
        tr {
            td { (n) }
            td { }
            @if t {
                td #m { (v.value.total) }
            }
            @for c in &v.value.values {
                td #m { (c.value) }
            }
        }
    }
}

#[component]
fn batch_value_set<'a>(v: &'a morgue::BatchValueSet, n: &'a str) -> impl Renderable + use<'a> {
    maud! {
        tr {
            td { (n) }
            td { }
            td #m { (v.total()) }

            @let columns = v.values.get(0).map(|r| r.value.values.len()).unwrap_or(0); 
            @let column_totals =
                (0..columns)
                    .map(|c|
                        v.values
                            .iter()
                            .map(|r| r.value.values.get(c).map(|rs| rs.value).unwrap_or(0))
                            .reduce(|a, i| a + i)
                            .unwrap_or(0)
                            as usize
                    );
            @for column_total in column_totals {
                BarTd p=(column_total * 100 / v.total() as usize) show=true;
            }
        }
        @for row in &v.values {
            tr {
                td #sub { (row.name) }
                BarTd p=((row.value.total * 100 / v.total()) as usize) show=true;
                td #m { (row.value.total) }
                @for c in &row.value.values {
                    @let is_zero = c.value == 0;
                    td #m .subtle[is_zero] { (c.value) }
                }
            }
        }
    }
}

#[component]
fn s_bar_td(p: isize, show: bool) -> impl Renderable {
    // Allow a little overfill to show that it exceeds 100%
    let clamped_percent = p.abs().clamp(0, 110); 
    let style = format!("width: {clamped_percent}%");

    maud! {
        td .bar-outer {
            div .bar {
                div .bar-inner .bar-neg[p < 0] style=(style) {
                    @if show {
                        (p) "%"
                    }
                }
            }
        }
    }
}

#[component]
fn bar_td(p: usize, show: bool) -> impl Renderable {
    maud! {
        td .bar-outer { Bar p=(p) show=(show); }
    }
}

#[component]
fn bar(p: usize, show: bool) -> impl Renderable {
    // Allow a little overfill to show that it exceeds 100%
    let clamped_percent = p.clamp(0, 110); 
    let style = format!("width: {clamped_percent}%");
    maud! {
        div .bar { div .bar-inner style=(style) { @if show { (p) "%" } } }
    }
}

async fn highscore_page(State(state): State<AppState>) -> impl IntoResponse {
    info!("page: highscore");

    let mut conn = state.db.lock().await;

    #[derive(Clone, FromRow)]
    struct Scores {
        id: i64,
        date: DateTime<Utc>,
        result: morgue::GameResult,
        player_name: String,
        end_level: u8,
        slain_foes: u32,
        stabbed_foes: u32,
    }

    let items = try_or_500!(sqlx::query_as::<_, Scores>(
        "SELECT
            s.id, p.name as player_name, s.date, s.result, s.end_level,
            s.slain_foes, s.stabbed_foes
        FROM Scores s
        JOIN Players p ON p.id = s.player
        ORDER BY s.end_level ASC, s.slain_foes DESC;"
    ).fetch_all(&mut *conn).await);

    let page = maud! {
        Doc page=(Page::Highscore) {
            h2 { "Highscore" }

            table .list {
                thead {
                    tr {
                        th { "player" }
                        th { "date/time" }
                        th { "result" }
                        th { "ended at" }
                        th { "foes vanquished" }
                    }
                }
                tbody {
                    @for item in items.iter() {
                        tr {
                            td { (item.player_name) }
                            td #m { (format!("{}", item.date.format("%Y-%m-%d %H:%M"))) }
                            td {
                                a .btn href=(format!("/s/{}", item.id)) {
                                    (match item.result {
                                        GameResult::Win => "won",
                                        GameResult::Lose => "died",
                                        GameResult::Unknown => "??",
                                    })
                                }
                            }
                            td { (game::LEVELS[item.end_level as usize]) }
                            td {
                                (item.slain_foes)" slain, "(item.stabbed_foes)" by surprise"
                            }
                        }
                    }
                }
            }

            h3 { "Manual Upload" }

            "You may upload your morgue file (" code { ".json" } ") manually if it does not appear here."

            br;
            form action="/api/upload-form" method="POST" enctype="multipart/form-data" {
                input type="file" id="file" name="file" required;
                button type="submit" { "Submit" }
            }
        }
    }.render();

    (StatusCode::OK, page)
}

async fn static_page(
    State(state): State<AppState>,
    uri: Uri,
) -> impl IntoResponse {
    let mut path = uri.path()
        .trim_start_matches("/"); // path() returns '/faq' instead of just 'faq'


    if path.is_empty() {
        path = "index";
    }

    // First, try a loaded HTML page
    info!("page: checking for static page matching path '{path}'");
    for page in &state.pages {
        if page.name == path
        || page.name == format!("{path}.html")
        {
            info!("page: {}", page.name);
            return api::AnyOf2::A((StatusCode::OK, maud! {
                Doc page=(page.meta.page.clone()) {
                    (Raw(page.html.clone()))
                }
            }.render()));
        }
    }

    // Then check for a public file...
    info!("page: fallback to servdir for {}", path);
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    match ServeDir::new("public")
        .not_found_service(e404_page.into_service())
        .oneshot(req)
        .await
    {
        Ok(res) => api::AnyOf2::B(res),
        Err(err) => api::AnyOf2::A(construct_500_page(err.into())),
    }
}

// Async wrapper around construct_404_page(), since non-async doesn't implement Handler.
async fn e404_page() -> impl IntoResponse {
    construct_404_page()
}

fn construct_500_page(e: anyhow::Error) -> (StatusCode, Rendered<String>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        maud! {
            Doc page=(Page::Error) {
                h2 { "500 Internal Server Error" }
                p { (e.to_string()) }
            }
        }.render()
    )
}

fn construct_404_page() -> (StatusCode, Rendered<String>) {
    (
        StatusCode::NOT_FOUND,
        maud! {
            Doc page=(Page::NotFound) {
                h2 { "404 Not Found" }
            }
        }.render()
    )
}

struct Doc<R: Renderable> {
    page: Page,
    children: R,
}

impl<R: Renderable> Renderable for Doc<R> {
    fn render_to(&self, output: &mut String) {
        let nav_pages = &[
            Page::Home, Page::Links, Page::Highscore,
            Page::External("download".into(), "https://github.com/kiedtl/roguelike/releases/tag/v4.1.0".into()),
            Page::External("discord".into(), "https://discord.gg/tUhUHffRCr".into()),
            Page::About,
        ];

        maud! {
            !DOCTYPE
            html {
                head lang="en" {
                    meta charset="utf-8";
                    link href=(format!("data:image/png;base64,{FAVICON}")) rel="icon";

                    script data-goatcounter="https://oathbreaker.goatcounter.com/count"
                        async src="//gc.zgo.at/count.js" { }

                    style { (Raw(STYLE)) }
                    title {
                        "Oathbreaker — " (self.page.title())
                    }
                }
                body {
                    aside {
                        img src="/images/avatar.png";
                        h1 { a href="/" { "Oathbreaker" } }
                        h3 { "A traditional roguelike with a focus on stealth" }
                        nav {
                            @for page in nav_pages {
                                @if matches!(page, Page::External(_, _)) {
                                    a .nav target="_blank" href=(page.href()) {
                                        (page.title()) " "
                                        img .inline src="/images/external.svg";
                                    }
                                } @else if *page == self.page {
                                    a .sel .nav href=(page.href()) { (page.title()) }
                                } @else {
                                    a .nav href=(page.href()) { (page.title()) }
                                }
                            }
                        }
                    }
                    main {
                        (self.children)
                        br;
                        hr;
                        footer {
                            p style="margin: 0.2rem 0 0.2rem 0" {
                                "Kiëd Llaentenn © 2021-2026 —"
                                a rel="license" href="http://creativecommons.org/licenses/by-nc-nd/4.0/" {
                                    (Raw("<svg class='inline' version='1.0' id='Layer_1' xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' x='0px' y='0px' width='64px' height='64px' viewBox='5.5 -3.5 64 64' enable-background='new 5.5 -3.5 64 64' xml:space='preserve'> <g> <circle fill='#FFFFFF' cx='37.785' cy='28.501' r='28.836'/> <path d='M37.441-3.5c8.951,0,16.572,3.125,22.857,9.372c3.008,3.009,5.295,6.448,6.857,10.314 c1.561,3.867,2.344,7.971,2.344,12.314c0,4.381-0.773,8.486-2.314,12.313c-1.543,3.828-3.82,7.21-6.828,10.143 c-3.123,3.085-6.666,5.448-10.629,7.086c-3.961,1.638-8.057,2.457-12.285,2.457s-8.276-0.808-12.143-2.429 c-3.866-1.618-7.333-3.961-10.4-7.027c-3.067-3.066-5.4-6.524-7-10.372S5.5,32.767,5.5,28.5c0-4.229,0.809-8.295,2.428-12.2 c1.619-3.905,3.972-7.4,7.057-10.486C21.08-0.394,28.565-3.5,37.441-3.5z M37.557,2.272c-7.314,0-13.467,2.553-18.458,7.657 c-2.515,2.553-4.448,5.419-5.8,8.6c-1.354,3.181-2.029,6.505-2.029,9.972c0,3.429,0.675,6.734,2.029,9.913 c1.353,3.183,3.285,6.021,5.8,8.516c2.514,2.496,5.351,4.399,8.515,5.715c3.161,1.314,6.476,1.971,9.943,1.971 c3.428,0,6.75-0.665,9.973-1.999c3.219-1.335,6.121-3.257,8.713-5.771c4.99-4.876,7.484-10.99,7.484-18.344 c0-3.543-0.648-6.895-1.943-10.057c-1.293-3.162-3.18-5.98-5.654-8.458C50.984,4.844,44.795,2.272,37.557,2.272z M37.156,23.187 l-4.287,2.229c-0.458-0.951-1.019-1.619-1.685-2c-0.667-0.38-1.286-0.571-1.858-0.571c-2.856,0-4.286,1.885-4.286,5.657 c0,1.714,0.362,3.084,1.085,4.113c0.724,1.029,1.791,1.544,3.201,1.544c1.867,0,3.181-0.915,3.944-2.743l3.942,2 c-0.838,1.563-2,2.791-3.486,3.686c-1.484,0.896-3.123,1.343-4.914,1.343c-2.857,0-5.163-0.875-6.915-2.629 c-1.752-1.752-2.628-4.19-2.628-7.313c0-3.048,0.886-5.466,2.657-7.257c1.771-1.79,4.009-2.686,6.715-2.686 C32.604,18.558,35.441,20.101,37.156,23.187z M55.613,23.187l-4.229,2.229c-0.457-0.951-1.02-1.619-1.686-2 c-0.668-0.38-1.307-0.571-1.914-0.571c-2.857,0-4.287,1.885-4.287,5.657c0,1.714,0.363,3.084,1.086,4.113 c0.723,1.029,1.789,1.544,3.201,1.544c1.865,0,3.18-0.915,3.941-2.743l4,2c-0.875,1.563-2.057,2.791-3.541,3.686 c-1.486,0.896-3.105,1.343-4.857,1.343c-2.896,0-5.209-0.875-6.941-2.629c-1.736-1.752-2.602-4.19-2.602-7.313 c0-3.048,0.885-5.466,2.658-7.257c1.77-1.79,4.008-2.686,6.713-2.686C51.117,18.558,53.938,20.101,55.613,23.187z'/> </g> </svg>"))
                                }
                            }
                            a href="https://github.com/kiedtl/badges" {
                                img .inline .badge src="//tilde.team/~kiedtl/images/badges/netscape/light.gif";
                            }
                        }
                    }
                }
            }
        }
        .render_to(output);
    }
}
