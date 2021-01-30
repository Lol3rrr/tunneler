use std::convert::TryInto;

use crate::message::MessageType;

#[derive(Debug, PartialEq)]
pub struct MessageHeader {
    pub id: u32,           // 4 bytes
    pub kind: MessageType, // 1 byte
    pub length: u64,       // 8 bytes
}

impl MessageHeader {
    pub fn new(id: u32, kind: MessageType, length: u64) -> MessageHeader {
        MessageHeader { id, kind, length }
    }

    pub fn deserialize(raw_data: [u8; 13]) -> Option<MessageHeader> {
        let id_part = &raw_data[0..4];
        let kind_part = raw_data[4];
        let length_part = &raw_data[5..13];

        let id = u32::from_le_bytes(id_part.try_into().unwrap());
        let kind = MessageType::deserialize(kind_part);
        if kind.is_none() {
            return None;
        }
        let kind = kind.unwrap();
        let length = u64::from_le_bytes(length_part.try_into().unwrap());

        Some(MessageHeader { id, kind, length })
    }
    pub fn serialize(&self) -> [u8; 13] {
        let mut output = [0; 13];

        let id = self.id.to_le_bytes();
        let kind = self.kind.serialize();
        let length = self.length.to_le_bytes();

        output[0] = id[0];
        output[1] = id[1];
        output[2] = id[2];
        output[3] = id[3];

        output[4] = kind;

        output[5] = length[0];
        output[6] = length[1];
        output[7] = length[2];
        output[8] = length[3];
        output[9] = length[4];
        output[10] = length[5];
        output[11] = length[6];
        output[12] = length[7];

        output
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }
    pub fn get_kind(&self) -> &MessageType {
        &self.kind
    }
    pub fn get_length(&self) -> u64 {
        self.length
    }
}

#[test]
fn message_header_deserialize_connect() {
    let mut input = [0; 13];
    input[0] = 13;
    input[4] = 0;
    input[5] = 20;
    assert_eq!(
        Some(MessageHeader {
            id: 13,
            kind: MessageType::Connect,
            length: 20,
        }),
        MessageHeader::deserialize(input)
    );
}
