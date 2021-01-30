use tokio::net::TcpStream;

pub struct Connection {
    stream: TcpStream,
    open: std::sync::atomic::AtomicBool,
}

impl Connection {
    pub fn new(tcp_con: TcpStream) -> Connection {
        Connection {
            stream: tcp_con,
            open: std::sync::atomic::AtomicBool::new(true),
        }
    }

    #[inline(always)]
    pub fn is_open(&self) -> bool {
        self.open.load(std::sync::atomic::Ordering::SeqCst)
    }
    #[inline(always)]
    pub fn close(&self) {
        self.open.store(false, std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.stream.readable().await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        self.stream.try_read(buf)
    }

    pub async fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
        match self.stream.writable().await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        self.stream.try_write(buf)
    }
}
