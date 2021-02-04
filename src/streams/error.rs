#[derive(Debug, PartialEq)]
pub enum SendError {
    Full,
    Closed,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            SendError::Full => write!(f, "The Channel is full"),
            SendError::Closed => write!(f, "The Channel has been closed"),
        }
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for SendError {
    fn from(raw: tokio::sync::mpsc::error::SendError<T>) -> SendError {
        let try_error = tokio::sync::mpsc::error::TrySendError::from(raw);
        match try_error {
            tokio::sync::mpsc::error::TrySendError::Full(_) => SendError::Full,
            tokio::sync::mpsc::error::TrySendError::Closed(_) => SendError::Closed,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum RecvError {
    Closed,
}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            RecvError::Closed => write!(f, "The Channel has been closed"),
        }
    }
}
