mod agent;
mod config;
mod dao;
mod prompt;
mod provider;
mod repl;
mod tools;
mod trace;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    agent::run().await
}
