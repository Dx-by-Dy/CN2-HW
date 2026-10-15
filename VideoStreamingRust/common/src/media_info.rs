use std::net::IpAddr;

/// Параметры медиапотока, которые сервер сообщает в ответ на DESCRIBE.
///
/// Передаются в формате SDP (RFC 4566):
///
/// ```text
/// v=0
/// o=- 0 0 IN IP4 192.168.0.18
/// s=movie.Mjpeg
/// t=0 0
/// a=range:npt=0-25.000          длительность, с
/// m=video 0 RTP/AVP 26           видео по RTP, тип нагрузки 26
/// b=AS:1366                      средний битрейт, кбит/с
/// a=rtpmap:26 JPEG/90000         кодек и частота RTP-часов
/// a=framerate:20
/// a=x-dimensions:384,288         ширина, высота
/// a=x-frames:500                 число кадров
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaInfo {
    pub payload_type: u8,
    pub encoding: String,
    /// Частота RTP-часов, Гц.
    pub clock_rate: u32,
    pub frame_rate: u32,
    pub width: u16,
    pub height: u16,
    pub frames: u32,
    /// Средний битрейт видео, бит/с.
    pub bitrate: u64,
}

impl MediaInfo {
    pub const CONTENT_TYPE: &str = "application/sdp";

    /// Длительность видео, с.
    pub fn duration(&self) -> f64 {
        if self.frame_rate == 0 {
            0.0
        } else {
            f64::from(self.frames) / f64::from(self.frame_rate)
        }
    }

    pub fn to_sdp(&self, file_name: &str, server_ip: IpAddr) -> String {
        let ip_version = if server_ip.is_ipv4() { "IP4" } else { "IP6" };
        format!(
            "v=0\n\
             o=- 0 0 IN {ip_version} {server_ip}\n\
             s={file_name}\n\
             t=0 0\n\
             a=range:npt=0-{duration:.3}\n\
             m=video 0 RTP/AVP {pt}\n\
             b=AS:{kbps}\n\
             a=rtpmap:{pt} {encoding}/{clock_rate}\n\
             a=framerate:{frame_rate}\n\
             a=x-dimensions:{width},{height}\n\
             a=x-frames:{frames}\n",
            duration = self.duration(),
            pt = self.payload_type,
            kbps = self.bitrate / 1000,
            encoding = self.encoding,
            clock_rate = self.clock_rate,
            frame_rate = self.frame_rate,
            width = self.width,
            height = self.height,
            frames = self.frames,
        )
    }

    pub fn from_sdp(sdp: &str) -> Option<Self> {
        let mut info = Self::default();
        let mut has_video = false;
        for line in sdp.lines() {
            let line = line.trim();
            if let Some(media) = line.strip_prefix("m=video ") {
                info.payload_type = media.split_whitespace().nth(2)?.parse().ok()?;
                has_video = true;
            } else if let Some(kbps) = line.strip_prefix("b=AS:") {
                info.bitrate = kbps.parse::<u64>().ok()? * 1000;
            } else if let Some(rtpmap) = line.strip_prefix("a=rtpmap:") {
                let (_, codec) = rtpmap.split_once(' ')?;
                let (encoding, clock_rate) = codec.split_once('/')?;
                info.encoding = encoding.to_owned();
                info.clock_rate = clock_rate.parse().ok()?;
            } else if let Some(rate) = line.strip_prefix("a=framerate:") {
                info.frame_rate = rate.parse().ok()?;
            } else if let Some(size) = line.strip_prefix("a=x-dimensions:") {
                let (width, height) = size.split_once(',')?;
                info.width = width.parse().ok()?;
                info.height = height.parse().ok()?;
            } else if let Some(frames) = line.strip_prefix("a=x-frames:") {
                info.frames = frames.parse().ok()?;
            }
        }
        has_video.then_some(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdp_roundtrip() {
        let info = MediaInfo {
            payload_type: 26,
            encoding: "JPEG".into(),
            clock_rate: 90000,
            frame_rate: 20,
            width: 384,
            height: 288,
            frames: 500,
            bitrate: 1_366_000,
        };
        let sdp = info.to_sdp("movie.Mjpeg", "192.168.0.18".parse().unwrap());
        assert!(sdp.contains("a=range:npt=0-25.000\n"));
        assert!(sdp.contains("a=rtpmap:26 JPEG/90000\n"));
        assert_eq!(MediaInfo::from_sdp(&sdp), Some(info));
    }
}
