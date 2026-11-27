use super::transmission::{CallSign, Signal, Transmission};

#[derive(Debug, Clone)]
pub enum DSCRMessage {
    CallSignOk(CallSign),           // K
    CallSignInUse,                  // U
    ActiveCallSigns(Vec<CallSign>), // C
    ReceivedMessage(Transmission),  // R
    ReconnectionOk,                 // E
}

impl DSCRMessage {
    pub fn from_str(source: &str) -> Option<DSCRMessage> {
        let mut contents = source.split(',');
        let key = contents.next()?;
        match key {
            "K" => {
                let call_sign = contents.next()?;
                let call_sign = CallSign::new(call_sign.parse().ok()?);
                Some(DSCRMessage::CallSignOk(call_sign))
            }
            "U" => Some(DSCRMessage::CallSignInUse),
            "C" => {
                let call_signs = contents
                    .map(|c| Some(CallSign::new(c.parse().ok()?)))
                    .collect::<Option<_>>()?;
                Some(DSCRMessage::ActiveCallSigns(call_signs))
            }
            "R" => {
                let sender = CallSign::new(contents.next()?.parse().ok()?);
                let sequence = contents.next()?.parse().ok()?;
                let message = contents
                    .map(|c| Some(Signal::from_number(c.parse().ok()?)))
                    .collect::<Option<_>>()?;

                Some(DSCRMessage::ReceivedMessage(Transmission::new(
                    sequence, sender, message,
                )))
            }
            "E" => Some(DSCRMessage::ReconnectionOk),
            _ => None,
        }
    }
}
