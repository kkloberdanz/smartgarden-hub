/*
 *  This file is part of SmartGarden.
 *
 *  SmartGarden is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  SmartGarden is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with SmartGarden.  If not, see <https://www.gnu.org/licenses/>.
 */


#![feature(proc_macro_hygiene, decl_macro)]


#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;


#[cfg(test)] mod tests;


use std::sync::Mutex;
use rocket::{Rocket, State};
use rocket_contrib::json::Json;
use rusqlite::Connection;
use rusqlite::types::ToSql;


type SensorID = i64;
type DbConn = Mutex<Connection>;


#[derive(Serialize, Deserialize, Debug)]
struct GardenData {
    sensor_id: SensorID,
    moisture_content: i8
}


fn http_ok(msg: &String) -> String {
    format!("HTTP/1.1 200 OK \r\n\r\n{}\r\n", msg)
}


fn http_bad_request(msg: &String) -> String {
    format!("HTTP/1.1 400 BAD REQUEST \r\n\r\n{}\r\n", msg)
}


fn get_latest_moisture(db_conn: &State<DbConn>, sensor_id: SensorID) -> i8 {
    let sql = "select moisture_content \
               from garden_data \
               where sensor_id = ?1 and \
                     time = (select max(time) \
                             from garden_data \
                             where sensor_id = ?1)";
    let params = [&sensor_id as &ToSql];
    db_conn
        .lock()
        .expect("db read lock")
        .query_row(&sql, &params, |row| row.get(0))
        .unwrap()
}


fn should_water(db_conn: &State<DbConn>, sensor_id: SensorID) -> bool {
    let moisture_content = get_latest_moisture(&db_conn, sensor_id);
    moisture_content < 20
}


#[get("/can-i-water/<sensor_id>")]
fn can_i_water(db_conn: State<DbConn>, sensor_id: SensorID) -> String {
    if should_water(&db_conn, sensor_id) {
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
    if data.moisture_content > 100 || data.moisture_content < 0 {
        let msg = String::from(
            "moisture_content must be an integer between 0 to 100");
        http_bad_request(&msg)
    } else {
        let msg = format!(
            "sensor #{} has moister content {}", data.sensor_id,
                                                 data.moisture_content);
        let sql = "insert into garden_data (sensor_id, moisture_content) \
                   values(?1, ?2)";
        let params = [&data.sensor_id as &ToSql, &data.moisture_content];
        db_conn
            .lock()
            .expect("db conn lock")
            .execute(&sql, &params).unwrap();
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
