pub struct Destination {
    ip: String,
    port: u32,
    formatted: String,
}

impl Destination {
    pub fn new(ip: String, port: u32) -> Self {
        let formatted_ip = format!("{}:{}", ip, port);

        Destination {
            ip,
            port,
            formatted: formatted_ip,
        }
    }

    pub async fn connect(&self) -> std::io::Result<tokio::net::TcpStream> {
        let stream = tokio::net::TcpStream::connect(&self.formatted).await?;

        Ok(stream)
    }

    pub fn get_full_address(&self) -> &str {
        &self.formatted
    }

    pub fn get_ip(&self) -> &str {
        &self.ip
    }
    pub fn get_port(&self) -> u32 {
        self.port
    }
}

#[test]
fn new_dest_ip() {
    let dest = Destination::new("localhost".to_owned(), 123);
    assert_eq!("localhost", dest.get_ip());
}

#[test]
fn new_dest_port() {
    let dest = Destination::new("localhost".to_owned(), 123);
    assert_eq!(123, dest.get_port());
}

#[test]
fn new_dest_formatted() {
    let dest = Destination::new("localhost".to_owned(), 123);
    assert_eq!("localhost:123", dest.get_full_address());
}
