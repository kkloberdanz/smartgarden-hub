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

extern crate chrono;
extern crate time;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
extern crate reqwest;
extern crate serde_json;

use chrono::Local;
use rocket::{Rocket, State};
use rocket_contrib::json::Json;
use rusqlite::types::ToSql;
use rusqlite::Connection;
use std::sync::Mutex;
use std::thread;
use time::Duration;

type SensorID = i64;
type DbConn = Mutex<Connection>;

enum MoistureLevel {
    Plenty,
    Low,
    Critical,
}

#[derive(Serialize, Deserialize, Debug)]
struct GardenData {
    sensor_id: SensorID,
    moisture_content: i8,
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
    humidity: f64,
}

#[derive(Debug)]
struct SensorMeta {
    sensor_id: SensorID,
    country: String,
    city: String,
}

fn http_ok(msg: &String) -> String {
    format!("HTTP/1.1 200 OK \r\n\r\n{}\r\n", msg)
}

fn http_bad_request(msg: &String) -> String {
    format!("HTTP/1.1 400 BAD REQUEST \r\n\r\n{}\r\n", msg)
}

fn get_latest_garden_record(
    db_conn: &State<DbConn>,
    sensor_id: SensorID,
) -> rusqlite::Result<GardenData> {
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
        .query_row(&sql, &params, |row| {
            Ok(GardenData {
                sensor_id: row.get(0)?,
                moisture_content: row.get(1)?,
            })
        })
}

fn wont_rain_soon(
    db_conn: &State<DbConn>,
    sensor_id: SensorID,
) -> rusqlite::Result<bool> {
    let now = Local::now();
    let twelve_hr_later = now + Duration::hours(12);
    let now_string = format!("{}", now.format("%Y-%m-%d %H:%M:%S"));
    println!("now {}", now_string);
    let twelve_hr_later_string =
        format!("{}", twelve_hr_later.format("%Y-%m-%d %H:%M:%S"));
    let sql =
        "select count(*) from forecast \
         where timeof_forcast = (select max(timeof_forcast) from forecast) \
         and time >= ?1 and time <= ?2 and lower(weather) like '%rain%'";

    let params = [&now_string as &ToSql, &twelve_hr_later_string];
    db_conn
        .lock()
        .expect("db read lock")
        .query_row(&sql, &params, |row| {
            let count: i32 = row.get(0)?;
            Ok(count == 0)
        })
}

fn describe_moisture(garden_record: &GardenData) -> MoistureLevel {
    if garden_record.moisture_content > 25 {
        MoistureLevel::Plenty
    } else if garden_record.moisture_content > 10 {
        MoistureLevel::Low
    } else {
        MoistureLevel::Critical
    }
}

fn check_water(
    db_conn: &State<DbConn>,
    garden_record: &GardenData,
) -> Result<bool, String> {
    let moisture_level = describe_moisture(&garden_record);
    let sensor_id = garden_record.sensor_id;
    match moisture_level {
        MoistureLevel::Plenty => {
            println!("plenty of water");
            Ok(false)
        }
        MoistureLevel::Low => {
            println!("low water");
            match wont_rain_soon(&db_conn, sensor_id) {
                Ok(no_rain) => {
                    if no_rain {
                        println!("won't rain soon");
                    } else {
                        println!("but it will rain soon");
                    }
                    Ok(no_rain)
                }
                Err(e) => return Err(format!("{}", e)),
            }
        }
        MoistureLevel::Critical => {
            println!("moisture level critical");
            Ok(true)
        }
    }
}

fn should_water(
    db_conn: &State<DbConn>,
    sensor_id: SensorID,
) -> Result<bool, String> {
    let garden_record = get_latest_garden_record(&db_conn, sensor_id);
    match garden_record {
        Ok(v) => check_water(&db_conn, &v),
        Err(e) => {
            println!("error: {}", e);
            Err(format!("no records for sensor_id: {}", sensor_id))
        }
    }
}

fn fetch_forecast(db_conn: &Connection) {
    let client = reqwest::Client::new();
    let url = "https://api.openweathermap.org\
               /data/2.5/forecast?q=Urbandale,US";
    let mut response = client
        .get(url)
        .header("x-api-key", "2fdc482d5509bc0866f5b3824454044a")
        .send()
        .unwrap();
    let json_data: serde_json::Value = response.json().unwrap();

    let list = match &json_data["list"] {
        serde_json::Value::Array(v) => v.to_vec(),
        _ => panic!("weather API is broken"),
    };

    let sql = "insert into forecast \
               (timeof_forcast, country, city, time, weather, \
               description, temp, temp_min, \
               temp_max, pressure, humidity) \
               values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
    for event in list {
        let forecast = Forecast {
            country: String::from("US"),
            city: String::from("Urbandale"),
            time: event["dt_txt"].as_str().unwrap().to_string(),
            weather: event["weather"][0]["main"].as_str().unwrap().to_string(),
            description: event["weather"][0]["description"]
                .as_str()
                .unwrap()
                .to_string(),
            temp: event["main"]["temp"].as_f64().unwrap(),
            temp_min: event["main"]["temp_min"].as_f64().unwrap(),
            temp_max: event["main"]["temp_max"].as_f64().unwrap(),
            pressure: event["main"]["pressure"].as_f64().unwrap(),
            humidity: event["main"]["humidity"].as_f64().unwrap(),
        };
        let now = Local::now();
        let now_string = format!("{}", now.format("%Y-%m-%d %H:%M:%S"));
        let params = [
            &now_string as &ToSql,
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
        db_conn.execute(&sql, &params).unwrap();
        println!("{:?}", forecast);
    }
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
        Err(e) => http_bad_request(&e),
    }
}

#[get("/")]
fn hello() -> String {
    let msg = String::from("Welcome to KloverTech SmartGarden");
    http_ok(&msg)
}

#[post("/log", format = "Application/json", data = "<data>")]
fn log(db_conn: State<DbConn>, data: Json<GardenData>) -> String {
    if data.moisture_content > 100 || data.moisture_content < 0 {
        let msg = String::from(
            "moisture_content must be an \
             integer between 0 to 100",
        );
        http_bad_request(&msg)
    } else {
        let msg = format!(
            "sensor #{} has moister content {}",
            data.sensor_id, data.moisture_content
        );
        let sql = "insert into garden_data (sensor_id, moisture_content) \
                   values(?1, ?2)";
        let params = [&data.sensor_id as &ToSql, &data.moisture_content];
        db_conn
            .lock()
            .expect("db conn lock")
            .execute(&sql, &params)
            .unwrap();
        http_ok(&msg)
    }
}

fn rocket() -> Rocket {
    let conn =
        Connection::open("db.sqlite").expect("failed to open db.sqlite file");

    rocket::ignite()
        .manage(Mutex::new(conn))
        .mount("/", routes![hello, log, can_i_water])
}

fn echo_thread() -> ! {
    println!("fetch_forecast thread active");
    let conn =
        Connection::open("db.sqlite").expect("failed to open db.sqlite file");
    loop {
        thread::sleep(std::time::Duration::from_secs(10800));
        println!("fetching forecast");
        fetch_forecast(&conn);
    }
}

fn main() {
    thread::spawn(move || echo_thread());
    rocket().launch();
}
