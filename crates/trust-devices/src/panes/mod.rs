//! The panes that the panel opens beside the graph (specification 25): their builders answer the
//! pane types of [the view contract](crate::view::types).
//!
//! - `add`: the add dialog, [`AddDialog`](crate::view::types::AddDialog).
//! - `forms`: the forms that the add dialog and the inspectors open,
//!   [`Form`](crate::view::types::Form), and what a submitted form saves.
//! - `usb`: the USB tray, [`UsbTray`](crate::view::types::UsbTray).
//! - `browse`: the browse panes, [`BrowseView`](crate::view::types::BrowseView).

pub(crate) mod add;
pub(crate) mod browse;
pub mod forms;
mod usb;

#[cfg(test)]
mod tests;
