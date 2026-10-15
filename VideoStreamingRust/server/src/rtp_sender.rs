use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{Event, RtpPacket};

use crate::video_stream::VideoStream;

const FRAME_INTERVAL: Duration = Duration::from_millis(50);
const MJPEG_PAYLOAD_TYPE: u8 = 26;

/// Фоновая передача кадров видео клиенту по RTP/UDP.
pub struct RtpSender {
    stop: Arc<Event>,
    worker: JoinHandle<VideoStream>,
}

impl RtpSender {
    pub fn start(video: VideoStream, dest: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let stop = Arc::new(Event::new());
        let worker = {
            let stop = Arc::clone(&stop);
            thread::spawn(move || send_frames(video, socket, dest, &stop))
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
    stop: &Event,
) -> VideoStream {
    while !stop.wait(FRAME_INTERVAL) {
        let Some(frame) = video.next_frame() else {
            continue;
        };
        let packet = make_rtp(&frame, video.frame_nbr());
        if socket.send_to(&packet, dest).is_err() {
            println!("Connection error");
        }
    }
    video
}

/// RTP-пакетирование видеоданных.
fn make_rtp(payload: &[u8], frame_nbr: u32) -> Vec<u8> {
    let version = 2;
    let padding = 0;
    let extension = 0;
    let cc = 0;
    let marker = 0;
    let pt = MJPEG_PAYLOAD_TYPE;
    let seqnum = frame_nbr as u16;
    let ssrc = 0;

    let mut packet = RtpPacket::new();
    packet.encode(
        version, padding, extension, cc, seqnum, marker, pt, ssrc, payload,
    );
    packet.get_packet()
}
