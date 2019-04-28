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
extern crate reqwest;
extern crate serde_json;


#[cfg(test)] mod tests;


use std::sync::Mutex;
use rocket::{Rocket, State};
use rocket_contrib::json::Json;
use rusqlite::Connection;
use rusqlite::types::ToSql;
use rocket::http::Method;


type SensorID = i64;
type DbConn = Mutex<Connection>;


#[derive(Serialize, Deserialize, Debug)]
struct GardenData {
    sensor_id: SensorID,
    moisture_content: i8
}


#[derive(Debug)]
struct Forecast {
    country: String,
    city: String,
    time: String,
    weather: String,
    description: String,
    temp: f64,
    temp_min: f64,
    temp_max: f64,
    pressure: f64,
    humidity: f64
}


#[derive(Debug)]
struct SensorMeta {
    sensor_id: SensorID,
    country: String,
    city: String
}


fn http_ok(msg: &String) -> String {
    format!("HTTP/1.1 200 OK \r\n\r\n{}\r\n", msg)
}


fn http_bad_request(msg: &String) -> String {
    format!("HTTP/1.1 400 BAD REQUEST \r\n\r\n{}\r\n", msg)
}


fn get_latest_garden_record(db_conn: &State<DbConn>,
                            sensor_id: SensorID) -> rusqlite::Result<GardenData> {
    let sql = "select sensor_id, moisture_content \
               from garden_data \
               where sensor_id = ?1 and \
                     time = (select max(time) \
                             from garden_data \
                             where sensor_id = ?1)";
    let params = [&sensor_id as &ToSql];
    db_conn
        .lock()
        .expect("db read lock")
        .query_row(&sql, &params, |row| Ok(
                GardenData {
                    sensor_id: row.get(0)?,
                    moisture_content: row.get(1)?,
                }))
}


fn wont_rain_soon(db_conn: &State<DbConn>,
                  sensor_id: SensorID) -> bool {
    true
}


fn should_water(db_conn: &State<DbConn>,
                sensor_id: SensorID) -> Result<bool, String> {
    let garden_record = get_latest_garden_record(&db_conn, sensor_id);
    match garden_record {
        Ok(v) => Ok((v.moisture_content < 20) &&
                    (wont_rain_soon(&db_conn, sensor_id))),
        Err(e) => {
            println!("error: {}", e);
            Err(format!("sensor_id: {} does not exist", sensor_id))
        }
    }
}


fn just_unwrap_string(string: Option<&str>) -> String {
    match string {
        Some(s) => String::from(s),
        None => String::from("")
    }
}


fn just_unwrap_f64(myf: Option<f64>) -> f64 {
    match myf {
        Some(f) => f,
        None => 0.0
    }
}


//fn fetch_forecast(db_conn: &State<DbConn>) -> Result<Vec<Forecast>, String> {
#[get("/forecast")]
fn fetch_forecast(db_conn: State<DbConn>) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = "https://api.openweathermap.org\
               /data/2.5/forecast?q=Urbandale,US";
    let mut response = client.get(url)
        .header("x-api-key", "2fdc482d5509bc0866f5b3824454044a")
        .send()
        .unwrap();
    let json_data: serde_json::Value = response.json().unwrap();

    let list = match &json_data["list"] {
        serde_json::Value::Array(v) => v.to_vec(),
        _ => panic!("weather API is broken")
    };

    let sql = "insert into forecast \
               (country, city, time, weather, description, temp, temp_min, \
                temp_max, pressure, humidity) \
               values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
    for event in list {
        let forecast = Forecast {
            country: String::from("US"),
            city: String::from("Urbandale"),
            time: just_unwrap_string(event["dt_txt"].as_str()),
            weather: just_unwrap_string(event["weather"][0]["main"].as_str()),
            description: just_unwrap_string(
                event["weather"][0]["description"].as_str()),
            temp: just_unwrap_f64(event["main"]["temp"].as_f64()),
            temp_min: just_unwrap_f64(event["main"]["temp_min"].as_f64()),
            temp_max: just_unwrap_f64(event["main"]["temp_max"].as_f64()),
            pressure: just_unwrap_f64(event["main"]["pressure"].as_f64()),
            humidity: just_unwrap_f64(event["main"]["humidity"].as_f64())
        };
        let params = [
            &forecast.country as &ToSql,
            &forecast.city,
            &forecast.time,
            &forecast.weather,
            &forecast.description,
            &forecast.temp,
            &forecast.temp_min,
            &forecast.temp_max,
            &forecast.pressure,
            &forecast.humidity,
        ];
        db_conn
            .lock()
            .expect("db conn lock inserting forecast")
            .execute(&sql, &params).unwrap();
        println!("{:?}", forecast);
    }

    Ok(format!("{:?}\n", "foobar"))
}


#[get("/can-i-water/<sensor_id>")]
fn can_i_water(db_conn: State<DbConn>, sensor_id: SensorID) -> String {
    match should_water(&db_conn, sensor_id) {
        Ok(b) => {
            if b {
                http_ok(&String::from("yes"))
            } else {
                http_ok(&String::from("no"))
            }
        }
        Err(e) => http_bad_request(&e)
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
        .mount("/", routes![hello, log, can_i_water, fetch_forecast])
}


fn main() {
    rocket().launch();
}
