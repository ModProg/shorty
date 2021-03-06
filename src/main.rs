use rand::distributions::Alphanumeric;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use rand::Rng;
use rocket::fairing::AdHoc;
use rocket::http::CookieJar;
use rocket::request::{FromRequest, Outcome};
use rocket::response::Redirect;
use rocket::{Build, Rocket, State};

use rocket::response::status;
use rocket_sync_db_pools::rusqlite::{self, params};

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate rocket_sync_db_pools;

mod wordlists;

#[database("redirects")]
struct Redirects(rusqlite::Connection);

struct Authenticated;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Authenticated {
    type Error = &'static str;

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let config: &State<Config> = request.guard().await.unwrap();
        let jar: &CookieJar<'_> = request.guard().await.unwrap();
        match (
            config.password.as_deref(),
            jar.get("PASSWORD").map(|c| c.value()),
        ) {
            (config_password, cookie_password) if config_password == cookie_password => {
                Outcome::Success(Self)
            }
            (None, _) => Outcome::Failure((
                rocket::http::Status::Unauthorized,
                "The cookie `PASSWORD` was not set, but a password is required.",
            )),
            _ => Outcome::Failure((
                rocket::http::Status::Unauthorized,
                "The wrong password was provided through the cookie `PASSWORD`",
            )),
        }
    }
}

#[delete("/<ident>")]
async fn delete(_a: Authenticated, db: Redirects, ident: String) {
    db.run(move |conn| conn.execute("DELETE FROM redirects WHERE ident = ?1", params![ident]))
        .await
        .expect("Delete cannot fail");
}

#[get("/<ident>")]
async fn get(db: Redirects, ident: String) -> Result<Redirect, status::NotFound<String>> {
    match db
        .run(move |conn| {
            conn.query_row::<String, _, _>(
                "SELECT target FROM redirects WHERE ident = ?1",
                params![ident],
                |r| r.get(0),
            )
        })
        .await
    {
        Ok(url) => Ok(Redirect::temporary(url)),
        Err(_) => Err(status::NotFound(
            "There is no shortened url to open here.".to_string(),
        )),
    }
}

#[post("/w", data = "<target>")]
async fn word_list_post(
    _a: Authenticated,
    config: &State<Config>,
    db: Redirects,
    target: String,
) -> String {
    let length = config.worded_length;
    let ident = db
        .run(move |conn| loop {
            let ident = gen_worded_ident(length);
            if conn
                .execute(
                    "INSERT INTO redirects (ident, target) VALUES (?1, ?2)",
                    params![ident, target],
                )
                .is_ok()
            {
                break ident;
            }
        })
        .await;

    format!("{}{}", config.base_url, ident)
}

#[post("/c", data = "<target>")]
async fn chared_post(
    _a: Authenticated,
    config: &State<Config>,
    db: Redirects,
    target: String,
) -> String {
    let length = config.chared_length;
    let ident = db
        .run(move |conn| loop {
            let ident = gen_chared_ident(length);
            if conn
                .execute(
                    "INSERT INTO redirects (ident, target) VALUES (?1, ?2)",
                    params![ident, target],
                )
                .is_ok()
            {
                break ident;
            }
        })
        .await;
    format!("{}{}", config.base_url, ident)
}

fn gen_worded_ident(length: usize) -> String {
    wordlists::ENG
        .choose_multiple(&mut rand::thread_rng(), length)
        .map(|f| format!("{}{}", &f[0..1].to_uppercase(), &f[1..]))
        .collect()
}

fn gen_chared_ident(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

async fn init_db(rocket: Rocket<Build>) -> Rocket<Build> {
    Redirects::get_one(&rocket)
        .await
        .expect("database mounted")
        .run(|conn| {
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS redirects (
                    ident VARCHAR PRIMARY KEY,
                    target VARCHAR NOT NULL
                )"#,
                params![],
            )
        })
        .await
        .expect("can init rusqlite DB");

    rocket
}

struct Config {
    chared_length: usize,
    worded_length: usize,
    base_url: String,
    password: Option<String>,
}

#[launch]
fn rocket() -> _ {
    let chared_length = std::env::var("CHARED_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let worded_length = std::env::var("WORDED_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let base_url = std::env::var("BASE_URL")
        .map(|url| if url.ends_with('/') { url } else { url + "/" })
        .unwrap_or_else(|_| "http://127.0.0.1:8000/".to_string());
    let password = std::env::var("PASSWORD").ok();

    rocket::build()
        .attach(Redirects::fairing())
        .attach(AdHoc::on_ignite("Rusqlite Init", init_db))
        .manage(Config {
            chared_length,
            worded_length,
            base_url,
            password,
        })
        .mount("/", routes![get, delete, word_list_post, chared_post])
}
