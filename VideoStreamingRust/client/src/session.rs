use common::{Method, State};

/// Состояние RTSP-сеанса на стороне клиента.
#[derive(Default)]
pub struct Session {
    pub state: State,
    /// CSeq последнего отправленного запроса.
    pub cseq: u32,
    /// Идентификатор сеанса из ответа на SETUP (0 — еще не получен).
    pub id: u32,
    /// Метод последнего отправленного запроса.
    pub request_sent: Option<Method>,
}
