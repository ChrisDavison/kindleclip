use crate::export::{has_exportable, render_book, render_zip, ExportOpts};
use crate::model::{self, Book, Kind};
use crate::state::{BookState, ClusterState, PersistentState};
use anyhow::{anyhow, Context, Result};
use askama::Template;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ServerState {
    state_path: PathBuf,
    data: PersistentState,
    library: HashMap<String, Book>,
}

type Shared = Arc<Mutex<ServerState>>;

fn lock(state: &Shared) -> MutexGuard<'_, ServerState> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn default_state_path(first_source: &FsPath) -> PathBuf {
    let dir = first_source.parent().unwrap_or_else(|| FsPath::new("."));
    let stem = first_source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "kindleclip".to_string());
    dir.join(format!("{stem}.kindleclip.json"))
}

/// Entry point for `kindleclip --serve`.
pub async fn run(args: crate::Opts) -> Result<()> {
    let sources = args.files.clone();
    if sources.is_empty() {
        return Err(anyhow!("serve mode needs at least one source file"));
    }
    let state_path = match &args.state {
        Some(p) => p.clone(),
        None => default_state_path(&sources[0]),
    };
    let data = PersistentState::load(&state_path)?;

    let mut library: HashMap<String, Book> = HashMap::new();
    for src in &sources {
        let text = std::fs::read_to_string(src)
            .with_context(|| format!("Failed to read clippings file {}", src.display()))?;
        let books = model::parse_source(src, &text)?;
        model::merge_into(&mut library, books);
    }
    let base = state_path
        .parent()
        .unwrap_or_else(|| FsPath::new("."))
        .to_path_buf();
    for up in data.uploads.clone() {
        let path = base.join(&up);
        let parsed = std::fs::read_to_string(&path).ok().and_then(|t| model::parse_source(&path, &t).ok());
        match parsed {
            Some(books) => model::merge_into(&mut library, books),
            None => eprintln!("Skipping stored upload {}: unreadable or unsupported", path.display()),
        }
    }

    let shared = Arc::new(Mutex::new(ServerState { state_path, data, library }));
    let app = router(shared.clone());
    let addr = format!("{}:{}", args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind {addr}"))?;
    println!(
        "Serving {} book(s) on http://{}",
        lock(&shared).library.len(),
        addr
    );
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/book/{book_id}", get(book_page))
        .route("/import", get(import_page).post(import_post))
        .route("/static/style.css", get(style_css))
        .route("/static/app.js", get(app_js))
        .route("/download/book/{book_id}", get(download_book))
        .route("/download/all", get(download_all))
        .route("/api/clipping/{book_id}/{clipping_id}/mark", post(api_mark))
        .route("/api/clipping/{book_id}/{clipping_id}/ignore", post(api_ignore))
        .route("/api/clipping/{book_id}/{clipping_id}/annotate", post(api_annotate))
        .route("/api/book/{book_id}/cluster", post(api_cluster_create))
        .route("/api/cluster/{book_id}/{cluster_id}/rename", post(api_cluster_rename))
        .route("/api/cluster/{book_id}/{cluster_id}/assign", post(api_cluster_assign))
        .route("/api/cluster/{book_id}/{cluster_id}/order", post(api_cluster_order))
        .route("/api/cluster/{book_id}/{cluster_id}", delete(api_cluster_delete))
        .route("/api/marks/clear", post(api_marks_clear))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}

// ----- pages -----

#[derive(Deserialize)]
struct IndexQuery {
    sort: Option<String>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    sort: String,
    books: Vec<BookRow>,
}

struct BookRow {
    id: String,
    title: String,
    author: String,
    count: usize,
    latest: String,
    sources: String,
    latest_ts: Option<DateTime<Utc>>,
    last_pos: usize,
}

async fn index(State(state): State<Shared>, Query(q): Query<IndexQuery>) -> Response {
    let sort = q.sort.unwrap_or_default();
    let guard = lock(&state);
    let mut rows: Vec<BookRow> = guard
        .library
        .values()
        .map(|b| BookRow {
            id: b.id.clone(),
            title: b.title.clone(),
            author: b.author.clone().unwrap_or_default(),
            count: b.clippings.len(),
            latest: b.latest().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            sources: b.source_labels(),
            latest_ts: b.latest(),
            last_pos: b.last_pos,
        })
        .collect();
    if sort == "alpha" {
        rows.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    } else {
        rows.sort_by(|a, b| {
            (b.latest_ts, b.last_pos)
                .cmp(&(a.latest_ts, a.last_pos))
        });
    }
    let tmpl = IndexTemplate { sort, books: rows };
    match tmpl.render() {
        Ok(body) => Html(body).into_response(),
        Err(e) => internal_error(e),
    }
}

#[derive(Template)]
#[template(path = "book.html")]
struct BookTemplate {
    book_id: String,
    title: String,
    author: String,
    marked_count: usize,
    clusters: Vec<ClusterView>,
    unclustered: Vec<ClippingView>,
}

struct ClusterView {
    id: String,
    name: String,
    items: Vec<ClippingView>,
}

struct ClippingView {
    id: String,
    text: String,
    note: String,
    location: String,
    added: String,
    chapter: String,
    source_label: String,
    is_note: bool,
    marked: bool,
    ignored: bool,
    annotation: String,
    in_cluster: bool,
    cluster_options: Vec<ClusterOption>,
}

struct ClusterOption {
    id: String,
    name: String,
    member: bool,
}

async fn book_page(State(state): State<Shared>, Path(book_id): Path<String>) -> Response {
    let guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    let bs = guard.data.books.get(&book_id).cloned().unwrap_or_default();
    let tmpl = build_book_template(book, &bs);
    match tmpl.render() {
        Ok(body) => Html(body).into_response(),
        Err(e) => internal_error(e),
    }
}

fn build_book_template(book: &Book, bs: &BookState) -> BookTemplate {
    let view = |c: &model::Clipping| -> ClippingView {
        let cs = bs.clipping_state(&c.id).cloned().unwrap_or_default();
        let member_of = bs.cluster_of(&c.id).map(|s| s.to_string());
        ClippingView {
            id: c.id.clone(),
            text: c.text.clone(),
            note: c.note.clone().unwrap_or_default(),
            location: c.location.clone().unwrap_or_default(),
            added: c
                .added
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
            chapter: c.chapter.clone().unwrap_or_default(),
            source_label: c.source.label().to_string(),
            is_note: c.kind == Kind::Note,
            marked: cs.marked,
            ignored: cs.ignored,
            annotation: cs.annotation,
            in_cluster: member_of.is_some(),
            cluster_options: bs
                .clusters
                .iter()
                .map(|cl| ClusterOption {
                    id: cl.id.clone(),
                    name: cl.name.clone(),
                    member: member_of.as_deref() == Some(cl.id.as_str()),
                })
                .collect(),
        }
    };
    let clusters = bs
        .clusters
        .iter()
        .map(|cl| ClusterView {
            id: cl.id.clone(),
            name: cl.name.clone(),
            items: cl
                .clipping_ids
                .iter()
                .filter_map(|cid| book.clippings.iter().find(|c| c.id == *cid))
                .map(&view)
                .collect(),
        })
        .collect();
    let unclustered = book
        .clippings
        .iter()
        .filter(|c| bs.cluster_of(&c.id).is_none())
        .map(&view)
        .collect();
    let marked_count = book
        .clippings
        .iter()
        .filter(|c| {
            bs.clipping_state(&c.id)
                .map(|s| s.marked && !s.ignored)
                .unwrap_or(false)
        })
        .count();
    BookTemplate {
        book_id: book.id.clone(),
        title: book.title.clone(),
        author: book.author.clone().unwrap_or_default(),
        marked_count,
        clusters,
        unclustered,
    }
}

#[derive(Template)]
#[template(path = "import.html")]
struct ImportTemplate;

async fn import_page() -> Response {
    match ImportTemplate.render() {
        Ok(body) => Html(body).into_response(),
        Err(e) => internal_error(e),
    }
}

async fn import_post(State(state): State<Shared>, mut multipart: Multipart) -> Response {
    let mut pending: Vec<(String, String, Vec<Book>)> = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Bad upload: {e}"),
                )
                    .into_response()
            }
        };
        let Some(file_name) = field.file_name().map(|s| s.to_string()) else {
            continue;
        };
        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("Bad upload: {e}")).into_response(),
        };
        if bytes.is_empty() {
            continue;
        }
        let clean = sanitize_filename(&file_name);
        let content = String::from_utf8_lossy(&bytes).to_string();
        let fake_path = PathBuf::from(&clean);
        match model::parse_source(&fake_path, &content) {
            Ok(books) => pending.push((clean, content, books)),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Could not parse {clean}: {e}"),
                )
                    .into_response()
            }
        }
    }

    let mut guard = lock(&state);
    let sources_dir = guard
        .state_path
        .parent()
        .unwrap_or_else(|| FsPath::new("."))
        .join("kindleclip-sources");
    for (name, content, books) in pending {
        if let Err(e) = std::fs::create_dir_all(&sources_dir) {
            return internal_error(anyhow!("Failed to create sources dir: {}", e));
        }
        let dest = sources_dir.join(&name);
        if let Err(e) = std::fs::write(&dest, content.as_bytes()) {
            return internal_error(anyhow!("Failed to store upload: {}", e));
        }
        let rel = format!(
            "kindleclip-sources/{}",
            dest.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        );
        if !guard.data.uploads.contains(&rel) {
            guard.data.uploads.push(rel);
        }
        model::merge_into(&mut guard.library, books);
    }
    if let Err(e) = guard.data.save(&guard.state_path) {
        return internal_error(e);
    }
    Redirect::to("/").into_response()
}

