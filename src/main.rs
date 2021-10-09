use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use actix_web::{
    delete,
    error::{ErrorBadRequest, ErrorInternalServerError},
    get,
    http::HeaderValue,
    post, web, App, HttpRequest, HttpResponse, HttpServer,
};
use async_graphql::{futures_util::StreamExt, http::graphiql_source, EmptySubscription, Schema};
use async_graphql_actix_web::{Request, Response};
use structopt::StructOpt;

mod config;
use config::Config;
use uuid::Uuid;
mod schema;
use schema::{CONTAINER_IMAGE_TYPE, ITEM_IMAGE_TYPE};

pub type Database = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;

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
    /// Path to the database file
    database: Option<PathBuf>,
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

    let database_path = opt.database.unwrap_or_else(|| {
        config
            .database
            .file
            .parse()
            .expect("Failed parsing database file name from config file")
    });
    let db = Arc::new(Database::open_default(database_path).expect("Failed opening database"));

    let schema = Schema::build(schema::QueryRoot, schema::MutationRoot, EmptySubscription)
        .data(db.clone())
        .finish();

    HttpServer::new(move || {
        App::new()
            .data(schema.clone())
            .data(db.clone())
            .service(playground)
            .service(gql)
            .service(upload_container_image)
            .service(fetch_container_image)
            .service(delete_container_image)
            .service(upload_item_image)
            .service(fetch_item_image)
            .service(delete_item_image)
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
pub async fn playground() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(graphiql_source("/api/v1", None))
}

#[post("/api/v1")]
pub async fn gql(schema: web::Data<schema::HomeboxSchema>, req: Request) -> Response {
    schema.execute(req.into_inner()).await.into()
}

#[post("/image/container/{id}")]
pub async fn upload_container_image(
    db: web::Data<Database>,
    id: web::Path<(String,)>,
    req: HttpRequest,
    mut data: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let uuid = id.into_inner().0.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(CONTAINER_IMAGE_TYPE)
        .chain(uuid.as_bytes().iter().copied())
        .collect();
    if req.headers().get("content-type") != Some(&HeaderValue::from_static("image/jpeg")) {
        return Ok(HttpResponse::BadRequest().body("Invalid content type."));
    }

    let mut bytes = web::BytesMut::new();
    while let Some(item) = data.next().await {
        bytes.extend_from_slice(&item?);
    }

    db.put(key, &bytes).map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body("OK"))
}

#[get("/image/container/{id}")]
pub async fn fetch_container_image(
    db: web::Data<Database>,
    id: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let uuid = id.into_inner().0.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(CONTAINER_IMAGE_TYPE)
        .chain(uuid.as_bytes().iter().copied())
        .collect();

    if let Some(data) = db.get(key).map_err(ErrorInternalServerError)? {
        Ok(HttpResponse::Ok().content_type("image/jpeg").body(data))
    } else {
        Ok(HttpResponse::NotFound().body("No such image"))
    }
}

#[delete("/image/container/{id}")]
pub async fn delete_container_image(
    db: web::Data<Database>,
    id: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let uuid = id.into_inner().0.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(CONTAINER_IMAGE_TYPE)
        .chain(uuid.as_bytes().iter().copied())
        .collect();
    db.delete(key).map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body("OK"))
}

#[post("/image/container/{container_id}/item/{item_id}")]
pub async fn upload_item_image(
    db: web::Data<Database>,
    id: web::Path<(String, String)>,
    req: HttpRequest,
    mut data: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let (container_id, item_id) = id.into_inner();
    let container_uuid = container_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let item_uuid = item_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(ITEM_IMAGE_TYPE)
        .chain(container_uuid.as_bytes().iter().copied())
        .chain(item_uuid.as_bytes().iter().copied())
        .collect();
    if req.headers().get("content-type") != Some(&HeaderValue::from_static("image/jpeg")) {
        return Ok(HttpResponse::BadRequest().body("Invalid content type."));
    }

    let mut bytes = web::BytesMut::new();
    while let Some(item) = data.next().await {
        bytes.extend_from_slice(&item?);
    }

    db.put(key, &bytes).map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body("OK"))
}

#[get("/image/container/{container_id}/item/{item_id}")]
pub async fn fetch_item_image(
    db: web::Data<Database>,
    id: web::Path<(String, String)>,
) -> Result<HttpResponse, actix_web::Error> {
    let (container_id, item_id) = id.into_inner();
    let container_uuid = container_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let item_uuid = item_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(ITEM_IMAGE_TYPE)
        .chain(container_uuid.as_bytes().iter().copied())
        .chain(item_uuid.as_bytes().iter().copied())
        .collect();

    if let Some(data) = db.get(key).map_err(ErrorInternalServerError)? {
        Ok(HttpResponse::Ok().content_type("image/jpeg").body(data))
    } else {
        Ok(HttpResponse::NotFound().body("No such image"))
    }
}

#[delete("/image/container/{container_id}/item/{item_id}")]
pub async fn delete_item_image(
    db: web::Data<Database>,
    id: web::Path<(String, String)>,
) -> Result<HttpResponse, actix_web::Error> {
    let (container_id, item_id) = id.into_inner();
    let container_uuid = container_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let item_uuid = item_id.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(ITEM_IMAGE_TYPE)
        .chain(container_uuid.as_bytes().iter().copied())
        .chain(item_uuid.as_bytes().iter().copied())
        .collect();
    db.delete(key).map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body("OK"))
}
