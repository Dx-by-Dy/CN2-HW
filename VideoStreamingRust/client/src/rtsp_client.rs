use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use common::{Method, RtspReply, RtspRequest};

use crate::config::Config;
use crate::rtp_receiver::RtpReceiver;
use crate::session::Session;
use crate::stats::Stats;
use crate::ui_handle::UiHandle;

/// RTSP-клиент: отправляет запросы и обрабатывает ответы сервера.
pub struct RtspClient {
    file_name: String,
    rtp_port: u16,
    /// `None`, если соединиться с сервером не удалось.
    socket: Option<TcpStream>,
    session: Mutex<Session>,
    receiver: Arc<RtpReceiver>,
    ui: Arc<UiHandle>,
}

impl RtspClient {
    /// Соединяемся с сервером и запускаем поток приема RTSP-ответов.
    pub fn connect(config: &Config, ui: Arc<UiHandle>) -> Arc<Self> {
        let socket = TcpStream::connect((config.server_addr.as_str(), config.server_port)).ok();
        if socket.is_none() {
            ui.show_warning(
                "Connection failed",
                format!("Cannot connect to '{}'.", config.server_addr),
            );
        }

        let client = Arc::new(Self {
            file_name: config.file_name.clone(),
            rtp_port: config.rtp_port,
            socket,
            session: Mutex::new(Session::default()),
            receiver: Arc::new(RtpReceiver::new(Arc::clone(&ui))),
            ui,
        });
        if client.socket.is_some() {
            let client = Arc::clone(&client);
            thread::spawn(move || client.recv_rtsp_replies());
        }
        client
    }

    pub fn stats(&self) -> MutexGuard<'_, Stats> {
        self.receiver.stats()
    }

    pub fn setup(&self) {
        self.send_rtsp_request(Method::Setup);
    }

    pub fn play(&self) {
        if self.send_rtsp_request(Method::Play) {
            self.receiver.start();
        }
    }

    pub fn pause(&self) {
        self.send_rtsp_request(Method::Pause);
    }

    pub fn teardown(&self) {
        self.send_rtsp_request(Method::Teardown);
    }

    /// Отправляем RTSP-запрос, если он допустим в текущем состоянии.
    fn send_rtsp_request(&self, method: Method) -> bool {
        let mut session = self.session.lock().unwrap();
        if !session.state.allows(method) {
            return false;
        }

        session.cseq += 1;
        session.request_sent = Some(method);

        let is_setup = method == Method::Setup;
        let request = RtspRequest {
            method,
            file_name: self.file_name.clone(),
            cseq: session.cseq,
            client_port: is_setup.then_some(self.rtp_port),
            session: (!is_setup).then_some(session.id),
        }
        .to_string();

        if let Some(socket) = &self.socket
            && let Err(e) = (&*socket).write_all(request.as_bytes())
        {
            println!("Failed to send RTSP request: {e}");
        }
        println!("\nData sent:\n{request}");
        true
    }

    /// Получаем RTSP-ответы от сервера.
    fn recv_rtsp_replies(&self) {
        let Some(socket) = &self.socket else {
            return;
        };
        let mut buf = [0u8; 1024];
        loop {
            let n = match (&*socket).read(&mut buf) {
                Ok(0) | Err(_) => break, // сервер закрыл соединение
                Ok(n) => n,
            };
            if let Some(reply) = RtspReply::parse(&String::from_utf8_lossy(&buf[..n])) {
                self.process_rtsp_reply(reply);
            }

            // Закрываем RTSP-сокет после TEARDOWN
            if self.session.lock().unwrap().request_sent == Some(Method::Teardown) {
                let _ = socket.shutdown(Shutdown::Both);
                break;
            }
        }
    }

    fn process_rtsp_reply(&self, reply: RtspReply) {
        let mut session = self.session.lock().unwrap();

        // Обрабатываем только ответ на последний запрос
        if reply.cseq != session.cseq {
            return;
        }
        // Идентификатор нового сеанса
        if session.id == 0 {
            session.id = reply.session;
        }
        if reply.session != session.id || !reply.is_ok() {
            return;
        }
        let Some(method) = session.request_sent else {
            return;
        };

        session.state = session.state.after(method);
        match method {
            Method::Setup => {
                if self.receiver.open(self.rtp_port).is_err() {
                    self.ui.show_warning(
                        "Bind failed",
                        format!("Cannot bind socket to port {}", self.rtp_port),
                    );
                }
            }
            Method::Play => {}
            Method::Pause => self.receiver.stop(),
            Method::Teardown => self.receiver.close(),
        }
    }
}
