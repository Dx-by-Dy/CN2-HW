use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::thread;

use common::{Method, RtspReply, RtspRequest, State};
use rand::Rng;

use crate::rtp_sender::RtpSender;
use crate::video_stream::VideoStream;

/// Обслуживает RTSP-сеанс одного клиента.
pub struct ServerWorker {
    rtsp_socket: TcpStream,
    client_ip: IpAddr,
    state: State,
    session: u32,
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
        let mut buf = [0u8; 256];
        loop {
            let n = match self.rtsp_socket.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let data = String::from_utf8_lossy(&buf[..n]);
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
        let code = match result {
            Ok(()) => {
                self.state = self.state.after(request.method);
                RtspReply::OK
            }
            Err(code) => code,
        };
        self.reply(RtspReply {
            code,
            cseq: request.cseq,
            session: self.session,
        });
    }

    fn execute(&mut self, request: &RtspRequest) -> Result<(), u16> {
        match request.method {
            Method::Setup => self.setup(request),
            Method::Play => self.play(),
            Method::Pause => {
                self.stop_streaming();
                Ok(())
            }
            Method::Teardown => {
                self.stop_streaming();
                self.video = None;
                Ok(())
            }
        }
    }

    fn setup(&mut self, request: &RtspRequest) -> Result<(), u16> {
        self.video = Some(VideoStream::open(&request.file_name).map_err(|_| RtspReply::NOT_FOUND)?);
        self.rtp_port = request.client_port;
        // Генерируем случайный идентификатор сеанса RTSP
        self.session = rand::rng().random_range(100000..=999999);
        Ok(())
    }

    fn play(&mut self) -> Result<(), u16> {
        const CON_ERR: u16 = RtspReply::INTERNAL_ERROR;
        let port = self.rtp_port.ok_or(CON_ERR)?;
        let video = self.video.take().ok_or(CON_ERR)?;
        let dest = SocketAddr::new(self.client_ip, port);
        self.sender = Some(RtpSender::start(video, dest).map_err(|_| CON_ERR)?);
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
