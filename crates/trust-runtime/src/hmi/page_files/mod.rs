//! HMI Builder page files (specification 39, §15): the page model, loading and validation.
//!
//! The format is implemented once, here. The runtime uses it directly; the editor reaches it
//! through the language server.

mod arrange;
mod regions;
pub use regions::HmiRegion;
mod attachments;
pub mod colors;
mod control_keys;
mod docked;
mod faceplates;
mod inline_faceplates;
mod limits;
mod lines;
mod load;
pub mod page_list;
mod pairs;
mod pipes;
mod popups;
mod texts;
mod types;
mod write;

#[cfg(test)]
mod arrange_tests;
#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
mod capability_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod faceplate_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod inline_faceplate_tests;
#[cfg(test)]
mod limit_tests;
#[cfg(test)]
mod page_list_tests;
#[cfg(test)]
mod pid_tests;
#[cfg(test)]
mod pipe_tests;
#[cfg(test)]
mod screen_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texts_tests;
#[cfg(test)]
mod write_tests;

pub use limits::HmiLimits;

pub use docked::{compose_docked, HmiDock, HmiDockEdge, HmiDockScope};
pub use faceplates::{parse_faceplate, write_faceplate, HmiFaceplate, HmiField};
pub use inline_faceplates::PLACED_FACEPLATE;
pub use lines::{click_line, object_key_line, problem_line};
pub(crate) use load::{error_at, line_at};
pub use load::{parse_page, parse_screen, PageFileError, SUPPORTED_FORMAT};
pub use pairs::{
    beside_instance, is_pair_member, pair_member_path, parse_pair, write_pair, HmiCallPosition,
    HmiChoice, HmiDependency, HmiPair, HmiPairProgram, HmiSetting, HmiTerminal,
    HmiTerminalDirection, INSTANCE_PLACEHOLDER,
};
pub use popups::parse_popup;
pub use texts::{is_language_tag, parse_texts, write_texts};
pub use types::{
    HmiArrange, HmiBinding, HmiBindingDirection, HmiButtonAction, HmiClick, HmiColors,
    HmiNavigation, HmiNavigationPosition, HmiNavigationStyle, HmiObject, HmiPage, HmiPipe,
    HmiPipeStyle, HmiPopup, HmiRole, HmiScalar, HmiScreen, HmiSymbolStyle, HmiTexts, HmiTheme,
    HmiTopBar, MODE_NAMES,
};
pub(crate) use write::string as toml_string;
pub use write::{write_page, write_popup, write_screen};
