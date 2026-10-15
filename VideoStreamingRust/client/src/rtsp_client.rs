use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::Duration;

use common::{MediaInfo, Method, RtspReply, RtspRequest, State, read_message};

use crate::config::Config;
use crate::rtp_receiver::RtpReceiver;
use crate::session::Session;
use crate::stats::Stats;
use crate::ui_handle::UiHandle;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// RTSP-клиент с тремя командами плеера: Play, Pause, Stop.
///
/// Play без сеанса подключается к серверу (если еще не подключен) и выполняет
/// цепочку DESCRIBE → SETUP → PLAY: каждый следующий запрос уходит после ответа
/// на предыдущий. Stop — это TEARDOWN: сеанс завершается, TCP-соединение остается
/// открытым, и следующий Play начинает новый сеанс с начала видео.
pub struct RtspClient {
    server_addr: String,
    server_port: u16,
    file_name: String,
    rtp_port: u16,
    /// Соединение с сервером; устанавливается при первом Play.
    socket: OnceLock<TcpStream>,
    session: Mutex<Session>,
    /// Описание видео из ответа на DESCRIBE.
    media_info: Mutex<Option<MediaInfo>>,
    receiver: Arc<RtpReceiver>,
    ui: Arc<UiHandle>,
}

impl RtspClient {
    pub fn new(config: &Config, ui: Arc<UiHandle>) -> Arc<Self> {
        Arc::new(Self {
            server_addr: config.server_addr.clone(),
            server_port: config.server_port,
            file_name: config.file_name.clone(),
            rtp_port: config.rtp_port,
            socket: OnceLock::new(),
            session: Mutex::new(Session::default()),
            media_info: Mutex::new(None),
            receiver: Arc::new(RtpReceiver::new(Arc::clone(&ui))),
            ui,
        })
    }

    pub fn state(&self) -> State {
        self.session.lock().unwrap().state
    }

    pub fn stats(&self) -> MutexGuard<'_, Stats> {
        self.receiver.stats()
    }

    pub fn media_info(&self) -> Option<MediaInfo> {
        self.media_info.lock().unwrap().clone()
    }

    /// Номер последнего показанного кадра.
    pub fn current_frame(&self) -> u16 {
        self.receiver.frame_nbr()
    }

    /// Play: без сеанса начинаем цепочку DESCRIBE → SETUP → PLAY, на паузе — PLAY.
    pub fn play(self: &Arc<Self>) {
        if !self.ensure_connected() {
            return;
        }
        let mut session = self.session.lock().unwrap();
        if session.state == State::Init {
            if self.send_rtsp_request(&mut session, Method::Describe) {
                session.play_requested = true;
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

    /// Подключаемся к серверу, если еще не подключены, и запускаем поток приема ответов.
    fn ensure_connected(self: &Arc<Self>) -> bool {
        if self.socket.get().is_some() {
            return true;
        }
        let Some(socket) = self.connect() else {
            self.ui.show_warning(
                "Connection failed",
                format!(
                    "Cannot connect to '{}:{}'.",
                    self.server_addr, self.server_port
                ),
            );
            return false;
        };
        if self.socket.set(socket).is_ok() {
            let client = Arc::clone(self);
            thread::spawn(move || client.recv_rtsp_replies());
        }
        true
    }

    fn connect(&self) -> Option<TcpStream> {
        let addrs = (self.server_addr.as_str(), self.server_port)
            .to_socket_addrs()
            .ok()?;
        addrs
            .into_iter()
            .find_map(|addr| TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).ok())
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

        let request = RtspRequest {
            method,
            file_name: self.file_name.clone(),
            cseq: session.cseq,
            client_port: (method == Method::Setup).then_some(self.rtp_port),
            session: (session.id != 0).then_some(session.id),
            accept: (method == Method::Describe).then(|| MediaInfo::CONTENT_TYPE.to_owned()),
        }
        .to_string();

        if let Some(socket) = self.socket.get()
            && let Err(e) = (&*socket).write_all(request.as_bytes())
        {
            println!("Failed to send RTSP request: {e}");
        }
        println!("\nData sent:\n{request}");
        true
    }

    /// Получаем RTSP-ответы, пока сервер не закроет соединение.
    fn recv_rtsp_replies(&self) {
        let Some(mut socket) = self.socket.get() else {
            return;
        };
        while let Ok(Some(data)) = read_message(&mut socket) {
            if let Some(reply) = RtspReply::parse(&data) {
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
            session.play_requested = false;
            self.ui.show_warning(
                "Server error",
                format!("Server replied {} {}", reply.code, reply.reason()),
            );
            return;
        }
        // Идентификатор нового сеанса приходит в ответе на SETUP
        if let Some(id) = reply.session {
            if session.id == 0 {
                session.id = id;
            } else if id != session.id {
                return;
            }
        }
        let Some(method) = session.request_sent else {
            return;
        };

        session.state = session.state.after(method);
        match method {
            Method::Describe => self.process_description(&mut session, &reply),
            Method::Setup => match self.receiver.open(self.rtp_port) {
                Ok(()) => {
                    if std::mem::take(&mut session.play_requested) {
                        self.send_play(&mut session);
                    }
                }
                Err(e) => {
                    session.play_requested = false;
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

    /// Ответ на DESCRIBE: запоминаем описание видео и продолжаем цепочку SETUP.
    fn process_description(&self, session: &mut Session, reply: &RtspReply) {
        let Some(info) = MediaInfo::from_sdp(&reply.body) else {
            session.play_requested = false;
            self.ui.show_warning(
                "Bad description",
                "Server sent no video stream description.".to_owned(),
            );
            return;
        };
        self.receiver.set_total_frames(info.frames);
        *self.media_info.lock().unwrap() = Some(info);

        if session.play_requested {
            self.send_rtsp_request(session, Method::Setup);
        }
    }
}
