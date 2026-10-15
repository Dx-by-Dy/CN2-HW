use std::io;
use std::net::TcpListener;

use crate::server_worker::ServerWorker;

/// Принимает RTSP/TCP-соединения и запускает для каждого клиента свой `ServerWorker`.
pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn bind(port: u16) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(("0.0.0.0", port))?,
        })
    }

    pub fn run(self) {
        loop {
            match self.listener.accept() {
                Ok((socket, addr)) => ServerWorker::new(socket, addr.ip()).spawn(),
                Err(e) => eprintln!("Ошибка accept: {e}"),
            }
        }
    }
}
