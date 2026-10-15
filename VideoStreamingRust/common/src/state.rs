use crate::Method;

/// Состояние RTSP-сеанса (одинаково для клиента и сервера).
///
/// ```text
/// INIT --SETUP-----> READY --PLAY--> PLAYING
/// INIT <-TEARDOWN--- READY <-PAUSE-- PLAYING
/// INIT <-----------TEARDOWN--------- PLAYING
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Init,
    Ready,
    Playing,
}

impl State {
    /// Можно ли выполнить `method` в этом состоянии.
    pub fn allows(self, method: Method) -> bool {
        match method {
            Method::Describe => true,
            Method::Setup => self == State::Init,
            Method::Play => self == State::Ready,
            Method::Pause => self == State::Playing,
            Method::Teardown => self != State::Init,
        }
    }

    /// Состояние после успешного выполнения `method` (DESCRIBE его не меняет).
    pub fn after(self, method: Method) -> State {
        match method {
            Method::Describe => self,
            Method::Setup | Method::Pause => State::Ready,
            Method::Play => State::Playing,
            Method::Teardown => State::Init,
        }
    }
}
