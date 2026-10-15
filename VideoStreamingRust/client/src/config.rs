/// Параметры запуска клиента.
pub struct Config {
    pub server_addr: String,
    pub server_port: u16,
    pub rtp_port: u16,
    pub file_name: String,
}

impl Config {
    /// Разбираем аргументы: `Server_name Server_port RTP_port Video_file`.
    pub fn from_args(mut args: impl Iterator<Item = String>) -> Option<Self> {
        Some(Self {
            server_addr: args.next()?,
            server_port: args.next()?.parse().ok()?,
            rtp_port: args.next()?.parse().ok()?,
            file_name: args.next()?,
        })
    }
}
