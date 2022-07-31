use std::sync::Arc;

use actix_session::Session;
use actix_web::{error::ErrorInternalServerError, post, web, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::{config::Config, schema::SESSION_TYPE, FileDatabase};

#[derive(Deserialize)]
pub struct LoginFormData {
    pub password: String,
}

// fetch("/login", { method: "POST", body: new URLSearchParams({ password: "..." }) })
#[post("/login")]
pub async fn login(
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
    config: web::Data<Arc<Config>>,
    form: web::Form<LoginFormData>,
) -> Result<HttpResponse, actix_web::Error> {
    // this is not really secure, since it's O(n) with n = first difference
    if form.password == config.auth.password {
        let token = Uuid::new_v4();
        let key: Vec<u8> = std::iter::once(SESSION_TYPE)
            .chain(token.as_bytes().iter().copied())
            .collect();

        db.put(key, &[]).map_err(ErrorInternalServerError)?;
        session.insert("auth", token)?;
        Ok(HttpResponse::Ok().body("OK"))
    } else {
        Ok(HttpResponse::Unauthorized().body("Invalid password"))
    }
}

#[post("/logout")]
pub async fn logout(
    session: Session,
    db: web::Data<Arc<FileDatabase>>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(token) = session.get::<Uuid>("auth").ok().flatten() {
        let key: Vec<u8> = std::iter::once(SESSION_TYPE)
            .chain(token.as_bytes().iter().copied())
            .collect();
        db.delete(key).ok();
        session.purge();
    }
    Ok(HttpResponse::Ok().body("OK"))
}

pub fn verify(session: &Session, db: &FileDatabase) -> Result<(), HttpResponse> {
    let verified = if let Some(token) = session.get::<Uuid>("auth").ok().flatten() {
        let key: Vec<u8> = std::iter::once(SESSION_TYPE)
            .chain(token.as_bytes().iter().copied())
            .collect();
        db.key_may_exist(&key) && db.get(key).ok().flatten().is_some()
    } else {
        false
    };
    if verified {
        Ok(())
    } else {
        Err(HttpResponse::Unauthorized()
            .content_type("text/html")
            .body(include_str!("../login.html")))
    }
}
