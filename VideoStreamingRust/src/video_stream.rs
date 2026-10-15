use std::fs::File;
use std::io::{self, BufReader, Read};

pub struct VideoStream {
    file: BufReader<File>,
    frame_num: u32,
}

impl VideoStream {
    pub fn new(filename: &str) -> io::Result<Self> {
        let file = File::open(filename)?;
        Ok(Self {
            file: BufReader::new(file),
            frame_num: 0,
        })
    }

    /// Получаем следующий кадр.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        // Из первых 5 байт получаем размер кадра
        let mut len_buf = [0u8; 5];
        self.file.read_exact(&mut len_buf).ok()?;
        let framelength: usize = std::str::from_utf8(&len_buf).ok()?.trim().parse().ok()?;

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
}
