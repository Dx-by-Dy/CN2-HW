use std::time::{SystemTime, UNIX_EPOCH};

pub const HEADER_SIZE: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct RtpPacket {
    pub header: [u8; HEADER_SIZE],
    pub payload: Vec<u8>,
}

impl RtpPacket {
    pub fn new() -> Self {
        Self::default()
    }

    /// Кодируем RTP-пакет с полями заголовка и полезной нагрузкой (данными).
    #[allow(clippy::too_many_arguments)]
    #[allow(unused_variables, unused_mut)] // убрать после заполнения
    pub fn encode(
        &mut self,
        version: u8,
        padding: u8,
        extension: u8,
        cc: u8,
        seqnum: u16,
        marker: u8,
        pt: u8,
        ssrc: u32,
        payload: &[u8],
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        let mut header = [0u8; HEADER_SIZE];
        //----------------
        // ДЛЯ ЗАПОЛНЕНИЯ
        //----------------
        // Заполняем байтовый массив header полями RTP-заголовка

        // header[0] = ...
        // ...
        // self.header = header;

        // Получаем данные из аргумента
        // self.payload = ...
    }

    /// Декодируем RTP-пакет.
    pub fn decode(&mut self, byte_stream: &[u8]) {
        let n = byte_stream.len().min(HEADER_SIZE);
        self.header = [0u8; HEADER_SIZE];
        self.header[..n].copy_from_slice(&byte_stream[..n]);
        self.payload = byte_stream[n..].to_vec();
    }

    /// Возвращаем версию RTP.
    pub fn version(&self) -> u8 {
        self.header[0] >> 6
    }

    /// Возвращаем номер последовательности (кадра).
    pub fn seq_num(&self) -> u16 {
        (self.header[2] as u16) << 8 | self.header[3] as u16
    }

    /// Возвращаем метку времени.
    pub fn timestamp(&self) -> u32 {
        (self.header[4] as u32) << 24
            | (self.header[5] as u32) << 16
            | (self.header[6] as u32) << 8
            | self.header[7] as u32
    }

    /// Возвращаем тип данных.
    pub fn payload_type(&self) -> u8 {
        self.header[1] & 127
    }

    /// Возвращаем данные.
    pub fn get_payload(&self) -> &[u8] {
        &self.payload
    }

    /// Возвращаем RTP-пакет.
    pub fn get_packet(&self) -> Vec<u8> {
        let mut packet = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        packet.extend_from_slice(&self.header);
        packet.extend_from_slice(&self.payload);
        packet
    }
}
