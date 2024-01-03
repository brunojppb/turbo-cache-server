#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    decay::run()?.await
}