// ----- static assets -----

async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("assets/style.css"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("assets/app.js"),
    )
}

// ----- downloads -----

#[derive(Deserialize)]
struct DownloadQuery {
    marked: Option<String>,
}

async fn download_book(
    State(state): State<Shared>,
    Path(book_id): Path<String>,
    Query(q): Query<DownloadQuery>,
) -> Response {
    let guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    let bs = guard.data.books.get(&book_id).cloned().unwrap_or_default();
    let opts = ExportOpts {
        as_list: false,
        only_marked: q.marked.is_some(),
    };
    if opts.only_marked && !has_exportable(book, &bs, &opts) {
        return (StatusCode::BAD_REQUEST, "No clippings are marked for export").into_response();
    }
    let body = render_book(book, &bs, &opts);
    respond_file(
        &format!("{}.md", book.filestem()),
        "text/markdown; charset=utf-8",
        body.into_bytes(),
    )
}

async fn download_all(State(state): State<Shared>) -> Response {
    let guard = lock(&state);
    let opts = ExportOpts { as_list: false, only_marked: false };
    let mut books: Vec<&Book> = guard.library.values().collect();
    books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();
    for book in books {
        let bs = guard.data.books.get(&book.id).cloned().unwrap_or_default();
        if !has_exportable(book, &bs, &opts) {
            continue;
        }
        let body = render_book(book, &bs, &opts);
        let mut stem = book.filestem();
        let mut n = 2;
        while !used_names.insert(stem.clone()) {
            stem = format!("{}-{n}", book.filestem());
            n += 1;
        }
        entries.push((format!("{stem}.md"), body));
    }
    match render_zip(entries) {
        Ok(bytes) => respond_file("kindleclip.zip", "application/zip", bytes),
        Err(e) => internal_error(e),
    }
}

