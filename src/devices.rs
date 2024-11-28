use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{event, Level};
use uuid::Uuid;

use crate::dnac::{ResponseType, DNAC};

use super::dnac::{FetchableType, Pagination};

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

#[derive(Debug, Deserialize)]
pub struct Device {
    pub id: Uuid,
    #[serde(rename = "collectionStatus")]
    pub collection_status: DeviceStatus,
    #[serde(rename = "managementIpAddress")]
    pub management_ip_address: String,
    pub hostname: Option<String>,
    pub description: Option<String>,
    pub family: Option<DeviceFamily>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum DeviceStatus {
    Unassociated,
    Synchronizing,
    #[serde(rename = "Sync Disabled")]
    SyncDisabled,
    #[serde(rename = "Could Not Synchronize")]
    CouldNotSynchronize,
    #[serde(rename = "Not Manageable")]
    NotManageable,
    Managed,
    #[serde(rename = "Partial Collection Failure")]
    PartialCollectionFailure,
    Incomplete,
    Unreachable,
    #[serde(rename = "Wrong Credential")]
    WrongCredential,
    Reachable,
    #[serde(rename = "In Progress")]
    InProgress,
}

pub enum DeviceFilter {
    Family(DeviceFamily),
    ManagementIPAddress(String),
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
                    DeviceFilter::ManagementIPAddress(ip) => {
                        query.push(("managementIpAddress", ip))
                    }
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

    pub async fn add_device(dnac: &DNAC, device: AddDevice) -> anyhow::Result<()> {
        let path = "/dna/intent/api/v1/network-device";
        Ok(dnac.post(path, Some(device), true).await?)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct AddDevice {
    #[serde(rename = "ipAddress")]
    pub ip_address: Vec<String>,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
    #[serde(rename = "userName")]
    pub user_name: String,
    #[serde(rename = "password")]
    pub password: String,
    #[serde(rename = "enablePassword")]
    pub enable_password: String,
    #[serde(rename = "cliTransport")]
    pub cli_transport: CliTransport,
    #[serde(rename = "snmpVersion")]
    pub snmp_version: SnmpVersion,
    #[serde(rename = "snmpUserName")]
    pub snmp_user_name: String,
    #[serde(rename = "snmpMode")]
    pub snmp_mode: SnmpMode,
    #[serde(rename = "snmpAuthPassphrase")]
    pub snmp_auth_passphrase: String,
    #[serde(rename = "snmpPrivPassphrase")]
    pub snmp_priv_passphrase: String,
    #[serde(rename = "snmpAuthProtocol")]
    pub snmp_auth_protocol: SnmpAuthProtocol,
    #[serde(rename = "snmpPrivProtocol")]
    pub snmp_priv_protocol: SnmpPrivProtocol,
    #[serde(rename = "netconfPort")]
    pub netconf_port: u16,
}

#[derive(Debug, Default, Serialize)]
pub enum DeviceType {
    #[default]
    #[serde(rename = "NETWORK_DEVICE")]
    NetworkDevice,
    #[serde(rename = "COMPUTE_DEVICE")]
    ComputeDevice,
    #[serde(rename = "MERAKI_DASHBOARD")]
    MerakiDashboard,
    #[serde(rename = "THIRD_PARTY_DEVICE")]
    ThirdPartyDevice,
    #[serde(rename = "NODATACHANGE")]
    NoDataChange,
}

#[derive(Debug, Default, Serialize)]
pub enum CliTransport {
    #[default]
    #[serde(rename = "ssh")]
    Ssh,
    #[serde(rename = "telnet")]
    Telnet,
}

#[derive(Debug, Default, Serialize)]
pub enum SnmpVersion {
    #[default]
    #[serde(rename = "v3")]
    V3,
    #[serde(rename = "v2")]
    V2,
}

#[derive(Debug, Default, Serialize)]
pub enum SnmpMode {
    #[default]
    #[serde(rename = "authPriv")]
    AuthPriv,
    #[serde(rename = "authNoPriv")]
    AuthNoPriv,
    #[serde(rename = "noAuthNoPriv")]
    NoAuthNoPriv,
}

#[derive(Debug, Default, Serialize)]
pub enum SnmpAuthProtocol {
    #[default]
    #[serde(rename = "sha")]
    Sha,
    #[serde(rename = "md5")]
    Md5,
}

#[derive(Debug, Default, Serialize)]
pub enum SnmpPrivProtocol {
    #[default]
    #[serde(rename = "AES128")]
    Aes128,
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
