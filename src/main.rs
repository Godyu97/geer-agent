mod agent;
mod config;
mod dao;
mod interaction;
mod prompt;
mod provider;
mod session;
mod tools;
mod trace;
mod ui;

#[cfg(all(test, unix))]
#[path = "../tests/support/mod.rs"]
mod test_support;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ui::run()
}
