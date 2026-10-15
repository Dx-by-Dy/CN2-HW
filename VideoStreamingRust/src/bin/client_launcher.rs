use video_streaming::client::Client;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let parsed = (|| {
        let server_addr = args.get(1)?.clone();
        let server_port = args.get(2)?.parse::<u16>().ok()?;
        let rtp_port = args.get(3)?.parse::<u16>().ok()?;
        let file_name = args.get(4)?.clone();
        Some((server_addr, server_port, rtp_port, file_name))
    })();
    let Some((server_addr, server_port, rtp_port, file_name)) = parsed else {
        println!("[Используйте формат: client Server_name Server_port RTP_port Video_file]\n");
        std::process::exit(1);
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("RTPClient")
            .with_inner_size([700.0, 360.0]),
        ..Default::default()
    };

    // Создаем новый клиент
    eframe::run_native(
        "RTPClient",
        options,
        Box::new(move |cc| Ok(Box::new(Client::new(cc, server_addr, server_port, rtp_port, file_name)))),
    )
}
