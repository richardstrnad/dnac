use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{event, Level};
use uuid::Uuid;

use crate::dnac::dnac::ResponseType;

use super::{
    dnac::{FetchableType, Pagination},
    DNAC,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DeviceFamily {
    #[serde(rename = "Switches and Hubs")]
    SwitchesAndHubs,
    #[serde(rename = "Unified AP")]
    UnifiedAp,
    #[serde(rename = "Routers")]
    Routers,
    #[serde(rename = "Wireless Controller")]
    WirelessController,
    #[serde(rename = "Wireless Sensor")]
    WirelessSensor,
}

impl ToString for DeviceFamily {
    fn to_string(&self) -> String {
        match &self {
            Self::SwitchesAndHubs => String::from("Switches and Hubs"),
            Self::UnifiedAp => String::from("Unified AP"),
            Self::Routers => String::from("Routers"),
            Self::WirelessController => String::from("Wireless Controller"),
            Self::WirelessSensor => String::from("Wireless Sensor"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub description: Option<String>,
    pub family: DeviceFamily,
}

pub enum DeviceFilter {
    Family(DeviceFamily),
}

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("General Device Error")]
    GeneralError,
    #[error("Invalid Device")]
    InvalidDevice,
}

impl Device {
    pub async fn get_device_list(
        dnac: &DNAC,
        filter: Option<DeviceFilter>,
        pagination: Option<Pagination>,
    ) -> Result<Vec<Device>, DeviceError> {
        let path = "/dna/intent/api/v1/network-device";
        let query = {
            let mut query = vec![];

            if let Some(filter) = filter {
                match filter {
                    DeviceFilter::Family(family) => query.push(("family", family.to_string())),
                }
            };

            query
        };
        let device_data = dnac
            .get::<Device>(path, Some(query.as_slice()), pagination)
            .await;

        match device_data {
            Ok(device_data) => match device_data.response {
                ResponseType::Array(data) => Ok(data.into_iter().map(|d| d).collect()),
                ResponseType::Item(data) => Ok(vec![data]),
            },
            Err(e) => {
                event!(Level::ERROR, "{e}");
                Err(DeviceError::GeneralError)
            }
        }
    }

    pub async fn get_all_devices(
        dnac: &DNAC,
        device_family: Option<DeviceFamily>,
    ) -> Result<Vec<Device>, DeviceError> {
        let mut offset = 1;
        let limit = 500;
        let mut devices: Vec<Device> = vec![];

        loop {
            event!(
                Level::DEBUG,
                "Fetching Devices with offset: {offset} and limit: {limit}"
            );
            let pagination = Pagination::builder()
                .with_offset(offset)
                .with_limit(limit)
                .build();

            let filter = match device_family {
                Some(device_family) => Some(DeviceFilter::Family(device_family)),
                None => None,
            };
            let current_devices = Device::get_device_list(dnac, filter, Some(pagination)).await?;
            if current_devices.len() <= 1 {
                if current_devices.len() == 1 {
                    if let None = devices.iter().find(|s| s.id == current_devices[0].id) {
                        devices.extend(current_devices);
                    }
                }
                break;
            }

            devices.extend(current_devices);
            offset += limit;
        }

        Ok(devices)
    }
}

#[async_trait::async_trait]
impl FetchableType for Device {
    type Filter = DeviceFilter;
    type Error = DeviceError;

    async fn fetch_list(
        dnac: &DNAC,
        filter: Option<Self::Filter>,
        pagination: Option<Pagination>,
    ) -> Result<Vec<Device>, DeviceError> {
        Device::get_device_list(dnac, filter, pagination).await
    }
}
