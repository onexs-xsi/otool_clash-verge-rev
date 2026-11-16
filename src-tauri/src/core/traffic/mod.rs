mod accounting;
mod collector;
pub(crate) mod commands;
mod database;
pub(crate) mod edition;
mod icon;
mod process;
mod wsl;

pub(crate) use collector::flush;

pub(crate) fn start() {
    collector::start();
    wsl::start();
}
