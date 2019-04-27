#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_contrib;
#[macro_use] extern crate serde_derive;


#[cfg(test)] mod tests;


use rocket_contrib::json::Json;


#[derive(Serialize, Deserialize, Debug)]
struct GardenData {
    sensor_id: u64,
    moisture_level: u32
}


fn http_ok(msg: &String) -> String {
    format!("HTTP/1.1 200 OK \r\n\r\n{}\r\n", msg)
}


#[get("/")]
fn hello() -> String {
    let msg = String::from("Welcome to KloverTech SmartGarden\n");
    http_ok(&msg)
}


#[post("/log", format="Application/json", data="<data>")]
fn log(data: Json<GardenData>) -> String {
    let msg = format!("sensor #{} has moister level {}", data.sensor_id,
                                                         data.moisture_level);
    http_ok(&msg)
}


fn main() {
    rocket::ignite().mount("/", routes![hello, log]).launch();
}
