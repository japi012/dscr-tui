use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc, task::JoinHandle};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

use crate::{
    message::DSCRMessage,
    transmission::{CallSign, Signal},
};

#[derive(Clone, Debug)]
pub enum RecievedEvent {
    Error,
    SocketClosed,
    DSCREvent(DSCRMessage),
    CrosstermEvent(crossterm::event::Event),
}

#[derive(Clone, Debug)]
pub enum SentEvent {
    SelectCallSign(CallSign, bool),
    SendMessage(Vec<Signal>),
}

impl From<SentEvent> for Message {
    fn from(value: SentEvent) -> Message {
        match value {
            SentEvent::SelectCallSign(call_sign, retrying) => {
                let CallSign::Valid(id) = call_sign else {
                    unreachable!();
                };

                let text = if retrying {
                    format!("S,{id},0")
                } else {
                    format!("S,{id}")
                };
                Message::Text(text.into())
            }
            SentEvent::SendMessage(message) => Message::Text(
                format!(
                    "M,{}",
                    message
                        .iter()
                        .map(|signal| signal.as_number().to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .into(),
            ),
        }
    }
}

#[derive(Debug)]
pub struct EventHandler {
    _tx: mpsc::UnboundedSender<RecievedEvent>,
    rx: mpsc::UnboundedReceiver<RecievedEvent>,
    ws_tx: mpsc::UnboundedSender<SentEvent>,
    _task: Option<JoinHandle<color_eyre::Result<()>>>,
}

impl EventHandler {
    pub fn new(websocket: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = tx.clone();

        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<SentEvent>();

        let (mut ws_write, mut ws_read) = websocket.split();

        let task = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();

            loop {
                tokio::select! {
                    crossterm_event = reader.next() => {
                        match crossterm_event {
                            Some(Ok(evt)) => {
                                tx.send(RecievedEvent::CrosstermEvent(evt))?;
                            }
                            Some(Err(_)) => {
                                tx.send(RecievedEvent::Error)?;
                            }
                            None => {}
                        }
                    }

                    dscr_event = ws_read.next() => {
                        match dscr_event {
                            Some(Ok(Message::Ping(_value))) => {}

                            Some(Ok(Message::Close(_closed))) => {
                                tx.send(RecievedEvent::SocketClosed)?;
                            }

                            Some(Ok(Message::Text(bytes))) => {
                                let msg_string = bytes.to_string();

                                if let Some(dscr_msg) = DSCRMessage::from_str(&msg_string) {
                                    tx.send(RecievedEvent::DSCREvent(dscr_msg))?;
                                }
                            }

                            Some(Ok(_)) => {}

                            Some(Err(err)) => {
                                return Err(err.into());
                            }

                            None => break,
                        }
                    }

                    outgoing = ws_rx.recv() => {
                        match outgoing {
                            Some(msg) => {
                                ws_write.send(msg.try_into()?).await?;
                            }
                            None => break,
                        }
                    }
                }
            }

            Ok(())
        });

        Self {
            _tx,
            rx,
            ws_tx,
            _task: Some(task),
        }
    }

    pub async fn next(&mut self) -> color_eyre::Result<RecievedEvent> {
        self.rx
            .recv()
            .await
            .ok_or(color_eyre::eyre::eyre!("unable to get event"))
    }

    pub fn send(&self, msg: SentEvent) -> color_eyre::Result<()> {
        self.ws_tx
            .send(msg)
            .map_err(|_| color_eyre::eyre::eyre!("websocket writer unavailable"))
    }
}
