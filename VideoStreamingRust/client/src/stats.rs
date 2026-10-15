use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Окно, по которому считается текущая скорость.
const WINDOW: Duration = Duration::from_secs(1);

/// Статистика RTP-сеанса: потери пакетов и скорость видеопотока.
///
/// Потери считаются по номерам последовательности (как в RFC 3550):
/// ожидалось `highest - first + 1` пакетов, потеряно `ожидалось - принято`.
/// Скорость считается по полезной нагрузке (данным видео), без RTP-заголовков.
#[derive(Default)]
pub struct Stats {
    first_seq: Option<u16>,
    highest_seq: u16,
    received: u32,
    out_of_order: u32,
    total_bytes: u64,
    /// Пакеты за последнее окно: (время прихода, размер данных).
    recent: VecDeque<(Instant, usize)>,
    /// Начало текущего интервала воспроизведения (`None` на паузе).
    playing_since: Option<Instant>,
    /// Суммарное время воспроизведения в завершенных интервалах.
    played: Duration,
}

impl Stats {
    /// Начало воспроизведения (PLAY).
    pub fn on_play(&mut self, now: Instant) {
        self.playing_since.get_or_insert(now);
    }

    /// Остановка воспроизведения (PAUSE/TEARDOWN): паузы не входят в среднюю скорость.
    pub fn on_pause(&mut self, now: Instant) {
        if let Some(since) = self.playing_since.take() {
            self.played += now.duration_since(since);
        }
    }

    pub fn on_packet(&mut self, seq: u16, payload_len: usize, now: Instant) {
        match self.first_seq {
            None => {
                self.first_seq = Some(seq);
                self.highest_seq = seq;
            }
            Some(_) if seq > self.highest_seq => self.highest_seq = seq,
            Some(_) => self.out_of_order += 1,
        }
        self.received += 1;
        self.total_bytes += payload_len as u64;

        self.recent.push_back((now, payload_len));
        while self
            .recent
            .front()
            .is_some_and(|&(t, _)| now.duration_since(t) > WINDOW)
        {
            self.recent.pop_front();
        }
    }

    pub fn received(&self) -> u32 {
        self.received
    }

    pub fn expected(&self) -> u32 {
        self.first_seq
            .map_or(0, |first| u32::from(self.highest_seq - first) + 1)
    }

    pub fn lost(&self) -> u32 {
        self.expected().saturating_sub(self.received)
    }

    /// Доля потерянных пакетов, от 0 до 1.
    pub fn loss_rate(&self) -> f64 {
        match self.expected() {
            0 => 0.0,
            expected => f64::from(self.lost()) / f64::from(expected),
        }
    }

    /// Пакеты, пришедшие позже пакета с большим номером (или повторно).
    pub fn out_of_order(&self) -> u32 {
        self.out_of_order
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Скорость за последнюю секунду, бит/с.
    pub fn current_bitrate(&self, now: Instant) -> f64 {
        let bytes: usize = self.in_window(now).map(|&(_, len)| len).sum();
        bytes as f64 * 8.0 / WINDOW.as_secs_f64()
    }

    /// Кадров за последнюю секунду.
    pub fn current_fps(&self, now: Instant) -> f64 {
        self.in_window(now).count() as f64 / WINDOW.as_secs_f64()
    }

    /// Средняя скорость за все время воспроизведения, бит/с.
    pub fn average_bitrate(&self, now: Instant) -> f64 {
        let current = self
            .playing_since
            .map_or(Duration::ZERO, |since| now.duration_since(since));
        let played = (self.played + current).as_secs_f64();
        if played == 0.0 {
            0.0
        } else {
            self.total_bytes as f64 * 8.0 / played
        }
    }

    fn in_window(&self, now: Instant) -> impl Iterator<Item = &(Instant, usize)> {
        self.recent
            .iter()
            .filter(move |&&(t, _)| now.duration_since(t) <= WINDOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loss_by_sequence_gaps() {
        let now = Instant::now();
        let mut stats = Stats::default();
        for seq in [1, 2, 4, 5, 8] {
            stats.on_packet(seq, 100, now);
        }
        assert_eq!(stats.expected(), 8);
        assert_eq!(stats.lost(), 3);
        assert!((stats.loss_rate() - 3.0 / 8.0).abs() < 1e-9);

        // Опоздавший пакет 3 заполняет пропуск
        stats.on_packet(3, 100, now);
        assert_eq!(stats.lost(), 2);
        assert_eq!(stats.out_of_order(), 1);
    }

    #[test]
    fn current_bitrate_uses_last_second() {
        let start = Instant::now();
        let mut stats = Stats::default();
        stats.on_packet(1, 1000, start);
        stats.on_packet(2, 1000, start + Duration::from_millis(1500));
        stats.on_packet(3, 1000, start + Duration::from_millis(2000));

        let now = start + Duration::from_millis(2000);
        assert_eq!(stats.current_bitrate(now), 2.0 * 1000.0 * 8.0);
        assert_eq!(stats.current_fps(now), 2.0);
        // Через 2 секунды без пакетов скорость падает до нуля
        assert_eq!(stats.current_bitrate(now + Duration::from_secs(2)), 0.0);
    }

    #[test]
    fn average_bitrate_excludes_pauses() {
        let start = Instant::now();
        let mut stats = Stats::default();
        stats.on_play(start);
        stats.on_packet(1, 1000, start + Duration::from_millis(500));
        stats.on_pause(start + Duration::from_secs(1));
        // 10 секунд паузы не учитываются
        let now = start + Duration::from_secs(11);
        assert_eq!(stats.average_bitrate(now), 1000.0 * 8.0);
    }
}
