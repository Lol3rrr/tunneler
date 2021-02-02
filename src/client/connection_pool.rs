use crate::Connection;

pub struct Pool {
    addr: String,
}

impl Pool {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub async fn get_con(&self) -> std::io::Result<std::sync::Arc<Connection>> {
        let std_stream = std::net::TcpStream::connect(&self.addr)?;
        std_stream.set_nonblocking(true)?;

        let raw_con = tokio::net::TcpStream::from_std(std_stream)?;
        let con = Connection::new(raw_con);

        Ok(std::sync::Arc::new(con))
    }
}
