#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;


#[cfg(test)] mod tests;


use std::sync::Mutex;
use rocket::{Rocket, State};
use rocket_contrib::json::Json;
use rusqlite::types::ToSql;
use rusqlite::{Connection, Error, Result, NO_PARAMS};


type SensorID = u64;
type DbConn = Mutex<Connection>;


#[derive(Serialize, Deserialize, Debug)]
struct GardenData {
    sensor_id: SensorID,
    moisture_content: u8
}


fn http_ok(msg: &String) -> String {
    format!("HTTP/1.1 200 OK \r\n\r\n{}\r\n", msg)
}


fn http_bad_request(msg: &String) -> String {
    format!("HTTP/1.1 400 BAD REQUEST \r\n\r\n{}\r\n", msg)
}


#[get("/can-i-water/<sensor_id>")]
fn can_i_water(sensor_id: SensorID) -> String {
    if true {
        http_ok(&String::from("yes"))
    } else {
        http_ok(&String::from("no"))
    }
}


#[get("/")]
fn hello() -> String {
    let msg = String::from("Welcome to KloverTech SmartGarden");
    http_ok(&msg)
}


#[post("/log", format="Application/json", data="<data>")]
fn log(db_conn: State<DbConn>, data: Json<GardenData>) -> String {
    if data.moisture_content > 100 {
        let msg = String::from(
            "moisture_content must be an integer between 0 to 100");
        http_bad_request(&msg)
    } else {
        let msg = format!(
            "sensor #{} has moister content {}", data.sensor_id,
                                                 data.moisture_content);
        http_ok(&msg)
    }
}


fn rocket() -> Rocket {
    let conn = Connection::open("db.sqlite")
        .expect("failed to open db.sqlite file");

    rocket::ignite()
        .manage(Mutex::new(conn))
        .mount("/", routes![hello, log, can_i_water])
}


fn main() {
    rocket().launch();
}