fn respond_file(filename: &str, content_type: &str, bytes: Vec<u8>) -> Response {
    let safe = filename.replace(['"', '\\'], "_");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{safe}\""))
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(axum::body::Body::from(bytes))
        .unwrap()
}

// ----- api -----

#[derive(Deserialize)]
struct MarkBody {
    marked: bool,
}

#[derive(Deserialize)]
struct IgnoreBody {
    ignored: bool,
}

#[derive(Deserialize)]
struct AnnotateBody {
    text: String,
}

#[derive(Deserialize)]
struct ClusterCreateBody {
    name: String,
}

#[derive(Deserialize)]
struct RenameBody {
    name: String,
}

#[derive(Deserialize)]
struct AssignBody {
    clipping_id: String,
    member: bool,
}

#[derive(Deserialize)]
struct OrderBody {
    clipping_ids: Vec<String>,
}

async fn api_mark(
    State(state): State<Shared>,
    Path((book_id, clipping_id)): Path<(String, String)>,
    Json(body): Json<MarkBody>,
) -> Response {
    let mut guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    if !book.clippings.iter().any(|c| c.id == clipping_id) {
        return not_found("No such clipping");
    }
    guard.data.book(&book_id).clipping(&clipping_id).marked = body.marked;
    finish(&guard)
}

