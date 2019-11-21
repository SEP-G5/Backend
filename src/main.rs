#![feature(proc_macro_hygiene, decl_macro)]

mod blockchain;
mod rest;

fn main() {
    println!("~ Bike Blockchain Backend ~");
    rest::server::run_server();
}
