mod app;
mod config;
mod rtp_receiver;
mod rtsp_client;
mod session;
mod stats;
mod ui_handle;
mod warning;

use app::ClientApp;
use config::Config;

fn main() -> eframe::Result {
    let Some(config) = Config::from_args(std::env::args().skip(1)) else {
        eprintln!("Usage: client <server_host> <server_port> <rtp_port> <video_file>");
        std::process::exit(1);
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RTPClient")
            .with_inner_size([920.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RTPClient",
        options,
        Box::new(move |cc| Ok(Box::new(ClientApp::new(cc, config)))),
    )
}
