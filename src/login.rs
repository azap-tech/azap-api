use crate::error::AppError;
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Result};
use deadpool_postgres::Client;
use deadpool_postgres::Pool;
use scrypt::{scrypt_check, scrypt_simple, ScryptParams};
use serde::Deserialize;
use serde_json::json;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(login);
    cfg.service(get_me);
    cfg.service(logout);
}

pub async fn create_user(db_conn: &Client, password: &str) -> Result<i32, AppError> {
    let params = ScryptParams::new(15, 8, 1).unwrap();
    let hashed_password = scrypt_simple(password, &params).expect("OS RNG should not fail");

    let sql = "INSERT into users (hsecret) values ($1) RETURNING id";
    let row = db_conn.query_one(sql, &[&hashed_password]).await?;
    let id: i32 = row.get("id");
    Ok(id)
}

#[get("/api/v1/me")]
async fn get_me(session: Session) -> Result<HttpResponse, AppError> {
    let sess: Option<i32> = session.get("azap")?;
    if sess.is_some() {
        Ok(HttpResponse::Ok().json(json!({ "status": "sucess","user":{"id": sess} })))
    } else {
        Ok(HttpResponse::Unauthorized()
            .json(json!({ "status": "error", "error":"invalide session" })))
    }
}

#[derive(Deserialize)]
struct Identity {
    secret: String,
    id: String,
}

#[post("/api/v2/token")]
async fn login(
    user: web::Json<Identity>,
    db_pool: web::Data<Pool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let user = user.into_inner();

    let db_conn = db_pool.get().await?;
    let row = db_conn
        .query_one("SELECT (id,hsecret) from users where id=$1", &[&id])
        .await?;
    let id: i32 = row.get("id");

    session.set("azap", id)?;
    session.renew();
    Ok(HttpResponse::Ok().json(json!({"status": "sucess","user":{"id": id} })))
}

#[post("/api/v2/logout")]
async fn logout(session: Session) -> Result<HttpResponse, AppError> {
    let id: Option<i32> = session.get("azap")?;
    if id.is_some() {
        session.remove("azap");
        Ok(HttpResponse::Ok().json(json!({ "status": "sucess" })))
    } else {
        Ok(HttpResponse::Ok().json(json!({ "status": "error" })))
    }
}
