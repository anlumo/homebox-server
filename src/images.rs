use std::sync::Arc;

use actix_session::Session;
use actix_web::{
    delete,
    error::{ErrorBadRequest, ErrorInternalServerError},
    get,
    http::HeaderValue,
    post, web, HttpRequest, HttpResponse,
};
use async_graphql::futures_util::StreamExt;
use uuid::Uuid;

use crate::{
    schema::{CONTAINER_IMAGE_TYPE, ITEM_IMAGE_TYPE},
    user_session, FileDatabase,
};

#[post("/image/container/{id}")]
pub async fn upload_container_image(
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String,)>,
    req: HttpRequest,
    mut data: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
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
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
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
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
    let uuid = id.into_inner().0.parse::<Uuid>().map_err(ErrorBadRequest)?;
    let key: Vec<u8> = std::iter::once(CONTAINER_IMAGE_TYPE)
        .chain(uuid.as_bytes().iter().copied())
        .collect();
    db.delete(key).map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body("OK"))
}

#[post("/image/container/{container_id}/item/{item_id}")]
pub async fn upload_item_image(
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String, String)>,
    req: HttpRequest,
    mut data: web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
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
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String, String)>,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
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
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    id: web::Path<(String, String)>,
) -> Result<HttpResponse, actix_web::Error> {
    user_session::verify(&session, &db)?;
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
