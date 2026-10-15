use std::fmt;

/// RTSP-ответ сервера.
///
/// ```text
/// RTSP/1.0 200 OK
/// CSeq: 1
/// Session: 123456
/// ```
///
/// Ответ на DESCRIBE приходит до создания сеанса (без Session) и содержит тело:
///
/// ```text
/// RTSP/1.0 200 OK
/// CSeq: 1
/// Content-Type: application/sdp
/// Content-Length: 187
///
/// v=0
/// ...
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspReply {
    pub code: u16,
    pub cseq: u32,
    pub session: Option<u32>,
    /// Тип тела (Content-Type); `None` — ответ без тела.
    pub content_type: Option<String>,
    pub body: String,
}

impl RtspReply {
    pub const OK: u16 = 200;
    pub const NOT_FOUND: u16 = 404;
    pub const METHOD_NOT_VALID: u16 = 455;
    pub const INTERNAL_ERROR: u16 = 500;

    pub fn new(code: u16, cseq: u32, session: Option<u32>) -> Self {
        Self {
            code,
            cseq,
            session,
            content_type: None,
            body: String::new(),
        }
    }

    pub fn ok(cseq: u32, session: u32) -> Self {
        Self::new(Self::OK, cseq, Some(session))
    }

    pub fn with_body(mut self, content_type: &str, body: String) -> Self {
        self.content_type = Some(content_type.to_owned());
        self.body = body;
        self
    }

    pub fn is_ok(&self) -> bool {
        self.code == Self::OK
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (head, body) = split_body(text);
        let mut lines = head.lines();
        let code = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;

        let mut reply = Self::new(code, 0, None);
        let mut cseq = None;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            match name.trim() {
                "CSeq" => cseq = value.trim().parse().ok(),
                "Session" => reply.session = value.trim().parse().ok(),
                "Content-Type" => reply.content_type = Some(value.trim().to_owned()),
                _ => {}
            }
        }
        reply.cseq = cseq?;
        reply.body = body.to_owned();
        Some(reply)
    }

    pub fn reason(&self) -> &'static str {
        match self.code {
            Self::OK => "OK",
            Self::NOT_FOUND => "Not Found",
            Self::METHOD_NOT_VALID => "Method Not Valid in This State",
            Self::INTERNAL_ERROR => "Internal Server Error",
            _ => "Unknown",
        }
    }
}

/// Делим сообщение на заголовки и тело по первой пустой строке.
fn split_body(text: &str) -> (&str, &str) {
    for separator in ["\r\n\r\n", "\n\n"] {
        if let Some((head, body)) = text.split_once(separator) {
            return (head, body);
        }
    }
    (text, "")
}

impl fmt::Display for RtspReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RTSP/1.0 {} {}\nCSeq: {}",
            self.code,
            self.reason(),
            self.cseq
        )?;
        if let Some(session) = self.session {
            write!(f, "\nSession: {session}")?;
        }
        if let Some(content_type) = &self.content_type {
            write!(
                f,
                "\nContent-Type: {content_type}\nContent-Length: {}\n\n{}",
                self.body.len(),
                self.body
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_roundtrip() {
        let text = "RTSP/1.0 200 OK\nCSeq: 3\nSession: 123456";
        assert_eq!(RtspReply::ok(3, 123456).to_string(), text);
        assert_eq!(RtspReply::parse(text), Some(RtspReply::ok(3, 123456)));
    }

    #[test]
    fn body_roundtrip() {
        let reply = RtspReply::new(200, 1, None).with_body("application/sdp", "v=0\ns=x\n".into());
        let text = reply.to_string();
        assert_eq!(
            text,
            "RTSP/1.0 200 OK\nCSeq: 1\nContent-Type: application/sdp\nContent-Length: 8\n\nv=0\ns=x\n"
        );
        assert_eq!(RtspReply::parse(&text), Some(reply));
    }
}
