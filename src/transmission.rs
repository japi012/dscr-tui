struct Signal(i64);

struct Message {
    contents: Vec<Signal>,
}

struct CallSign(u16);

struct Transmission {
    id: u16,
    sender: CallSign,
    message: Message,
}
