mod app;
mod dictionary;
mod transmission;

use app::App;

use crate::dictionary::{Dictionary, TMFDSDictionary};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let source = std::fs::read_to_string("test-dictionary.save")?;
    let dictionary: TMFDSDictionary = serde_json::from_str(&source)?;
    let dictionary = Dictionary::from(dictionary);

    ratatui::run(|terminal| App::with_dictionary(dictionary).run(terminal))?;

    Ok(())
}
