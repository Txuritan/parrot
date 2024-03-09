pub mod autopause;
pub mod clear;
pub mod leave;
pub mod manage_sources;
pub mod now_playing;
pub mod pause;
pub mod play;
pub mod queue;
pub mod remove;
pub mod repeat;
pub mod resume;
pub mod seek;
pub mod shuffle;
pub mod skip;
pub mod stop;
pub mod summon;
pub mod version;
pub mod volume;
pub mod voteskip;

pub use self::{
    autopause::*, clear::*, leave::*, manage_sources::*, now_playing::*, pause::*, play::*,
    queue::*, remove::*, repeat::*, resume::*, seek::*, shuffle::*, skip::*, stop::*, summon::*,
    version::*, volume::*, voteskip::*,
};
