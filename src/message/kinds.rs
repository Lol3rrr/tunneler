#[derive(Debug, PartialEq, Clone)]
pub enum MessageType {
    Connect,
    Close,
    Data,
    Heartbeat,
    Establish,
    Key,
    Verify,
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
            5 => Some(MessageType::Key),
            6 => Some(MessageType::Verify),
            7 => Some(MessageType::Acknowledge),
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
            MessageType::Key => 5,
            MessageType::Verify => 6,
            MessageType::Acknowledge => 7,
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
fn message_type_deserialize_key() {
    assert_eq!(Some(MessageType::Key), MessageType::deserialize(5));
}
#[test]
fn message_type_deserialize_verify() {
    assert_eq!(Some(MessageType::Verify), MessageType::deserialize(6));
}
#[test]
fn message_type_deserialize_acknowledge() {
    assert_eq!(Some(MessageType::Acknowledge), MessageType::deserialize(7));
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
fn message_type_serialize_key() {
    assert_eq!(5, MessageType::Key.serialize());
}
#[test]
fn message_type_serialize_verify() {
    assert_eq!(6, MessageType::Verify.serialize());
}
#[test]
fn message_type_serialize_acknowledge() {
    assert_eq!(7, MessageType::Acknowledge.serialize());
}
