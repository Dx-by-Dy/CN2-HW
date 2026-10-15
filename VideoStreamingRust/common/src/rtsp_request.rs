use std::fmt;

use crate::Method;

/// RTSP-запрос клиента.
///
/// ```text
/// SETUP movie.Mjpeg RTSP/1.0
/// CSeq: 1
/// Transport: RTP/UDP; client_port= 25000
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtspRequest {
    pub method: Method,
    pub file_name: String,
    pub cseq: u32,
    /// Заголовок Transport: только в SETUP.
    pub client_port: Option<u16>,
    /// Заголовок Session: во всех запросах, кроме SETUP.
    pub session: Option<u32>,
}

impl RtspRequest {
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let mut request_line = lines.next()?.split_whitespace();
        let method = request_line.next()?.parse().ok()?;
        let file_name = request_line.next()?.to_owned();

        let mut cseq = None;
        let mut client_port = None;
        let mut session = None;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            match name.trim() {
                "CSeq" => cseq = value.trim().parse().ok(),
                "Session" => session = value.trim().parse().ok(),
                "Transport" => {
                    client_port = value
                        .split_once("client_port=")
                        .and_then(|(_, port)| port.split(';').next()?.trim().parse().ok());
                }
                _ => {}
            }
        }

        Some(Self {
            method,
            file_name,
            cseq: cseq?,
            client_port,
            session,
        })
    }
}

impl fmt::Display for RtspRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} RTSP/1.0\nCSeq: {}",
            self.method, self.file_name, self.cseq
        )?;
        if let Some(port) = self.client_port {
            write!(f, "\nTransport: RTP/UDP; client_port= {port}")?;
        }
        if let Some(session) = self.session {
            write!(f, "\nSession: {session}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_format_matches_task() {
        let request = RtspRequest {
            method: Method::Setup,
            file_name: "movie.Mjpeg".into(),
            cseq: 1,
            client_port: Some(25000),
            session: None,
        };
        let text = "SETUP movie.Mjpeg RTSP/1.0\nCSeq: 1\nTransport: RTP/UDP; client_port= 25000";
        assert_eq!(request.to_string(), text);
        assert_eq!(RtspRequest::parse(text), Some(request));
    }

    #[test]
    fn play_roundtrip() {
        let text = "PLAY movie.Mjpeg RTSP/1.0\nCSeq: 2\nSession: 123456";
        let request = RtspRequest::parse(text).unwrap();
        assert_eq!(request.method, Method::Play);
        assert_eq!(request.session, Some(123456));
        assert_eq!(request.to_string(), text);
    }

    #[test]
    fn transport_without_space() {
        let text = "SETUP a RTSP/1.0\r\nCSeq: 1\r\nTransport: RTP/UDP;client_port=4000\r\n";
        assert_eq!(RtspRequest::parse(text).unwrap().client_port, Some(4000));
    }
}
