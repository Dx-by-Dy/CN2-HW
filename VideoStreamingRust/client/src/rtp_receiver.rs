use std::io;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use common::{Event, RtpPacket};
use eframe::egui;

use crate::stats::Stats;
use crate::ui_handle::UiHandle;

/// Прием RTP-пакетов с кадрами видео.
pub struct RtpReceiver {
    socket: Mutex<Option<Arc<UdpSocket>>>,
    /// Флаг остановки текущего потока приема.
    stop: Mutex<Arc<Event>>,
    /// Номер последнего показанного кадра.
    frame_nbr: AtomicU16,
    stats: Mutex<Stats>,
    ui: Arc<UiHandle>,
}

impl RtpReceiver {
    pub fn new(ui: Arc<UiHandle>) -> Self {
        Self {
            socket: Mutex::new(None),
            stop: Mutex::new(Arc::new(Event::new())),
            frame_nbr: AtomicU16::new(0),
            stats: Mutex::new(Stats::default()),
            ui,
        }
    }

    /// Открываем RTP-сокет, привязанный к указанному порту.
    pub fn open(&self, port: u16) -> io::Result<()> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        socket.set_read_timeout(Some(Duration::from_millis(500)))?;
        *self.socket.lock().unwrap() = Some(Arc::new(socket));
        Ok(())
    }

    /// Запускаем поток приема (PLAY).
    pub fn start(self: &Arc<Self>) {
        let stop = Arc::new(Event::new());
        *self.stop.lock().unwrap() = Arc::clone(&stop);
        self.stats().on_play(Instant::now());
        let receiver = Arc::clone(self);
        thread::spawn(move || receiver.listen(&stop));
    }

    /// Останавливаем поток приема (PAUSE).
    pub fn stop(&self) {
        self.stop.lock().unwrap().set();
        self.stats().on_pause(Instant::now());
    }

    /// Останавливаем прием и закрываем сокет (TEARDOWN).
    pub fn close(&self) {
        self.stop();
        *self.socket.lock().unwrap() = None;
    }

    pub fn stats(&self) -> MutexGuard<'_, Stats> {
        self.stats.lock().unwrap()
    }

    fn listen(&self, stop: &Event) {
        let Some(socket) = self.socket.lock().unwrap().clone() else {
            return;
        };
        let mut buf = vec![0u8; 20480];
        while !stop.is_set() {
            // Ошибка здесь — обычно тайм-аут: просто проверяем флаг остановки
            if let Ok(n) = socket.recv(&mut buf) {
                self.handle_packet(&buf[..n]);
            }
        }
    }

    fn handle_packet(&self, data: &[u8]) {
        let mut packet = RtpPacket::new();
        packet.decode(data);

        let curr_frame_nbr = packet.seq_num();
        println!("Current seq num: {curr_frame_nbr}");
        self.stats()
            .on_packet(curr_frame_nbr, packet.get_payload().len(), Instant::now());

        // Отбрасываем опоздавшие пакеты
        if curr_frame_nbr > self.frame_nbr.load(Ordering::Relaxed) {
            self.frame_nbr.store(curr_frame_nbr, Ordering::Relaxed);
            match decode_jpeg(packet.get_payload()) {
                Some(frame) => self.ui.show_frame(frame),
                None => println!("Failed to decode frame"),
            }
        }
    }
}

fn decode_jpeg(data: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg).ok()?;
    let img = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}
