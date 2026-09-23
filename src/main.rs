mod config;
mod provider;
mod repl;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    repl::run().await
}
