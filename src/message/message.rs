use crate::MessageHeader;

#[cfg(test)]
use crate::MessageType;

#[derive(Debug, PartialEq, Clone)]
pub struct Message {
    header: MessageHeader,
    data: Vec<u8>, // X bytes
}

impl Message {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Message {
        Message { header, data }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let data = &self.data;
        let data_length = self.header.get_length() as usize;

        let mut output = vec![0; 13 + data_length];

        let header = self.header.serialize();

        output[0..13].copy_from_slice(&header);
        let copy_end = 13 + data_length;
        output[13..copy_end].copy_from_slice(&data[0..data_length]);

        output
    }

    pub fn get_header(&self) -> &MessageHeader {
        &self.header
    }
    pub fn get_data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

#[test]
fn message_serialize_connect() {
    let mut inner_data = vec![0; 4092];
    inner_data[0] = 1;
    inner_data[1] = 1;
    let output = Message {
        header: MessageHeader {
            id: 13,
            kind: MessageType::Connect,
            length: 2,
        },
        data: inner_data,
    }
    .serialize();

    let mut expected = vec![0; 15];
    expected[0] = 13;
    expected[5] = 2;
    expected[13] = 1;
    expected[14] = 1;
    assert_eq!(expected, output);
}
#[test]
fn message_serialize_data() {
    let inner_data = vec![0; 4092];
    let mut msg = Message {
        header: MessageHeader {
            id: 13,
            kind: MessageType::Data,
            length: 12,
        },
        data: inner_data,
    };
    msg.data[2] = 33;
    let output = msg.serialize();

    let mut expected = vec![0; 25];
    expected[0] = 13;
    expected[4] = 2;
    expected[5] = 12;
    expected[15] = 33;
    assert_eq!(expected, output);
}
