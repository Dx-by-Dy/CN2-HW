use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use common::{Method, RtspReply, RtspRequest, State};

use crate::config::Config;
use crate::rtp_receiver::RtpReceiver;
use crate::session::Session;
use crate::stats::Stats;
use crate::ui_handle::UiHandle;

/// RTSP-клиент с тремя командами плеера: Play, Pause, Stop.
///
/// SETUP отправляется автоматически при первом Play (и при Play после Stop).
/// Stop — это TEARDOWN: сеанс завершается, TCP-соединение остается открытым,
/// и следующий Play начинает новый сеанс с начала видео.
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

    pub fn state(&self) -> State {
        self.session.lock().unwrap().state
    }

    pub fn stats(&self) -> MutexGuard<'_, Stats> {
        self.receiver.stats()
    }

    /// Play: без сеанса сначала SETUP, PLAY уйдет после ответа на него.
    pub fn play(&self) {
        let mut session = self.session.lock().unwrap();
        if session.state == State::Init {
            if self.send_rtsp_request(&mut session, Method::Setup) {
                session.play_after_setup = true;
            }
        } else {
            self.send_play(&mut session);
        }
    }

    pub fn pause(&self) {
        let mut session = self.session.lock().unwrap();
        self.send_rtsp_request(&mut session, Method::Pause);
    }

    /// Stop: завершаем сеанс (TEARDOWN), соединение с сервером не разрываем.
    pub fn stop(&self) {
        let mut session = self.session.lock().unwrap();
        self.send_rtsp_request(&mut session, Method::Teardown);
    }

    fn send_play(&self, session: &mut Session) {
        if self.send_rtsp_request(session, Method::Play) {
            self.receiver.start();
        }
    }

    /// Отправляем RTSP-запрос, если он допустим в текущем состоянии
    /// и ответ на предыдущий запрос уже получен.
    fn send_rtsp_request(&self, session: &mut Session, method: Method) -> bool {
        if session.awaiting_reply || !session.state.allows(method) {
            return false;
        }

        session.cseq += 1;
        session.request_sent = Some(method);
        session.awaiting_reply = true;

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

    /// Получаем RTSP-ответы, пока сервер не закроет соединение.
    fn recv_rtsp_replies(&self) {
        let Some(socket) = &self.socket else {
            return;
        };
        let mut buf = [0u8; 1024];
        loop {
            let n = match (&*socket).read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if let Some(reply) = RtspReply::parse(&String::from_utf8_lossy(&buf[..n])) {
                self.process_rtsp_reply(reply);
            }
        }
    }

    fn process_rtsp_reply(&self, reply: RtspReply) {
        let mut session = self.session.lock().unwrap();

        // Обрабатываем только ответ на последний запрос
        if reply.cseq != session.cseq {
            return;
        }
        session.awaiting_reply = false;

        if !reply.is_ok() {
            session.play_after_setup = false;
            self.ui.show_warning(
                "Server error",
                format!("Server replied {} {}", reply.code, reply.reason()),
            );
            return;
        }
        // Идентификатор нового сеанса
        if session.id == 0 {
            session.id = reply.session;
        }
        if reply.session != session.id {
            return;
        }
        let Some(method) = session.request_sent else {
            return;
        };

        session.state = session.state.after(method);
        match method {
            Method::Setup => match self.receiver.open(self.rtp_port) {
                Ok(()) => {
                    if std::mem::take(&mut session.play_after_setup) {
                        self.send_play(&mut session);
                    }
                }
                Err(e) => {
                    session.play_after_setup = false;
                    self.ui.show_warning(
                        "Bind failed",
                        format!("Cannot bind socket to port {}: {e}", self.rtp_port),
                    );
                }
            },
            Method::Play => {}
            Method::Pause => self.receiver.stop(),
            Method::Teardown => {
                session.id = 0;
                self.receiver.stop();
                self.receiver.reset();
            }
        }
    }
}
