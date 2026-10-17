//! The app state (Elm architecture): all state in [`Model`], changed only by [`update`]; side effects
//! are returned as [`Cmd`] values for the runtime to perform, and their results come back as [`Msg`].

mod model;
mod update;

pub use model::{
    Cmd, Confirm, Key, LIST_LEFT, LIST_TOP, MIN_H, MIN_W, Model, Mouse, Msg, TREND_LEN, list_rows,
};
pub use update::update;
