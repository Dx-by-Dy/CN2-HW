use common::{Method, State};

/// Состояние RTSP-сеанса на стороне клиента.
#[derive(Default)]
pub struct Session {
    pub state: State,
    /// CSeq последнего отправленного запроса (растет на протяжении всего TCP-соединения).
    pub cseq: u32,
    /// Идентификатор сеанса из ответа на SETUP (0 — сеанса нет).
    pub id: u32,
    /// Метод последнего отправленного запроса.
    pub request_sent: Option<Method>,
    /// Ждем ответа на последний запрос: новые запросы до ответа не отправляем.
    pub awaiting_reply: bool,
    /// PLAY нажали без сеанса: после ответа на SETUP сразу отправить PLAY.
    pub play_after_setup: bool,
}
