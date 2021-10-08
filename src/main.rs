use std::{net::SocketAddr, path::PathBuf};

use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use async_graphql::{http::graphiql_source, EmptySubscription, Schema};
use async_graphql_actix_web::{Request, Response};
use structopt::StructOpt;

mod config;
use config::Config;
mod schema;

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
    let db = Database::open_default(database_path).expect("Failed opening database");

    let schema = Schema::build(schema::QueryRoot, schema::MutationRoot, EmptySubscription)
        .data(db)
        .finish();

    HttpServer::new(move || {
        App::new()
            .data(schema.clone())
            .service(playground)
            .service(gql)
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
