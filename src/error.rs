#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    MissingConfig(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::IO(ref cause) => {
                write!(f, "IO-Error: {}", cause)
            }
            Error::MissingConfig(ref cause) => {
                write!(f, "Missing-Configuration: {}", cause)
            }
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Error {
        Error::IO(cause)
    }
}
