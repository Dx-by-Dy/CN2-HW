use std::io::{self, Read};

/// Читаем из TCP одно RTSP-сообщение. `None` — соединение закрыто.
///
/// Сообщения без тела в учебном протоколе не заканчиваются пустой строкой,
/// поэтому такое сообщение — это то, что пришло за одно чтение (как в исходном коде).
/// Если в заголовках есть Content-Length, дочитываем заголовки до пустой строки
/// и тело ровно до указанной длины: длинное сообщение может прийти частями.
///
/// Ограничение: начало сообщения до строки Content-Length должно прийти одним
/// куском, иначе его не отличить от сообщения без тела. Сообщения у нас короткие
/// и отправляются одним вызовом, так что на практике это выполняется.
pub fn read_message(stream: &mut impl Read) -> io::Result<Option<String>> {
    let mut chunk = [0u8; 2048];
    let mut data = Vec::new();
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok((!data.is_empty()).then(|| String::from_utf8_lossy(&data).into_owned()));
        }
        data.extend_from_slice(&chunk[..n]);
        if is_complete(&data) {
            return Ok(Some(String::from_utf8_lossy(&data).into_owned()));
        }
    }
}

fn is_complete(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(data);
    let Some(content_length) = content_length(&text) else {
        return true; // без тела
    };
    match header_end(data) {
        Some(end) => data.len() >= end + content_length,
        None => false, // заголовки пришли не полностью
    }
}

fn content_length(text: &str) -> Option<usize> {
    text.lines()
        .take_while(|line| !line.trim().is_empty())
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == "Content-Length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
}

/// Позиция начала тела (после пустой строки).
fn header_end(data: &[u8]) -> Option<usize> {
    let crlf = data
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4);
    let lf = data.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Отдает данные кусками заданных размеров, как TCP.
    struct Chunked<'a> {
        data: &'a [u8],
        chunks: Vec<usize>,
    }

    impl<'a> Chunked<'a> {
        fn new(data: &'a str, mut chunks: Vec<usize>) -> Self {
            chunks.reverse();
            Self {
                data: data.as_bytes(),
                chunks,
            }
        }
    }

    impl Read for Chunked<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let chunk = self.chunks.pop().unwrap_or(usize::MAX);
            let n = chunk.min(self.data.len()).min(buf.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    const SDP_REPLY: &str =
        "RTSP/1.0 200 OK\nCSeq: 1\nContent-Type: application/sdp\nContent-Length: 10\n\n0123456789";

    #[test]
    fn message_without_body() {
        let text = "PLAY a RTSP/1.0\nCSeq: 2\nSession: 1";
        let mut stream = Chunked::new(text, vec![]);
        assert_eq!(read_message(&mut stream).unwrap().unwrap(), text);
    }

    #[test]
    fn body_split_into_chunks() {
        // Тело приходит тремя кусками
        let mut stream = Chunked::new(SDP_REPLY, vec![80, 2, 3]);
        assert_eq!(read_message(&mut stream).unwrap().unwrap(), SDP_REPLY);
    }

    #[test]
    fn headers_split_before_blank_line() {
        // Первый кусок обрывается прямо перед пустой строкой
        let mut stream = Chunked::new(SDP_REPLY, vec![73, 1]);
        assert_eq!(read_message(&mut stream).unwrap().unwrap(), SDP_REPLY);
    }

    #[test]
    fn closed_connection() {
        let mut stream = Chunked::new("", vec![]);
        assert_eq!(read_message(&mut stream).unwrap(), None);
    }
}
