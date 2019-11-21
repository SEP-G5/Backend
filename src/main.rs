#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

mod blockchain;
mod rest;

fn main() {
    println!("~ Bike Blockchain Backend ~");
    rest::server::run_server();
}
