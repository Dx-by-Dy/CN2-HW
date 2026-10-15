use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::thread;

use common::{MediaInfo, Method, RtspReply, RtspRequest, State, read_message};
use rand::Rng;

use crate::rtp_sender::{FRAME_RATE, RtpSender};
use crate::video_stream::VideoStream;

/// Обслуживает RTSP-сеанс одного клиента.
pub struct ServerWorker {
    rtsp_socket: TcpStream,
    client_ip: IpAddr,
    state: State,
    /// Идентификатор сеанса (0 — сеанса нет).
    session: u32,
    /// Начальная метка времени RTP в сеансе.
    timestamp_base: u32,
    rtp_port: Option<u16>,
    /// Видеофайл, пока поток не передается (READY).
    video: Option<VideoStream>,
    /// Передача RTP, пока идет воспроизведение (PLAYING).
    sender: Option<RtpSender>,
}

impl ServerWorker {
    pub fn new(rtsp_socket: TcpStream, client_ip: IpAddr) -> Self {
        Self {
            rtsp_socket,
            client_ip,
            state: State::Init,
            session: 0,
            timestamp_base: 0,
            rtp_port: None,
            video: None,
            sender: None,
        }
    }

    pub fn spawn(mut self) {
        thread::spawn(move || self.recv_rtsp_requests());
    }

    /// Принимаем RTSP-запросы, пока клиент не закроет соединение.
    fn recv_rtsp_requests(&mut self) {
        while let Ok(Some(data)) = read_message(&mut self.rtsp_socket) {
            println!("Data received:\n{data}");

            match RtspRequest::parse(&data) {
                Some(request) => self.process_rtsp_request(request),
                None => println!("Malformed request"),
            }
        }
        self.stop_streaming();
    }

    /// Обрабатываем запрос и всегда отвечаем: 200 или код ошибки.
    fn process_rtsp_request(&mut self, request: RtspRequest) {
        println!("Processing {}\n", request.method);

        let result = if !self.state.allows(request.method) {
            Err(RtspReply::METHOD_NOT_VALID)
        } else {
            self.execute(&request)
        };
        let session = |worker: &Self| (worker.session != 0).then_some(worker.session);
        let reply = match result {
            Ok(body) => {
                self.state = self.state.after(request.method);
                let reply = RtspReply::new(RtspReply::OK, request.cseq, session(self));
                match body {
                    Some(sdp) => reply.with_body(MediaInfo::CONTENT_TYPE, sdp),
                    None => reply,
                }
            }
            Err(code) => RtspReply::new(code, request.cseq, session(self)),
        };
        self.reply(reply);
    }

    /// Выполняем команду. `Ok(Some(тело))` — ответ с телом (DESCRIBE).
    fn execute(&mut self, request: &RtspRequest) -> Result<Option<String>, u16> {
        match request.method {
            Method::Describe => self.describe(request).map(Some),
            Method::Setup => self.setup(request).map(|()| None),
            Method::Play => self.play().map(|()| None),
            Method::Pause => {
                self.stop_streaming();
                Ok(None)
            }
            Method::Teardown => {
                self.stop_streaming();
                self.video = None;
                self.session = 0;
                Ok(None)
            }
        }
    }

    /// Описание медиапотока в формате SDP.
    fn describe(&self, request: &RtspRequest) -> Result<String, u16> {
        let info =
            VideoStream::probe(&request.file_name, FRAME_RATE).map_err(|_| RtspReply::NOT_FOUND)?;
        let server_ip = self
            .rtsp_socket
            .local_addr()
            .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |addr| addr.ip());
        Ok(info.to_sdp(&request.file_name, server_ip))
    }

    fn setup(&mut self, request: &RtspRequest) -> Result<(), u16> {
        self.video = Some(VideoStream::open(&request.file_name).map_err(|_| RtspReply::NOT_FOUND)?);
        self.rtp_port = request.client_port;
        // Генерируем случайный идентификатор сеанса RTSP
        self.session = rand::rng().random_range(100000..=999999);
        self.timestamp_base = rand::rng().random();
        Ok(())
    }

    fn play(&mut self) -> Result<(), u16> {
        const CON_ERR: u16 = RtspReply::INTERNAL_ERROR;
        let port = self.rtp_port.ok_or(CON_ERR)?;
        let video = self.video.take().ok_or(CON_ERR)?;
        let dest = SocketAddr::new(self.client_ip, port);
        self.sender =
            Some(RtpSender::start(video, dest, self.timestamp_base).map_err(|_| CON_ERR)?);
        Ok(())
    }

    /// Останавливаем передачу RTP и забираем видеофайл обратно.
    fn stop_streaming(&mut self) {
        if let Some(sender) = self.sender.take() {
            self.video = Some(sender.stop());
        }
    }

    fn reply(&mut self, reply: RtspReply) {
        if !reply.is_ok() {
            println!("{} {}\n", reply.code, reply.reason());
        }
        if self
            .rtsp_socket
            .write_all(reply.to_string().as_bytes())
            .is_err()
        {
            println!("Connection error");
        }
    }
}
