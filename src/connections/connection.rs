use crate::MessageHeader;

use tokio::net::TcpStream;

use log::{debug, error};

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

    pub async fn forward_to_connection(
        &self,
        header: &MessageHeader,
        out: std::sync::Arc<Self>,
    ) -> std::io::Result<()> {
        let buffer_response = true;

        let total_length = header.get_length() as usize;
        let mut left_to_read = total_length;

        while left_to_read > 0 {
            debug!(
                "[{}] {} Bytes out of {} left to read",
                header.get_id(),
                left_to_read,
                total_length
            );

            let mut read_buf = vec![0; left_to_read];
            match self.read(&mut read_buf).await {
                Ok(0) => {
                    error!("[{}] Read 0 Bytes", header.get_id());
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    left_to_read -= n;
                    debug!("[{}] Read {} Bytes", header.get_id(), n);

                    match out.write(&read_buf).await {
                        Ok(_) => {}
                        Err(e) => {
                            return Err(e);
                        }
                    };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }

        debug!("[{}] Done forwarding", header.get_id());

        return Ok(());
    }
}
