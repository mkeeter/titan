use anyhow::Result;

mod app;
mod command;
mod document;
mod protocol;
mod parser;
mod tofu;
mod view;

use crate::app::App;

////////////////////////////////////////////////////////////////////////////////

fn main() -> Result<()> {
    let dirs = directories::ProjectDirs::from("com", "mkeeter", "titan")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other,
                                           "Could not get ProjectDirs"))?;
    let db = sled::open(dirs.data_dir())?;

    let mut app = App::new(&db)?;
    app.run("gemini.circumlunar.space/docs/specification.gmi".to_string())?;
    Ok(())
}