async fn api_ignore(
    State(state): State<Shared>,
    Path((book_id, clipping_id)): Path<(String, String)>,
    Json(body): Json<IgnoreBody>,
) -> Response {
    let mut guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    if !book.clippings.iter().any(|c| c.id == clipping_id) {
        return not_found("No such clipping");
    }
    guard.data.book(&book_id).clipping(&clipping_id).ignored = body.ignored;
    finish(&guard)
}

async fn api_annotate(
    State(state): State<Shared>,
    Path((book_id, clipping_id)): Path<(String, String)>,
    Json(body): Json<AnnotateBody>,
) -> Response {
    let mut guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    if !book.clippings.iter().any(|c| c.id == clipping_id) {
        return not_found("No such clipping");
    }
    guard.data.book(&book_id).clipping(&clipping_id).annotation = body.text;
    finish(&guard)
}

async fn api_cluster_create(
    State(state): State<Shared>,
    Path(book_id): Path<String>,
    Json(body): Json<ClusterCreateBody>,
) -> Response {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Cluster name must not be empty").into_response();
    }
    let mut guard = lock(&state);
    if !guard.library.contains_key(&book_id) {
        return not_found("No such book");
    }
    let id = model::fnv1a64(&format!("{book_id}\u{1}{name}\u{1}{}", epoch_millis()));
    guard.data.book(&book_id).clusters.push(ClusterState {
        id,
        name,
        clipping_ids: Vec::new(),
    });
    finish(&guard)
}

async fn api_cluster_rename(
    State(state): State<Shared>,
    Path((book_id, cluster_id)): Path<(String, String)>,
    Json(body): Json<RenameBody>,
) -> Response {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "Cluster name must not be empty").into_response();
    }
    let mut guard = lock(&state);
    match guard
        .data
        .book(&book_id)
        .clusters
        .iter_mut()
        .find(|c| c.id == cluster_id)
    {
        Some(cluster) => cluster.name = name,
        None => return not_found("No such cluster"),
    }
    finish(&guard)
}

async fn api_cluster_delete(
    State(state): State<Shared>,
    Path((book_id, cluster_id)): Path<(String, String)>,
) -> Response {
    let mut guard = lock(&state);
    let bs = guard.data.book(&book_id);
    let before = bs.clusters.len();
    bs.clusters.retain(|c| c.id != cluster_id);
    if bs.clusters.len() == before {
        return not_found("No such cluster");
    }
    finish(&guard)
}

