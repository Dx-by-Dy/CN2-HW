use std::fs::File;
use std::io::{self, BufReader, Read};

use common::{MJPEG_CLOCK_RATE, MJPEG_PAYLOAD_TYPE, MediaInfo};

/// Чтение кадров из файла MJPEG: перед каждым JPEG-кадром идут 5 байт с его длиной.
pub struct VideoStream {
    file: BufReader<File>,
    frame_num: u32,
}

impl VideoStream {
    pub fn open(filename: &str) -> io::Result<Self> {
        let file = File::open(filename)?;
        Ok(Self {
            file: BufReader::new(file),
            frame_num: 0,
        })
    }

    /// Описание файла для DESCRIBE. Проходим по заголовкам кадров, пропуская
    /// сами кадры; разрешение берем из первого кадра.
    pub fn probe(filename: &str, frame_rate: u32) -> io::Result<MediaInfo> {
        let mut stream = Self::open(filename)?;
        let first = stream.next_frame();
        let (width, height) = first.as_deref().and_then(jpeg_dimensions).unwrap_or((0, 0));

        let mut frames = u32::from(first.is_some());
        let mut total_bytes = first.map_or(0, |frame| frame.len() as u64);
        while let Some(len) = stream.skip_frame() {
            frames += 1;
            total_bytes += len as u64;
        }

        let bitrate = if frames == 0 {
            0
        } else {
            total_bytes * 8 * u64::from(frame_rate) / u64::from(frames)
        };
        Ok(MediaInfo {
            payload_type: MJPEG_PAYLOAD_TYPE,
            encoding: "JPEG".to_owned(),
            clock_rate: MJPEG_CLOCK_RATE,
            frame_rate,
            width,
            height,
            frames,
            bitrate,
        })
    }

    /// Получаем следующий кадр.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        let framelength = self.read_frame_length()?;

        // Читаем текущий кадр
        let mut data = vec![0u8; framelength];
        self.file.read_exact(&mut data).ok()?;
        self.frame_num += 1;
        Some(data)
    }

    /// Получаем номер кадра.
    pub fn frame_nbr(&self) -> u32 {
        self.frame_num
    }

    /// Пропускаем кадр, не читая его. Возвращаем его размер.
    fn skip_frame(&mut self) -> Option<usize> {
        let framelength = self.read_frame_length()?;
        self.file.seek_relative(framelength as i64).ok()?;
        self.frame_num += 1;
        Some(framelength)
    }

    /// Из первых 5 байт получаем размер кадра.
    fn read_frame_length(&mut self) -> Option<usize> {
        let mut len_buf = [0u8; 5];
        self.file.read_exact(&mut len_buf).ok()?;
        std::str::from_utf8(&len_buf).ok()?.trim().parse().ok()
    }
}

/// Ширина и высота JPEG-изображения из маркера SOF (начало кадра).
fn jpeg_dimensions(data: &[u8]) -> Option<(u16, u16)> {
    let be16 =
        |i: usize| -> Option<u16> { Some(u16::from_be_bytes([*data.get(i)?, *data.get(i + 1)?])) };
    let mut i = 2; // после SOI (FF D8)
    loop {
        if *data.get(i)? != 0xFF {
            return None;
        }
        let marker = *data.get(i + 1)?;
        match marker {
            0xFF => i += 1, // байт-заполнитель
            // SOF0..SOF15, кроме DHT (C4), JPG (C8) и DAC (CC)
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            _ => i += 2 + usize::from(be16(i + 2)?),
        }
    }
}
