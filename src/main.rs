mod config;
mod discord;
mod logger;
mod minecraft;
mod network;
mod scanner;
mod stats;

use crate::logger::setup_environment;

use crate::scanner::run_scanner;

#[tokio::main]
async fn main() {
    setup_environment();
    run_scanner().await;
}
