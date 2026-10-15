mod app;
mod config;
mod rtp_receiver;
mod rtsp_client;
mod session;
mod ui_handle;
mod warning;

use app::ClientApp;
use config::Config;

fn main() -> eframe::Result {
    let Some(config) = Config::from_args(std::env::args().skip(1)) else {
        eprintln!("[Используйте формат: client Server_name Server_port RTP_port Video_file]");
        std::process::exit(1);
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RTPClient")
            .with_inner_size([700.0, 360.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RTPClient",
        options,
        Box::new(move |cc| Ok(Box::new(ClientApp::new(cc, config)))),
    )
}
