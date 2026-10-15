use std::fmt;
use std::str::FromStr;

/// Метод RTSP-запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Setup,
    Play,
    Pause,
    Teardown,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Setup => "SETUP",
            Method::Play => "PLAY",
            Method::Pause => "PAUSE",
            Method::Teardown => "TEARDOWN",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Method {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SETUP" => Ok(Method::Setup),
            "PLAY" => Ok(Method::Play),
            "PAUSE" => Ok(Method::Pause),
            "TEARDOWN" => Ok(Method::Teardown),
            _ => Err(()),
        }
    }
}
