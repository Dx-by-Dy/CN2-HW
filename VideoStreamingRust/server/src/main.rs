mod rtp_sender;
mod server;
mod server_worker;
mod video_stream;

use server::Server;

fn main() {
    let Some(port) = std::env::args().nth(1).and_then(|p| p.parse::<u16>().ok()) else {
        eprintln!("Usage: server <server_port>");
        std::process::exit(1);
    };

    match Server::bind(port) {
        Ok(server) => server.run(),
        Err(e) => {
            eprintln!("Failed to bind port {port}: {e}");
            std::process::exit(1);
        }
    }
}
