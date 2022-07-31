use std::{
    fs::File,
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use actix_session::{
    config::BrowserSession, storage::CookieSessionStore, Session, SessionMiddleware,
};
use actix_web::{
    cookie::{time::Duration, Key, SameSite},
    get, post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use async_graphql::{futures_util::lock::Mutex, http::graphiql_source, EmptySubscription, Schema};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, SqliteConnection};
use structopt::StructOpt;

mod config;
use config::Config;
mod images;
mod schema;
mod user_session;

pub type FileDatabase = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;
pub type MetadataDatabase = Arc<Mutex<SqliteConnection>>;

#[derive(StructOpt, Debug)]
#[structopt(name = "homebox-server", about = "Backend for Homebox")]
struct Opt {
    #[structopt(short, long, parse(try_from_str))]
    /// Listening port, format address:port
    address: Option<SocketAddr>,
    #[structopt(short, long, parse(try_from_str), default_value = "config.yaml")]
    /// Path to the config file
    config: PathBuf,
    #[structopt(short, long, parse(try_from_str))]
    /// Path to the file database
    file_database: Option<PathBuf>,
    /// sqlite URL to the metadata database
    metadata_database: Option<String>,
}

fn get_secret_key(store: impl AsRef<Path>) -> std::io::Result<Key> {
    if let Ok(mut file) = File::open(&store) {
        let mut buffer = Vec::new();
        let len = file.read_to_end(&mut buffer)?;
        if len == 64 {
            return Ok(Key::from(&buffer));
        }
    }
    log::warn!("Unable to read cookie master key, generating a new one.");
    let key = Key::generate();
    let mut file = File::create(store)?;
    file.write_all(key.master())?;
    file.sync_all()?;
    Ok(key)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    let (config, logging_config) = match Config::parse(&opt.config) {
        Err(err) => {
            eprintln!("Error in config file `{}`: {}", opt.config.display(), err);
            std::process::exit(-1);
        }
        Ok(config) => config,
    };

    if let Err(e) = log4rs::init_config(logging_config) {
        eprintln!("log4rs: {}", e);
        std::process::exit(-1);
    }

    let file_database_path = opt.file_database.unwrap_or_else(|| {
        config
            .database
            .file
            .parse()
            .expect("Failed parsing file database file name from config file")
    });
    let file_db =
        Arc::new(FileDatabase::open_default(file_database_path).expect("Failed opening database"));

    let metadata_database_path = opt
        .metadata_database
        .as_deref()
        .unwrap_or(&config.database.metadata);
    let mut metadata_db = SqliteConnectOptions::from_str(metadata_database_path)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true)
        .connect()
        .await
        .unwrap();
    sqlx::migrate!()
        .run(&mut metadata_db)
        .await
        .expect("Failed applying sqlite migrations");
    let metadata_db = Arc::new(Mutex::new(metadata_db));

    let schema = Schema::build(schema::QueryRoot, schema::MutationRoot, EmptySubscription)
        .data(metadata_db.clone())
        .finish();

    let config = Arc::new(config);
    let inner_config = config.clone();
    let cookie_key = get_secret_key(&config.auth.cookie_storage)?;
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(schema.clone()))
            .app_data(Data::new(file_db.clone()))
            .app_data(Data::new(inner_config.clone()))
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), cookie_key.clone())
                    .cookie_secure(false)
                    .cookie_path("/".to_owned())
                    .cookie_http_only(true)
                    .cookie_same_site(SameSite::Strict)
                    .session_lifecycle(BrowserSession::default().state_ttl(Duration::days(1)))
                    .build(),
            )
            .service(user_session::login)
            .service(user_session::logout)
            .service(playground)
            .service(gql)
            .service(gql_sdl)
            .service(images::upload_container_image)
            .service(images::fetch_container_image)
            .service(images::delete_container_image)
            .service(images::upload_item_image)
            .service(images::fetch_item_image)
            .service(images::delete_item_image)
            .service(images::barcode_container)
            .service(images::barcode_item)
    })
    .bind(
        opt.address
            .or(config.server.address)
            .unwrap_or_else(|| "127.0.0.1:3000".parse().unwrap()),
    )?
    .run()
    .await
}

#[get("/")]
pub async fn playground(session: Session, db: web::Data<Arc<FileDatabase>>) -> HttpResponse {
    user_session::verify(&session, &db)
        .err()
        .unwrap_or_else(|| {
            HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(graphiql_source("/api/v1", None))
        })
}

#[post("/api/v1")]
pub async fn gql(
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    schema: web::Data<schema::HomeboxSchema>,
    req: GraphQLRequest,
    actix_req: actix_web::HttpRequest,
) -> HttpResponse {
    if let Err(response) = user_session::verify(&session, &db) {
        response
    } else {
        GraphQLResponse::from(schema.execute(req.into_inner()).await).respond_to(&actix_req)
    }
}

#[get("/sdl")]
pub async fn gql_sdl(schema: web::Data<schema::HomeboxSchema>) -> HttpResponse {
    HttpResponse::Ok()
        .append_header(("Content-type", "text/plain"))
        .body(schema.sdl())
}
