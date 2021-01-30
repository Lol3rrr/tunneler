use crate::Connection as CustomConnection;

use mobc::{async_trait, Manager};

pub struct ConnectionManager {
    out_url: String,
}

impl ConnectionManager {
    pub fn new(out_ip: &str, out_port: u32) -> ConnectionManager {
        let dest_ip = format!("{}:{}", out_ip, out_port);

        ConnectionManager { out_url: dest_ip }
    }
}

#[async_trait]
impl Manager for ConnectionManager {
    type Connection = CustomConnection;
    type Error = std::io::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let std_stream = std::net::TcpStream::connect(&self.out_url)?;
        std_stream.set_nonblocking(true)?;

        let raw_con = tokio::net::TcpStream::from_std(std_stream)?;
        let con = CustomConnection::new(raw_con);

        Ok(con)
    }

    async fn check(&self, conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        let open = conn.is_open();
        println!("Con is still open: {}", open);
        match open {
            true => Ok(conn),
            false => Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset)),
        }
    }

    fn validate(&self, conn: &mut Self::Connection) -> bool {
        conn.is_open()
    }
}
