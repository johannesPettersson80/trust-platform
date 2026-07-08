use std::collections::BTreeMap;

use trust_ads_core::{PointQuality, SymbolDescriptor};
use trust_runtime_core::value::Value;

use super::transport::{
    AdsDeviceState, AdsHandleRequest, AdsNotificationSample, AdsPointAddress, AdsReadResult,
    AdsResolvedHandle, AdsSubscribeRequest, AdsSubscription, AdsTransport, AdsTransportError,
    AdsWriteRequest,
};

#[derive(Debug, Default)]
pub struct MockAdsTransport {
    connected: bool,
    device_state: AdsDeviceState,
    symbol_version: u32,
    next_handle: u32,
    next_subscription: u32,
    symbols: Vec<SymbolDescriptor>,
    values: BTreeMap<String, Value>,
    qualities: BTreeMap<String, PointQuality>,
    subscription_ids_by_point: BTreeMap<String, u32>,
    notifications: Vec<AdsNotificationSample>,
    sumup_read_batches: usize,
    disconnect_count: usize,
    subscribe_count: usize,
}

impl MockAdsTransport {
    pub fn new(symbols: Vec<SymbolDescriptor>) -> Self {
        Self {
            connected: false,
            device_state: AdsDeviceState::Run,
            symbol_version: 1,
            next_handle: 1,
            next_subscription: 1,
            symbols,
            values: BTreeMap::new(),
            qualities: BTreeMap::new(),
            subscription_ids_by_point: BTreeMap::new(),
            notifications: Vec::new(),
            sumup_read_batches: 0,
            disconnect_count: 0,
            subscribe_count: 0,
        }
    }

    pub fn set_value(
        &mut self,
        point_name: impl Into<String>,
        value: Value,
        quality: PointQuality,
    ) {
        let point_name = point_name.into();
        self.values.insert(point_name.clone(), value);
        self.qualities.insert(point_name, quality);
    }

    pub fn value(&self, point_name: &str) -> Option<&Value> {
        self.values.get(point_name)
    }

    pub fn bump_symbol_version(&mut self) {
        self.symbol_version = self.symbol_version.saturating_add(1);
    }

    pub fn set_symbols(&mut self, symbols: Vec<SymbolDescriptor>) {
        self.symbols = symbols;
    }

    pub fn emit_notification(
        &mut self,
        point_name: &str,
        value: Value,
        quality: PointQuality,
    ) -> Result<(), AdsTransportError> {
        let subscription_id = self
            .subscription_ids_by_point
            .get(point_name)
            .copied()
            .ok_or_else(|| {
                AdsTransportError::new(format!(
                    "mock ADS point '{point_name}' does not have a subscription"
                ))
            })?;
        self.notifications.push(AdsNotificationSample {
            point_name: point_name.to_string(),
            subscription_id,
            value: Some(value),
            quality,
        });
        Ok(())
    }

    pub fn subscription_id_for_point(&self, point_name: &str) -> Option<u32> {
        self.subscription_ids_by_point.get(point_name).copied()
    }

    pub fn sumup_read_batches(&self) -> usize {
        self.sumup_read_batches
    }

    pub fn disconnect_count(&self) -> usize {
        self.disconnect_count
    }

    pub fn subscribe_count(&self) -> usize {
        self.subscribe_count
    }
}

impl AdsTransport for MockAdsTransport {
    fn connect(&mut self) -> Result<(), AdsTransportError> {
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), AdsTransportError> {
        self.disconnect_count = self.disconnect_count.saturating_add(1);
        self.connected = false;
        self.subscription_ids_by_point.clear();
        self.notifications.clear();
        Ok(())
    }

    fn read_state(&mut self) -> Result<AdsDeviceState, AdsTransportError> {
        self.require_connected()?;
        Ok(self.device_state)
    }

    fn upload_symbol_table(&mut self) -> Result<Vec<SymbolDescriptor>, AdsTransportError> {
        self.require_connected()?;
        Ok(self.symbols.clone())
    }

