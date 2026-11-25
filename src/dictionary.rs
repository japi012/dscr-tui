use std::collections::HashMap;

use thiserror::Error;

use bimap::BiHashMap;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::transmission::Signal;

#[derive(
    Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
enum FormatMode {
    #[default]
    NoSpace = 0,
    Space,
    Line,
    TwoLinen,
}

impl FormatMode {
    fn as_str(self) -> &'static str {
        match self {
            FormatMode::NoSpace => "",
            FormatMode::Space => " ",
            FormatMode::Line => "\n",
            FormatMode::TwoLinen => "\n\n",
        }
    }

    fn combine(self, other: Self) -> Self {
        self.max(other)
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
struct WordDict {
    keys: Vec<i64>,
    values: Vec<String>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
struct DescDict {
    keys: Vec<i64>,
    values: Vec<Description>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Description {
    desc: String,
    format_mode: FormatMode,
    format_mode_after: FormatMode,
    break_on_double: bool,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TMFDSDictionary {
    word_dict: WordDict,
    desc_dict: DescDict,
    id: u64,
    before_user_default_mode: FormatMode,
    after_user_default_mode: FormatMode,
}

#[derive(Default, Debug, Clone)]
pub struct Dictionary {
    signals: BiHashMap<Signal, String>,
    description: HashMap<Signal, Description>,
}

#[derive(Error, Debug)]
pub enum ParseSignalError {
    #[error("invalid signal `{0}`")]
    InvalidSignal(String),
    #[error("null input")]
    NullInput,
    #[error("integer overflow")]
    IntegerOverflow,
}

impl Dictionary {
    fn get_name(&self, signal: Signal) -> String {
        match signal {
            Signal::Word(freq) => self
                .signals
                .get_by_left(&signal)
                .map(|s| s.clone())
                .unwrap_or(format!("|{freq}")),
            Signal::Number(n) => n.to_string(),
        }
    }

    fn get_signal(&self, name: &str) -> Option<Signal> {
        if name.starts_with("|-") {
            let number = (&name[1..]).parse().ok()?;
            Some(if number < 0 {
                Signal::Word(number)
            } else {
                Signal::Number(number as u64)
            })
        } else {
            self.signals.get_by_right(name).copied()
        }
    }

    fn get_description(&self, signal: Signal) -> Option<&Description> {
        self.description.get(&signal)
    }

    fn get_format_mode_before(&self, signal: Signal) -> FormatMode {
        match signal {
            Signal::Word(_) => self
                .get_description(signal)
                .map(|desc| desc.format_mode)
                .unwrap_or(FormatMode::Space),
            Signal::Number(_) => FormatMode::NoSpace,
        }
    }

    fn get_format_mode_after(&self, signal: Signal) -> FormatMode {
        match signal {
            Signal::Word(_) => self
                .get_description(signal)
                .map(|desc| desc.format_mode_after)
                .unwrap_or(FormatMode::Space),
            Signal::Number(_) => FormatMode::NoSpace,
        }
    }

    fn get_break_on_double(&self, signal: Signal) -> bool {
        match signal {
            Signal::Word(_) => self
                .get_description(signal)
                .map(|desc| desc.break_on_double)
                .unwrap_or(false),
            Signal::Number(_) => false,
        }
    }

    pub fn parse_signals(&self, source: &str) -> Result<Vec<Signal>, ParseSignalError> {
        let words = source.split_whitespace();
        let mut signals = Vec::new();

        for word in words {
            let mut starting_position = 0;
            let mut portion = word;

            while starting_position < word.len() {
                loop {
                    if portion.is_empty() {
                        return Err(ParseSignalError::InvalidSignal(String::from(
                            (&word[starting_position..])
                                .split_once(|c: char| c.is_ascii_digit())
                                .map(|(s, _)| s)
                                .unwrap_or(word),
                        )));
                    }

                    if portion.starts_with("|-")
                        && (&portion[2..]).chars().all(|c| c.is_ascii_digit())
                    {
                        let signal = self
                            .get_signal(portion)
                            .ok_or(ParseSignalError::IntegerOverflow)?;
                        signals.push(signal);
                        starting_position += portion.len();
                        portion = &word[starting_position..];
                        break;
                    } else if let Some(signal) = self.get_signal(portion) {
                        signals.push(signal);
                        starting_position += portion.len();
                        portion = &word[starting_position..];
                        break;
                    } else if portion.chars().all(|c| c.is_ascii_digit()) {
                        let number = portion
                            .parse()
                            .map_err(|_| ParseSignalError::IntegerOverflow)?;
                        if let Some(Signal::Number(prev_number)) = signals.iter().last() {
                            *signals.iter_mut().last().unwrap() = Signal::Number(
                                format!("{prev_number}{number}")
                                    .parse()
                                    .map_err(|_| ParseSignalError::IntegerOverflow)?,
                            );
                        } else {
                            signals.push(Signal::Number(number));
                        }
                        starting_position += portion.len();
                        portion = &word[starting_position..];
                        break;
                    }

                    portion = &word[starting_position..starting_position + portion.len() - 1];
                }
            }
        }

        if signals.is_empty() {
            return Err(ParseSignalError::NullInput);
        }

        Ok(signals)
    }

    pub fn format_signals(&self, signals: &[Signal]) -> String {
        if signals.is_empty() {
            return self.get_name(signals[0]);
        }

        let mut output = String::new();
        let mut broke_on_double = false;

        for [signal_a, signal_b] in signals.array_windows() {
            if !broke_on_double {
                output.push_str(&self.get_name(*signal_a));

                let format_mode_a = self.get_format_mode_after(*signal_a);
                let format_mode_b = self.get_format_mode_before(*signal_b);

                output.push_str(format_mode_a.combine(format_mode_b).as_str());
            }

            broke_on_double = false;

            if signal_a == signal_b && self.get_break_on_double(*signal_a) {
                output.push_str(&self.get_name(*signal_b));
                output.push('\n');
                broke_on_double = true;
            }
        }
        if !broke_on_double {
            output.push_str(&self.get_name(*signals.last().unwrap()));
        }
        output
    }
}

impl From<TMFDSDictionary> for Dictionary {
    fn from(
        TMFDSDictionary {
            word_dict,
            desc_dict,
            ..
        }: TMFDSDictionary,
    ) -> Self {
        let WordDict { keys, values } = word_dict;
        let signals: BiHashMap<_, _> = { keys.into_iter().map(Signal::Word) }
            .zip(values.into_iter())
            .collect();

        let DescDict { keys, values } = desc_dict;
        let description: HashMap<_, _> = { keys.into_iter().map(Signal::Word) }
            .zip(values.into_iter())
            .collect();

        Self {
            signals,
            description,
        }
    }
}
