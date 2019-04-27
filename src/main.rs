#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;


#[cfg(test)] mod tests;


use rocket_contrib::json::Json;
use rocket_contrib::databases::diesel;


#[database("sqlite_db")]
struct DbConn(diesel::SqliteConnection);


type SensorID = u64;


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
fn log(conn: DbConn, data: Json<GardenData>) -> String {
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


fn main() {
    rocket::ignite()
        .mount("/", routes![hello, log, can_i_water])
        .attach(DbConn::fairing())
        .launch();
}
