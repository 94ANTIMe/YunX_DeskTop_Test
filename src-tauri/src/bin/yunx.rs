#[tokio::main]
async fn main() {
    std::process::exit(yunx_desktop_lib::cli::run().await);
}
