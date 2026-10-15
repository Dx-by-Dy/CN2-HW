use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{Event, MJPEG_CLOCK_RATE, MJPEG_PAYLOAD_TYPE, RtpPacket};

use crate::video_stream::VideoStream;

/// Кадров в секунду: сервер отправляет по кадру каждые 50 мс.
pub const FRAME_RATE: u32 = 20;
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / FRAME_RATE as u64);
/// На сколько растет метка времени RTP за кадр: 90000 / 20 = 4500.
const TIMESTAMP_STEP: u32 = MJPEG_CLOCK_RATE / FRAME_RATE;

/// Фоновая передача кадров видео клиенту по RTP/UDP.
pub struct RtpSender {
    stop: Arc<Event>,
    worker: JoinHandle<VideoStream>,
}

impl RtpSender {
    /// `timestamp_base` — случайная начальная метка времени сеанса (RFC 3550).
    pub fn start(video: VideoStream, dest: SocketAddr, timestamp_base: u32) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let stop = Arc::new(Event::new());
        let worker = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || send_frames(video, socket, dest, timestamp_base, &stop))
        };
        Ok(Self { stop, worker })
    }

    /// Останавливаем передачу (PAUSE/TEARDOWN). Возвращаем видео, чтобы продолжить с того же кадра.
    pub fn stop(self) -> VideoStream {
        self.stop.set();
        self.worker.join().expect("RTP sender thread panicked")
    }
}

fn send_frames(
    mut video: VideoStream,
    socket: UdpSocket,
    dest: SocketAddr,
    timestamp_base: u32,
    stop: &Event,
) -> VideoStream {
    while !stop.wait(FRAME_INTERVAL) {
        let Some(frame) = video.next_frame() else {
            continue;
        };
        let packet = make_rtp(&frame, video.frame_nbr(), timestamp_base);
        if socket.send_to(&packet, dest).is_err() {
            println!("Connection error");
        }
    }
    video
}

/// RTP-пакетирование видеоданных.
///
/// Метка времени — время кадра в видео в единицах 90 кГц: кадр N (с 1) показывается
/// через (N - 1) / 20 с от начала, то есть на (N - 1) * 4500 тактов позже первого.
fn make_rtp(payload: &[u8], frame_nbr: u32, timestamp_base: u32) -> Vec<u8> {
    let version = 2;
    let padding = 0;
    let extension = 0;
    let cc = 0;
    let marker = 0;
    let pt = MJPEG_PAYLOAD_TYPE;
    let seqnum = frame_nbr as u16;
    let timestamp = timestamp_base.wrapping_add(frame_nbr.saturating_sub(1) * TIMESTAMP_STEP);
    let ssrc = 0;

    let mut packet = RtpPacket::new();
    packet.encode(
        version, padding, extension, cc, seqnum, timestamp, marker, pt, ssrc, payload,
    );
    packet.get_packet()
}
