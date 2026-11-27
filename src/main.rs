mod app;
mod dictionary;
mod event;
mod message;
mod transmission;

use app::App;
use color_eyre::eyre::bail;

use crate::{
    dictionary::{Dictionary, TMFDSDictionary},
    event::EventHandler,
    transmission::CallSign,
};

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
};

use clap::Parser;

#[derive(Parser)]
#[command(name = "DSCR 1212 Client")]
#[command(version = "0.1")]
#[command(about = "A client for the Deep Space Communication Relay for the terminal.", long_about = None)]
struct DSCRArgs {
    /// Call sign to log in with
    #[arg(short, long, value_parser = call_sign_parser)]
    call_sign: Option<CallSign>,

    /// Path to the dictionary to use
    #[arg(short, long, value_parser = dictionary_parser)]
    dictionary: Option<Dictionary>,

    /// Websocket address to connect to
    #[arg(short, long, default_value_t = "wss://dscr-relay.dixonary.co.uk".to_owned())]
    websocket_address: String,
}

fn call_sign_parser(s: &str) -> color_eyre::Result<CallSign> {
    let id = u16::from_str_radix(s, 8)?;
    if !(0..=0o7777).contains(&id) {
        bail!("call sign not in range from 0000 to 7777")
    }
    Ok(CallSign::new(id))
}

fn dictionary_parser(s: &str) -> color_eyre::Result<Dictionary> {
    let source = std::fs::read_to_string(s)?;
    let dictionary: TMFDSDictionary = serde_json::from_str(&source)?;
    Ok(dictionary.into())
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let dscr_args = DSCRArgs::parse();

    let (ws_stream, _response) =
        tokio_tungstenite::connect_async(&dscr_args.websocket_address).await?;

    let mut terminal = ratatui::init();
    execute!(std::io::stderr(), EnableBracketedPaste, EnableMouseCapture)?;

    let event_handler = EventHandler::new(ws_stream);
    let mut app = App::new(
        dscr_args.websocket_address,
        dscr_args
            .call_sign
            .unwrap_or_else(|| CallSign::new(rand::random_range(0..=0o7777))),
        dscr_args.dictionary.unwrap_or_default(),
        event_handler,
    );
    app.run(&mut terminal).await?;

    execute!(
        std::io::stderr(),
        DisableBracketedPaste,
        DisableMouseCapture,
    )?;
    ratatui::restore();

    Ok(())
}
