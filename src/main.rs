use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use rocket::fairing::AdHoc;
use rocket::response::Redirect;
use rocket::Build;
use rocket::Rocket;
use rocket::State;

use rocket_sync_db_pools::rusqlite::{self, params};

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate rocket_sync_db_pools;

#[database("redirects")]
struct Redirects(rusqlite::Connection);

#[derive(Debug, Clone)]
struct UrlRedirect {
    ident: String,
    target: String,
}

#[get("/<ident>")]
async fn get(db: Redirects, ident: String) -> Redirect {
    let target: String = db
        .run(move |conn| {
            conn.query_row(
                "SELECT target FROM redirects WHERE ident = ?1",
                params![ident],
                |r| r.get(0),
            )
        })
        .await
        .ok()
        .unwrap();
    Redirect::temporary(target)
}

#[post("/w", data = "<target>")]
async fn word_list_post(config: &State<Config>, db: Redirects, target: String) -> String {
    let length = config.worded_length;
    db.run(move |conn| {
        let ident = gen_worded_ident(length);
        conn.execute(
            "INSERT INTO redirects (ident, target) VALUES (?1, ?2)",
            params![ident, target],
        )
        .unwrap();
        ident
    })
    .await
}

#[post("/c", data = "<target>")]
async fn chared_post(config: &State<Config>, db: Redirects, target: String) -> String {
    let length = config.chared_length;
    db.run(move |conn| {
        let ident = gen_chared_ident(length);
        conn.execute(
            "INSERT INTO redirects (ident, target) VALUES (?1, ?2)",
            params![ident, target],
        )
        .unwrap();
        ident
    })
    .await
}

fn gen_worded_ident(length: usize) -> String {
    // Fallback for now
    gen_chared_ident(length)
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
}

#[launch]
fn rocket() -> _ {
    let chared_length = std::env::var("CHARED_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let worded_length = std::env::var("CHARED_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);

    rocket::build()
        .attach(Redirects::fairing())
        .attach(AdHoc::on_ignite("Rusqlite Init", init_db))
        .manage(Config {
            chared_length,
            worded_length,
        })
        .mount("/", routes![get, word_list_post, chared_post])
}
