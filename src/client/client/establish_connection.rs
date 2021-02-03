use crate::{Connection, Message, MessageHeader, MessageType};

use rsa::{BigUint, PaddingScheme, PublicKey, RSAPublicKey};

use log::error;

// The validation flow is like this
//
// 1. Client connects
// 2. Server generates and sends public key
// 3. Client sends encrypted password/key
// 4. Server decrypts the message and checks if the password/key is valid
// 5a. If valid: Server sends an Acknowledge message and its done
// 5b. If invalid: Server closes the connection
pub async fn establish_connection(adr: &str, key: &[u8]) -> Option<std::sync::Arc<Connection>> {
    let connection = match tokio::net::TcpStream::connect(&adr).await {
        Ok(c) => c,
        Err(e) => {
            error!("Establishing-Connection: {}", e);
            return None;
        }
    };
    let connection_arc = std::sync::Arc::new(Connection::new(connection));

    // Step 2 - Receive
    let mut head_buf = [0; 13];
    let header = match connection_arc.read_total(&mut head_buf, 13).await {
        Ok(_) => {
            let msg = MessageHeader::deserialize(head_buf);
            msg.as_ref()?;
            msg.unwrap()
        }
        Err(e) => {
            error!("Reading Message-Header: {}", e);
            return None;
        }
    };
    if *header.get_kind() != MessageType::Key {
        return None;
    }

    let mut key_buf = [0; 4092];
    let mut recv_pub_key = match connection_arc.read_raw(&mut key_buf).await {
        Ok(0) => {
            return None;
        }
        Ok(n) => key_buf[0..n].to_vec(),
        Err(e) => {
            error!("Reading Public-Key from Server: {}", e);
            return None;
        }
    };

    let e_bytes = recv_pub_key.split_off(256);
    let n_bytes = recv_pub_key;

    let pub_key = RSAPublicKey::new(
        BigUint::from_bytes_le(&n_bytes),
        BigUint::from_bytes_le(&e_bytes),
    )
    .expect("Could not create Public-Key");

    let encrypted_key = pub_key
        .encrypt(&mut rand::rngs::OsRng, PaddingScheme::PKCS1v15Encrypt, key)
        .expect("Could not encrypt Key");

    let msg_header = MessageHeader::new(0, MessageType::Verify, encrypted_key.len() as u64);
    let msg = Message::new(msg_header, encrypted_key);

    match connection_arc.write_raw(&msg.serialize()).await {
        Ok(_) => {}
        Err(e) => {
            error!("Sending Encrypted Key/Password: {}", e);
            return None;
        }
    };

    let mut buf = [0; 13];
    let header = match connection_arc.read_total(&mut buf, 13).await {
        Ok(_) => match MessageHeader::deserialize(buf) {
            Some(c) => c,
            None => {
                return None;
            }
        },
        Err(e) => {
            error!("Reading response: {}", e);
            return None;
        }
    };

    if *header.get_kind() != MessageType::Acknowledge {
        return None;
    }

    Some(connection_arc)
}
