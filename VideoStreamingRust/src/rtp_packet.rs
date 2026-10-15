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

        header[0] =  (version & 0b11) << 6 | (padding & 0b1) << 5 | (extension & 0b1) << 4 | (cc & 0b1111);
        header[1] = (marker & 0b1) << 7 | (pt & 0b0111_1111);

        header[2] = (seqnum >> 8) as u8;
        header[3] = seqnum as u8;

        header[4] = (timestamp >> 24) as u8;
        header[5] = (timestamp >> 16) as u8;
        header[6] = (timestamp >> 8) as u8;
        header[7] = timestamp as u8;

        header[8] = (ssrc >> 24) as u8;
        header[9] = (ssrc >> 16) as u8;
        header[10] = (ssrc >> 8) as u8;
        header[11] = ssrc as u8;
        self.header = header;

        self.payload = payload.to_vec();
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
