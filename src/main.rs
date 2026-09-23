mod agent;
mod config;
mod prompt;
mod provider;
mod repl;
mod tools;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    agent::run().await
}
