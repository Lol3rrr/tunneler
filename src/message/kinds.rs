#[derive(Debug, PartialEq)]
pub enum MessageType {
    Connect,
    Close,
    Data,
    Heartbeat,
    Establish,
    Acknowledge,
}

impl MessageType {
    pub fn deserialize(data: u8) -> Option<MessageType> {
        match data {
            0 => Some(MessageType::Connect),
            1 => Some(MessageType::Close),
            2 => Some(MessageType::Data),
            3 => Some(MessageType::Heartbeat),
            4 => Some(MessageType::Establish),
            5 => Some(MessageType::Acknowledge),
            _ => None,
        }
    }

    pub fn serialize(&self) -> u8 {
        match *self {
            MessageType::Connect => 0,
            MessageType::Close => 1,
            MessageType::Data => 2,
            MessageType::Heartbeat => 3,
            MessageType::Establish => 4,
            MessageType::Acknowledge => 5,
        }
    }
}

#[test]
fn message_type_deserialize_connect() {
    assert_eq!(Some(MessageType::Connect), MessageType::deserialize(0));
}
#[test]
fn message_type_deserialize_close() {
    assert_eq!(Some(MessageType::Close), MessageType::deserialize(1));
}
#[test]
fn message_type_deserialize_data() {
    assert_eq!(Some(MessageType::Data), MessageType::deserialize(2));
}
#[test]
fn message_type_deserialize_heartbeat() {
    assert_eq!(Some(MessageType::Heartbeat), MessageType::deserialize(3));
}
#[test]
fn message_type_deserialize_establish() {
    assert_eq!(Some(MessageType::Establish), MessageType::deserialize(4));
}
#[test]
fn message_type_deserialize_acknowledge() {
    assert_eq!(Some(MessageType::Acknowledge), MessageType::deserialize(5));
}
#[test]
fn message_type_deserialize_invalid() {
    assert_eq!(None, MessageType::deserialize(123));
}

#[test]
fn message_type_serialize_connect() {
    assert_eq!(0, MessageType::Connect.serialize());
}
#[test]
fn message_type_serialize_close() {
    assert_eq!(1, MessageType::Close.serialize());
}
#[test]
fn message_type_serialize_data() {
    assert_eq!(2, MessageType::Data.serialize());
}
#[test]
fn message_type_serialize_heartbeat() {
    assert_eq!(3, MessageType::Heartbeat.serialize());
}
#[test]
fn message_type_serialize_establish() {
    assert_eq!(4, MessageType::Establish.serialize());
}
#[test]
fn message_type_serialize_acknowledge() {
    assert_eq!(5, MessageType::Acknowledge.serialize());
}
