use clap::Parser;

#[derive(Parser)]
#[command(name = "aaagent-serve")]
#[command(author, version, about = "aaagent-rs web server for chat UI", long_about = None)]
struct Cli {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    // Load .env file if it exists (ignore if not found)
    match dotenvy::dotenv() {
        Ok(path) => println!("✓ Loaded .env from: {:?}", path),
        Err(e) => println!("ℹ️ No .env file found: {}", e),
    }

    let cli = Cli::parse();

    if cli.verbose {
        println!("Verbose mode enabled");
    }

    println!("Starting aaagent-rs chat UI server...");

    // Create router
    let app = aaagent::api::create_router().await;

    // Start server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], cli.port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    println!("Server running on http://0.0.0.0:{}", cli.port);
    println!("Open http://localhost:{} in your browser", cli.port);

    axum::serve(listener, app).await.expect("Server error");
}
