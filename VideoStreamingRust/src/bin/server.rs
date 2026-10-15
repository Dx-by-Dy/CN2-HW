use std::net::TcpListener;

use video_streaming::server_worker::{ClientInfo, ServerWorker};

fn main() {
    let Some(server_port) = std::env::args().nth(1).and_then(|p| p.parse::<u16>().ok()) else {
        println!("[Используйте формат: server Server_port]\n");
        std::process::exit(1);
    };

    let rtsp_socket = TcpListener::bind(("0.0.0.0", server_port)).expect("не удалось открыть порт");

    // Получаем информацию о клиенте (адрес, порт) из сеанса RTSP/TCP
    loop {
        match rtsp_socket.accept() {
            Ok(conn) => ServerWorker::new(ClientInfo::new(conn)).run(),
            Err(e) => println!("Ошибка accept: {e}"),
        }
    }
}
