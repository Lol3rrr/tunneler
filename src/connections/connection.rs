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

    pub async fn read_raw(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.stream.readable().await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        self.stream.try_read(buf)
    }

    pub async fn write_raw(&self, buf: &[u8]) -> std::io::Result<usize> {
        match self.stream.writable().await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        self.stream.try_write(buf)
    }

    pub async fn drain(&self, size: usize) -> std::io::Result<()> {
        let mut left_to_drain = size;

        let mut buffer = vec![0; size];
        while left_to_drain > 0 {
            match self.read_raw(&mut buffer[0..left_to_drain]).await {
                Ok(0) => {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    left_to_drain -= n;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }

        Ok(())
    }

    pub async fn write_total(&self, data: &[u8], length: usize) -> std::io::Result<()> {
        let mut offset = 0;
        let mut left_to_send = length;

        while left_to_send > 0 {
            match self.write_raw(&data[offset..offset + left_to_send]).await {
                Ok(0) => {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    offset += n;
                    left_to_send -= n;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }

        Ok(())
    }

    pub async fn read_total(&self, data: &mut [u8], length: usize) -> std::io::Result<()> {
        let mut offset = 0;
        let mut left_to_read = length;

        while left_to_read > 0 {
            match self
                .read_raw(&mut data[offset..offset + left_to_read])
                .await
            {
                Ok(0) => {
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    offset += n;
                    left_to_read -= n;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }

        Ok(())
    }

    /// This method reads the Data from connection and simply forwards it in the same chunks
    async fn unbuffered_forward(
        &self,
        header: &MessageHeader,
        out: std::sync::Arc<Self>,
    ) -> std::io::Result<()> {
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
            match self.read_raw(&mut read_buf).await {
                Ok(0) => {
                    error!("[{}] Read 0 Bytes", header.get_id());
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    left_to_read -= n;
                    debug!("[{}] Read {} Bytes", header.get_id(), n);

                    match out.write_total(&read_buf, n).await {
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

        Ok(())
    }
    /// This method waits until it has received the entire Data and then forwards it all in
    /// one big message
    async fn buffered_forward(
        &self,
        header: &MessageHeader,
        out: std::sync::Arc<Self>,
    ) -> std::io::Result<()> {
        let total_length = header.get_length() as usize;
        let mut message_buffer: Vec<u8> = vec![0; total_length];

        let mut offset = 0;
        let mut left_to_read = total_length;

        while left_to_read > 0 {
            debug!(
                "[{}] {} Bytes out of {} left to read",
                header.get_id(),
                left_to_read,
                total_length
            );

            match self
                .read_raw(&mut message_buffer[offset..offset + left_to_read])
                .await
            {
                Ok(0) => {
                    error!("[{}] Read 0 Bytes", header.get_id());
                    return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
                }
                Ok(n) => {
                    offset += n;
                    left_to_read -= n;
                    debug!("[{}] Read {} Bytes", header.get_id(), n);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            };
        }

        debug!("[{}] Done buffering message", header.get_id());

        match out.write_total(&message_buffer, total_length).await {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        };

        debug!("[{}] Done forwarding", header.get_id());

        Ok(())
    }

    pub async fn forward_to_connection(
        &self,
        header: &MessageHeader,
        out: std::sync::Arc<Self>,
    ) -> std::io::Result<()> {
        let buffer_response = true;

        if buffer_response {
            self.buffered_forward(header, out).await
        } else {
            self.unbuffered_forward(header, out).await
        }
    }
}
