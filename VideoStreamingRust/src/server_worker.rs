use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rand::Rng;

use crate::event::Event;
use crate::rtp_packet::RtpPacket;
use crate::video_stream::VideoStream;

const SETUP: &str = "SETUP";
const PLAY: &str = "PLAY";
const PAUSE: &str = "PAUSE";
const TEARDOWN: &str = "TEARDOWN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Init,
    Ready,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplyCode {
    Ok200,
    FileNotFound404,
    ConErr500,
}

pub struct ClientInfo {
    pub rtsp_socket: (TcpStream, SocketAddr),
    pub video_stream: Option<Arc<Mutex<VideoStream>>>,
    pub session: u32,
    pub rtp_port: Option<u16>,
    pub rtp_socket: Option<Arc<UdpSocket>>,
    pub event: Option<Arc<Event>>,
    pub worker: Option<JoinHandle<()>>,
}

impl ClientInfo {
    pub fn new(rtsp_socket: (TcpStream, SocketAddr)) -> Self {
        Self {
            rtsp_socket,
            video_stream: None,
            session: 0,
            rtp_port: None,
            rtp_socket: None,
            event: None,
            worker: None,
        }
    }
}

pub struct ServerWorker {
    client_info: ClientInfo,
    state: State,
}

impl ServerWorker {
    pub fn new(client_info: ClientInfo) -> Self {
        Self {
            client_info,
            state: State::Init,
        }
    }

    pub fn run(mut self) {
        thread::spawn(move || self.recv_rtsp_request());
    }

    /// Принимаем запрос RTSP от клиента.
    fn recv_rtsp_request(&mut self) {
        let mut buf = [0u8; 256];
        loop {
            let n = match self.client_info.rtsp_socket.0.read(&mut buf) {
                Ok(0) | Err(_) => break, // клиент закрыл соединение
                Ok(n) => n,
            };
            let data = String::from_utf8_lossy(&buf[..n]).into_owned();
            println!("Данные получены:\n{data}");
            self.process_rtsp_request(&data);
        }
        // Соединение закрыто: останавливаем отправку RTP, если она идет
        if let Some(event) = &self.client_info.event {
            event.set();
        }
    }