    fn resolve_handles(
        &mut self,
        requests: &[AdsHandleRequest],
    ) -> Result<Vec<AdsResolvedHandle>, AdsTransportError> {
        self.require_connected()?;
        requests
            .iter()
            .map(|request| {
                self.resolve_address(&request.address)?;
                let handle = self.next_handle;
                self.next_handle = self.next_handle.saturating_add(1);
                Ok(AdsResolvedHandle {
                    point_name: request.point_name.clone(),
                    address: request.address.clone(),
                    data_type: request.data_type.clone(),
                    handle,
                })
            })
            .collect()
    }

    fn sumup_read(
        &mut self,
        handles: &[AdsResolvedHandle],
    ) -> Result<Vec<AdsReadResult>, AdsTransportError> {
        self.require_connected()?;
        self.sumup_read_batches = self.sumup_read_batches.saturating_add(1);
        handles
            .iter()
            .map(|handle| {
                let (value, quality) =
                    if let Some(value) = self.values.get(handle.point_name.as_str()).cloned() {
                        let quality = self
                            .qualities
                            .get(handle.point_name.as_str())
                            .cloned()
                            .unwrap_or_else(|| PointQuality::good(0));
                        (Some(value), quality)
                    } else {
                        (
                            None,
                            PointQuality::error(
                                0,
                                format!("mock ADS value '{}' is not set", handle.point_name),
                            ),
                        )
                    };
                Ok(AdsReadResult {
                    point_name: handle.point_name.clone(),
                    value,
                    quality,
                })
            })
            .collect()
    }

    fn sumup_write(
        &mut self,
        writes: &[AdsWriteRequest],
    ) -> Result<Vec<PointQuality>, AdsTransportError> {
        self.require_connected()?;
        let mut qualities = Vec::with_capacity(writes.len());
        for write in writes {
            let quality = PointQuality::good(0);
            self.values
                .insert(write.handle.point_name.clone(), write.value.clone());
            self.qualities
                .insert(write.handle.point_name.clone(), quality.clone());
            qualities.push(quality);
        }
        Ok(qualities)
    }

    fn subscribe(
        &mut self,
        request: AdsSubscribeRequest,
    ) -> Result<AdsSubscription, AdsTransportError> {
        self.require_connected()?;
        self.subscribe_count = self.subscribe_count.saturating_add(1);
        let subscription_id = self.next_subscription;
        self.next_subscription = self.next_subscription.saturating_add(1);
        self.subscription_ids_by_point
            .insert(request.handle.point_name.clone(), subscription_id);
        Ok(AdsSubscription {
            point_name: request.handle.point_name,
            subscription_id,
        })
    }

    fn drain_notifications(&mut self) -> Result<Vec<AdsNotificationSample>, AdsTransportError> {
        self.require_connected()?;
        Ok(std::mem::take(&mut self.notifications))
    }

    fn symbol_version(&mut self) -> Result<u32, AdsTransportError> {
        self.require_connected()?;
        Ok(self.symbol_version)
    }
}

impl MockAdsTransport {
    fn require_connected(&self) -> Result<(), AdsTransportError> {
        if self.connected {
            Ok(())
        } else {
            Err(AdsTransportError::new(
                "mock ADS transport is not connected",
            ))
        }
    }

    fn resolve_address(&self, address: &AdsPointAddress) -> Result<(), AdsTransportError> {
        let found = self.symbols.iter().any(|symbol| match address {
            AdsPointAddress::Symbol(name) => symbol.name == *name,
            AdsPointAddress::Index {
                index_group,
                index_offset,
                size,
            } => {
                symbol.index_group == *index_group
                    && symbol.index_offset == *index_offset
                    && symbol.byte_size == *size
            }
        });
        if found {
            Ok(())
        } else {
            Err(AdsTransportError::new(format!(
                "mock ADS address {address:?} was not found"
            )))
        }
    }
}
