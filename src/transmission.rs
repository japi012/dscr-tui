#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Signal {
    Number(u64),
    Word(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallSign {
    id: u16,
}

impl CallSign {
    pub fn new(id: u16) -> Self {
        debug_assert!(id <= 0o7777, "invalid call sign");
        Self { id }
    }

    pub fn hue(self) -> f64 {
        (137.5 * self.id as f64) % 360.0
    }
}

impl std::fmt::Display for CallSign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04o}", self.id)
    }
}

type TransmissionId = u16;

#[derive(Debug, Clone)]
pub struct Transmission {
    id: TransmissionId,
    sender: CallSign,
    message: Box<[Signal]>,
}

impl Transmission {
    pub fn new(id: TransmissionId, sender: CallSign, message: Box<[Signal]>) -> Self {
        Self {
            id,
            sender,
            message,
        }
    }

    pub fn id(&self) -> TransmissionId {
        self.id
    }

    pub fn sender(&self) -> CallSign {
        self.sender
    }

    pub fn message(&self) -> &[Signal] {
        &self.message
    }
}