    /// Обрабатываем запрос от клиента.
    fn process_rtsp_request(&mut self, data: &str) {
        // Получаем тип запроса
        let request: Vec<&str> = data.lines().collect();
        let line1: Vec<&str> = request.first().map_or(vec![], |l| l.split(' ').collect());
        let (Some(&request_type), Some(&filename)) = (line1.first(), line1.get(1)) else {
            println!("Некорректный запрос");
            return;
        };

        // Получаем номер последовательности RTSP
        let Some(seq) = request.get(1).and_then(|l| l.split(' ').nth(1)).map(str::trim) else {
            println!("Некорректный запрос: нет CSeq");
            return;
        };

        match request_type {
            // Обрабатываем SETUP-запрос
            SETUP => {
                if self.state == State::Init {
                    // Обновляем состояние
                    println!("обработка команды SETUP\n");

                    match VideoStream::new(filename) {
                        Ok(stream) => {
                            self.client_info.video_stream = Some(Arc::new(Mutex::new(stream)));
                            self.state = State::Ready;
                        }
                        Err(_) => {
                            self.reply_rtsp(ReplyCode::FileNotFound404, seq);
                            return;
                        }
                    }

                    // Генерируем случайный идентификатор сеанса RTSP
                    self.client_info.session = rand::rng().random_range(100000..=999999);

                    // Отправляем RTSP-ответ
                    self.reply_rtsp(ReplyCode::Ok200, seq);

                    // Из последней строки извлекаем номер RTP/UDP порта
                    self.client_info.rtp_port = request
                        .get(2)
                        .and_then(|l| l.split(' ').nth(3))
                        .and_then(|p| p.trim().parse().ok());
                }
            }

            // Обрабатываем PLAY-запрос
            PLAY => {
                if self.state == State::Ready {
                    println!("обрабатываем команду PLAY\n");
                    self.state = State::Playing;

                    // Создаем новый сокет для RTP/UDP
                    let rtp_socket = match UdpSocket::bind("0.0.0.0:0") {
                        Ok(s) => Arc::new(s),
                        Err(_) => {
                            self.reply_rtsp(ReplyCode::ConErr500, seq);
                            return;
                        }
                    };
                    self.client_info.rtp_socket = Some(Arc::clone(&rtp_socket));

                    self.reply_rtsp(ReplyCode::Ok200, seq);

                    // Создаем новый поток и запускаем отправку пакетов RTP
                    let event = Arc::new(Event::new());
                    self.client_info.event = Some(Arc::clone(&event));
                    let video_stream = Arc::clone(self.client_info.video_stream.as_ref().unwrap());
                    let address = self.client_info.rtsp_socket.1.ip();
                    let port = self.client_info.rtp_port;
                    self.client_info.worker = Some(thread::spawn(move || {
                        send_rtp(event, video_stream, rtp_socket, address, port)
                    }));
                }
            }

            // Обрабатываем PAUSE-запрос
            PAUSE => {
                if self.state == State::Playing {
                    println!("обрабатываем команду PAUSE\n");
                    self.state = State::Ready;

                    if let Some(event) = &self.client_info.event {
                        event.set();
                    }

                    self.reply_rtsp(ReplyCode::Ok200, seq);
                }
            }

            // Обрабатываем TEARDOWN-запрос
            TEARDOWN => {
                println!("обрабатываем команду TEARDOWN\n");

                if let Some(event) = &self.client_info.event {
                    event.set();
                }

                self.reply_rtsp(ReplyCode::Ok200, seq);

                // Закрываем RTP-сокет
                self.client_info.rtp_socket = None;
            }

            _ => {}
        }
    }

    /// Отправляем RTSP-ответ клиенту.
    fn reply_rtsp(&mut self, code: ReplyCode, seq: &str) {
        match code {
            ReplyCode::Ok200 => {
                let reply = format!(
                    "RTSP/1.0 200 OK\nCSeq: {}\nSession: {}",
                    seq, self.client_info.session
                );
                let conn_socket = &mut self.client_info.rtsp_socket.0;
                if conn_socket.write_all(reply.as_bytes()).is_err() {
                    println!("Ошибка соединения");
                }
            }
            // Сообщения об ошибках
            ReplyCode::FileNotFound404 => println!("404 Файл не найден"),
            ReplyCode::ConErr500 => println!("500 Ошибка соединения"),
        }
    }
}

/// Отправляем RTP-пакеты через UDP.
fn send_rtp(
    event: Arc<Event>,
    video_stream: Arc<Mutex<VideoStream>>,
    rtp_socket: Arc<UdpSocket>,
    address: std::net::IpAddr,
    port: Option<u16>,
) {
    loop {
        // Останавливаем отправку, если запрос PAUSE или TEARDOWN
        if event.wait(Duration::from_millis(50)) {
            break;
        }

        let (data, frame_number) = {
            let mut stream = video_stream.lock().unwrap();
            (stream.next_frame(), stream.frame_nbr())
        };
        if let Some(data) = data {
            let sent = port.map(|port| rtp_socket.send_to(&make_rtp(&data, frame_number), (address, port)));
            if !matches!(sent, Some(Ok(_))) {
                println!("Ошибка соединения");
            }
        }
    }
}

/// RTP-пакетирование видеоданных.
fn make_rtp(payload: &[u8], frame_nbr: u32) -> Vec<u8> {
    let version = 2;
    let padding = 0;
    let extension = 0;
    let cc = 0;
    let marker = 0;
    let pt = 26; // тип для MJPEG
    let seqnum = frame_nbr as u16;
    let ssrc = 0;

    let mut rtp_packet = RtpPacket::new();

    rtp_packet.encode(version, padding, extension, cc, seqnum, marker, pt, ssrc, payload);

    rtp_packet.get_packet()
}
