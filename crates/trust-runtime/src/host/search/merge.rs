//! The devices of a search (specification 11, "Devices"). Results belong to one device when they
//! share an address, an AMS Net ID, a USB serial number or a truST runtime ID; a result that links
//! two devices joins them into the one found first. Every change moves the cursor, so that a
//! status read with `since` lists only what changed. Pure: the results carry their own times.

use std::collections::BTreeSet;

use super::model::{DeviceKey, FoundDevice, MergedDevice, ProtocolFind};
use super::presence;

/// The devices found so far.
#[derive(Debug, Default)]
pub(crate) struct Devices {
    entries: Vec<Entry>,
    cursor: u64,
    merged: Vec<(u64, MergedDevice)>,
}

#[derive(Debug)]
struct Entry {
    device: FoundDevice,
    keys: BTreeSet<DeviceKey>,
    changed: u64,
}

/// What adding one result did to its device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Added {
    New,
    Changed,
    Unchanged,
}

impl Devices {
    /// Adds one result to its device; answers whether it is a new result rather than one seen
    /// again.
    pub(crate) fn add(&mut self, find: ProtocolFind) -> bool {
        let keys = device_keys(&find);
        let matching: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.keys.is_disjoint(&keys))
            .map(|(index, _)| index)
            .collect();
        let cursor = self.cursor + 1;
        let target = match matching.first() {
            Some(&first) => first,
            None => {
                self.entries.push(Entry::new(&keys));
                self.entries.len() - 1
            }
        };
        let mut changed = matching.len() != 1;
        // The later devices join the first one, the last first, so the indices stay valid.
        for &other in matching.iter().skip(1).rev() {
            let absorbed = self.entries.remove(other);
            let entry = &mut self.entries[target];
            self.merged.push((
                cursor,
                MergedDevice {
                    id: absorbed.device.id,
                    into: entry.device.id.clone(),
                },
            ));
            entry.keys.extend(absorbed.keys);
            for find in absorbed.device.finds {
                insert_find(&mut entry.device.finds, find);
            }
        }
        let entry = &mut self.entries[target];
        let added_keys = !keys.is_subset(&entry.keys);
        entry.keys.extend(keys);
        let added = insert_find(&mut entry.device.finds, find);
        changed |= added_keys || added != Added::Unchanged;
        if changed {
            entry.refresh();
            entry.changed = cursor;
            self.cursor = cursor;
        }
        added == Added::New
    }

    /// The change counter of the last change.
    pub(crate) fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Every device, or with `since` the devices changed after it and the devices that joined
    /// another after it.
    pub(crate) fn snapshot(&self, since: Option<u64>) -> (Vec<FoundDevice>, Vec<MergedDevice>) {
        let Some(since) = since else {
            let devices = self.entries.iter().map(|entry| entry.device.clone());
            return (devices.collect(), Vec::new());
        };
        let devices = self
            .entries
            .iter()
            .filter(|entry| entry.changed > since)
            .map(|entry| entry.device.clone())
            .collect();
        let merged = self
            .merged
            .iter()
            .filter(|(cursor, _)| *cursor > since)
            .map(|(_, merged)| merged.clone())
            .collect();
        (devices, merged)
    }
}

impl Entry {
    /// A device named by the first of `keys`: an address before an AMS Net ID, a USB serial
    /// number, a truST runtime ID and a bus-local identity.
    fn new(keys: &BTreeSet<DeviceKey>) -> Self {
        let id = keys
            .first()
            .map_or_else(|| "dev:unknown".to_string(), DeviceKey::device_id);
        Self {
            device: FoundDevice {
                id,
                addresses: Vec::new(),
                ams_net_ids: Vec::new(),
                usb_serials: Vec::new(),
                runtime_ids: Vec::new(),
                last_seen_ms: 0,
                finds: Vec::new(),
            },
            keys: BTreeSet::new(),
            changed: 0,
        }
    }

    /// Lists the device's keys and its latest `last_seen` again.
    fn refresh(&mut self) {
        let device = &mut self.device;
        device.addresses.clear();
        device.ams_net_ids.clear();
        device.usb_serials.clear();
        device.runtime_ids.clear();
        for key in &self.keys {
            match key {
                DeviceKey::Address(address) => device.addresses.push(address.clone()),
                DeviceKey::AmsNetId(net_id) => device.ams_net_ids.push(net_id.clone()),
                DeviceKey::UsbSerial(serial) => device.usb_serials.push(serial.clone()),
                DeviceKey::RuntimeId(id) => device.runtime_ids.push(id.clone()),
                DeviceKey::Local(_) => {}
            }
        }
        device.last_seen_ms = device
            .finds
            .iter()
            .map(|find| find.last_seen_ms)
            .max()
            .unwrap_or(0);
    }
}

/// The keys of a result; a result without any forms its own device.
fn device_keys(find: &ProtocolFind) -> BTreeSet<DeviceKey> {
    let mut keys: BTreeSet<DeviceKey> = find
        .keys
        .iter()
        .filter(|key| !key_text(key).is_empty())
        .cloned()
        .collect();
    if keys.is_empty() {
        keys.insert(DeviceKey::Local(format!(
            "{}:{}",
            find.protocol.as_str(),
            find.key
        )));
    }
    keys
}

fn key_text(key: &DeviceKey) -> &str {
    match key {
        DeviceKey::Address(text)
        | DeviceKey::AmsNetId(text)
        | DeviceKey::UsbSerial(text)
        | DeviceKey::RuntimeId(text)
        | DeviceKey::Local(text) => text,
    }
}

/// Adds `find` to a device's results, or folds it into the same result seen before.
fn insert_find(finds: &mut Vec<ProtocolFind>, find: ProtocolFind) -> Added {
    match finds
        .iter_mut()
        .find(|known| known.protocol == find.protocol && known.key == find.key)
    {
        Some(known) => {
            if presence::reobserve(known, find) {
                Added::Changed
            } else {
                Added::Unchanged
            }
        }
        None => {
            finds.push(find);
            Added::New
        }
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
