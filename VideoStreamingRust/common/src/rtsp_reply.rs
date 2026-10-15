use std::fmt;

/// RTSP-ответ сервера.
///
/// ```text
/// RTSP/1.0 200 OK
/// CSeq: 1
/// Session: 123456
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspReply {
    pub code: u16,
    pub cseq: u32,
    pub session: u32,
}

impl RtspReply {
    pub fn ok(cseq: u32, session: u32) -> Self {
        Self {
            code: 200,
            cseq,
            session,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.code == 200
    }

    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let code = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;

        let mut cseq = None;
        let mut session = None;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            match name.trim() {
                "CSeq" => cseq = value.trim().parse().ok(),
                "Session" => session = value.trim().parse().ok(),
                _ => {}
            }
        }

        Some(Self {
            code,
            cseq: cseq?,
            session: session?,
        })
    }

    fn reason(&self) -> &'static str {
        match self.code {
            200 => "OK",
            404 => "FILE_NOT_FOUND",
            500 => "CONNECTION_ERROR",
            _ => "UNKNOWN",
        }
    }
}

impl fmt::Display for RtspReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RTSP/1.0 {} {}\nCSeq: {}\nSession: {}",
            self.code,
            self.reason(),
            self.cseq,
            self.session
        )
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
}
