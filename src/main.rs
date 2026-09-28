mod agent;
mod config;
mod dao;
mod interaction;
mod prompt;
mod provider;
mod repl;
mod session;
mod tools;
mod trace;
mod ui;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ui::run().await
}