async fn api_cluster_assign(
    State(state): State<Shared>,
    Path((book_id, cluster_id)): Path<(String, String)>,
    Json(body): Json<AssignBody>,
) -> Response {
    let mut guard = lock(&state);
    let Some(book) = guard.library.get(&book_id) else {
        return not_found("No such book");
    };
    if !book.clippings.iter().any(|c| c.id == body.clipping_id) {
        return not_found("No such clipping");
    }
    let bs = guard.data.book(&book_id);
    let Some(cluster) = bs.clusters.iter_mut().find(|c| c.id == cluster_id) else {
        return not_found("No such cluster");
    };
    if body.member {
        if !cluster.clipping_ids.contains(&body.clipping_id) {
            cluster.clipping_ids.push(body.clipping_id);
        }
    } else {
        cluster.clipping_ids.retain(|id| id != &body.clipping_id);
    }
    finish(&guard)
}

async fn api_cluster_order(
    State(state): State<Shared>,
    Path((book_id, cluster_id)): Path<(String, String)>,
    Json(body): Json<OrderBody>,
) -> Response {
    let mut guard = lock(&state);
    if !guard.library.contains_key(&book_id) {
        return not_found("No such book");
    }
    // Clone the known ids so the library borrow ends before the state is
    // mutated below.
    let known: HashSet<String> = guard
        .library
        .get(&book_id)
        .map(|b| b.clippings.iter().map(|c| c.id.clone()).collect())
        .unwrap_or_default();
    let bs = guard.data.book(&book_id);
    let Some(cluster) = bs.clusters.iter_mut().find(|c| c.id == cluster_id) else {
        return not_found("No such cluster");
    };
    cluster.clipping_ids = body
        .clipping_ids
        .into_iter()
        .filter(|id| known.contains(id))
        .collect();
    finish(&guard)
}

async fn api_marks_clear(State(state): State<Shared>) -> Response {
    let mut guard = lock(&state);
    let book_ids: Vec<String> = guard.library.keys().cloned().collect();
    for book_id in book_ids {
        let clipping_ids: Vec<String> = guard
            .library
            .get(&book_id)
            .map(|b| b.clippings.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default();
        let bs = guard.data.book(&book_id);
        for cid in clipping_ids {
            bs.clipping(&cid).marked = false;
        }
    }
    finish(&guard)
}

/// Persist state and answer with success.
fn finish(guard: &ServerState) -> Response {
    match guard.data.save(&guard.state_path) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => internal_error(e),
    }
}

// ----- helpers -----

fn not_found(message: &str) -> Response {
    (StatusCode::NOT_FOUND, message.to_string()).into_response()
}

fn internal_error(error: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
}

fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("upload");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        format!("upload-{}", epoch_millis())
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Clipping, Source};
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn clip(id: &str, kind: Kind, text: &str) -> model::Clipping {
        Clipping {
            id: id.to_string(),
            kind,
            text: text.to_string(),
            note: None,
            location: None,
            page: None,
            chapter: None,
            added: None,
            source: Source::MyClippings,
        }
    }

    fn fixture() -> Shared {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let book = Book {
            id: "book1".into(),
            title: "Test Book".into(),
            author: Some("An Author".into()),
            clippings: vec![
                clip("c1", Kind::Highlight, "first"),
                clip("c2", Kind::Highlight, "second"),
                clip("c3", Kind::Note, "a kindle note"),
            ],
            last_pos: 3,
        };
        let mut library = HashMap::new();
        library.insert("book1".to_string(), book);
        let state_path = std::env::temp_dir().join(format!("kindleclip-server-test-{}-{n}.json", std::process::id()));
        let _ = std::fs::remove_file(&state_path);
        Arc::new(Mutex::new(ServerState {
            state_path,
            data: PersistentState::default(),
            library,
        }))
    }

    async fn call(app: Router, req: Request<Body>) -> (StatusCode, String) {
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    fn json_req(method: &str, uri: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn index_lists_books() {
        let app = router(fixture());
        let (status, body) = call(app, Request::builder().uri("/").body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Test Book"), "body: {body}");
        assert!(body.contains("An Author"));
        assert!(body.contains("3 clippings"), "body: {body}");
    }

    #[tokio::test]
    async fn index_sorts_by_alpha() {
        let state = fixture();
        let mut second = Book {
            id: "book2".into(),
            title: "Aardvark".into(),
            author: None,
            clippings: vec![clip("c9", Kind::Highlight, "zzz")],
            last_pos: 0,
        };
        second.clippings[0].added = None;
        lock(&state).library.insert("book2".into(), second);
        let app = router(state);
        let (_, body) = call(
            app,
            Request::builder().uri("/?sort=alpha").body(Body::empty()).unwrap(),
        )
        .await;
        let aardvark = body.find("Aardvark").unwrap();
        let test_book = body.find("Test Book").unwrap();
        assert!(aardvark < test_book, "alpha sort should list Aardvark first");
    }

    #[tokio::test]
    async fn mark_then_download_marked() {
        let app = router(fixture());
        let (status, _) = call(
            app.clone(),
            json_req("POST", "/api/clipping/book1/c1/mark", "{\"marked\":true}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/download/book/book1?marked=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("first"));
        assert!(!body.contains("second"), "unmarked clippings excluded");
    }

    #[tokio::test]
    async fn annotate_then_download() {
        let app = router(fixture());
        let (status, _) = call(
            app.clone(),
            json_req(
                "POST",
                "/api/clipping/book1/c1/annotate",
                "{\"text\":\"my thought\"}",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/download/book/book1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("first\n\n> my thought"), "body: {body}");
    }

    #[tokio::test]
    async fn ignore_excludes_from_download() {
        let app = router(fixture());
        let (status, _) = call(
            app.clone(),
            json_req("POST", "/api/clipping/book1/c1/ignore", "{\"ignored\":true}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/download/book/book1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(!body.contains("first"));
        assert!(body.contains("second"));
    }

    #[tokio::test]
    async fn cluster_lifecycle() {
        let shared = fixture();
        let state_path = lock(&shared).state_path.clone();
        let app = router(shared);
        let (status, _) = call(
            app.clone(),
            json_req("POST", "/api/book/book1/cluster", "{\"name\":\"Favourites\"}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let data = PersistentState::load(&state_path).unwrap();
        let cluster_id = data.books["book1"].clusters[0].id.clone();

        let (status, _) = call(
            app.clone(),
            json_req(
                "POST",
                &format!("/api/cluster/book1/{cluster_id}/assign"),
                "{\"clipping_id\":\"c2\",\"member\":true}",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = call(
            app.clone(),
            Request::builder()
                .uri("/download/book/book1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("## Favourites\n\nsecond\n\n## Unclustered\n\nfirst"),
            "body: {body}"
        );

        let (status, _) = call(
            app.clone(),
            json_req(
                "POST",
                &format!("/api/cluster/book1/{cluster_id}/rename"),
                "{\"name\":\"Best bits\"}",
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = call(
            app.clone(),
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/cluster/book1/{cluster_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let data = PersistentState::load(&state_path).unwrap();
        assert!(data.books["book1"].clusters.is_empty());
    }

    #[tokio::test]
    async fn unknown_book_is_404() {
        let app = router(fixture());
        let (status, _) = call(
            app,
            Request::builder()
                .uri("/download/book/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn clear_marks_resets_everything() {
        let app = router(fixture());
        for cid in ["c1", "c2"] {
            let (status, _) = call(
                app.clone(),
                json_req(
                    "POST",
                    &format!("/api/clipping/book1/{cid}/mark"),
                    "{\"marked\":true}",
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, _) = call(
            app.clone(),
            json_req("POST", "/api/marks/clear", "{}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(
            app,
            Request::builder()
                .uri("/download/book/book1?marked=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "nothing left marked");
        assert!(body.contains("No clippings are marked"));
    }
}
