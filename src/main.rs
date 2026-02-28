mod config;
mod client;
mod error;
mod server;

use config::AIConfig;
use error::Result;
use server::MCPServer;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-v") {
        println!("ai-search-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    
    let config = AIConfig::from_env()?;
    let server = MCPServer::new(config)?;
    server.run().await
}
