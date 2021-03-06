use crate::doctor::Doctor;
use crate::error::AppError;
use crate::tickets::Ticket;
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

#[get("/api/v2/me")]
async fn get_me(db_pool: web::Data<Pool>, session: Session) -> Result<HttpResponse, AppError> {
    let id: Option<i32> = session.get("azap")?;
    let location: Option<i32> = session.get("azap-location")?;
    let doctor: Option<i32> = session.get("azap-doctor")?;

    let db_conn = db_pool.get().await?;
    let doctors: Vec<Doctor> = if location.is_some() {
        db_conn
            .query("SELECT * from doctors where location_id=$1", &[&location])
            .await?
            .iter()
            .map(|row| row.into())
            .collect()
    } else {
        vec![]
    };

    let tickets: Vec<Ticket> = if location.is_some() && doctor.is_some() {
        db_conn
            .query(
                "SELECT * from tickets where location_id=$1 and doctor_id=$2",
                &[&location.unwrap(), &doctor.unwrap()],
            )
            .await?
            .iter()
            .map(|row| row.into())
            .collect()
    } else if location.is_some() && doctor.is_none() {
        db_conn
            .query(
                "SELECT * from tickets where location_id=$1",
                &[&location.unwrap()],
            )
            .await?
            .iter()
            .map(|row| row.into())
            .collect()
    } else {
        vec![]
    };

    if id.is_some() {
        Ok(HttpResponse::Ok().json(json!({
                "status": "sucess",
                "id": id,
                "locationId":location,
                "doctorId":doctor,
                "doctors":doctors,
                "tickets":tickets
        })))
    } else {
        Ok(HttpResponse::Unauthorized()
            .json(json!({ "status": "error", "error":"invalid session" })))
    }
}

#[derive(Deserialize)]
struct Identity {
    secret: String,
    id: i32,
}

#[post("/api/v2/login")]
async fn login(
    user: web::Json<Identity>,
    db_pool: web::Data<Pool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let user = user.into_inner();
    let id = user.id;

    let db_conn = db_pool.get().await?;
    let row = db_conn
        .query_one("SELECT id,hsecret from users where id=$1", &[&id])
        .await?;
    let id: i32 = row.get("id");
    let hsecret: String = row.get("hsecret");
    if scrypt_check(&user.secret, &hsecret).is_ok() {
        session.set("azap", id)?;
        //check if is location
        let res = db_conn
            .query("SELECT id from locations where id=$1", &[&id])
            .await?;
        if !res.is_empty() {
            let id: i32 = res[0].get("id");
            session.set("azap-location", id)?;
        }
        // check if is doctor
        let res = db_conn
            .query("SELECT id from doctors where id=$1", &[&id])
            .await?;
        if !res.is_empty() {
            let id: i32 = res[0].get("id");
            session.set("azap-doctor", id)?;
        }
        session.renew();
        return Ok(HttpResponse::Ok().json(json!({"status": "sucess","id": id})));
    }
    session.purge();
    Ok(HttpResponse::Unauthorized().json(json!({"error": "error"})))
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
