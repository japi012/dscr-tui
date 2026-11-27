use ratatui::{palette::Hsl, style::Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Signal {
    Number(u64),
    Word(i64),
}

impl Signal {
    pub fn from_number(n: i64) -> Self {
        if n < 0 {
            Self::Word(n)
        } else {
            Self::Number(n as u64)
        }
    }

    pub fn as_number(self) -> i64 {
        match self {
            Self::Word(n) => n,
            Self::Number(n) => n as i64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallSign {
    Valid(u16),
    Invalid,
}

impl CallSign {
    pub fn new(id: u16) -> Self {
        if id <= 0o7777 {
            Self::Valid(id)
        } else {
            Self::Invalid
        }
    }

    fn hue(self) -> f32 {
        match self {
            Self::Valid(i) => (137.5 * i as f32) % 360.0,
            Self::Invalid => 0.0,
        }
    }

    pub fn color(self) -> Color {
        Color::from_hsl(Hsl::from_components((self.hue(), 1.0, 0.7)))
    }
}

impl std::fmt::Display for CallSign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Valid(i) => write!(f, "{i:04o}"),
            Self::Invalid => write!(f, "????"),
        }
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
