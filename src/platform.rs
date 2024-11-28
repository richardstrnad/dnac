use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::dnac::DNAC;

#[derive(Serialize, Deserialize, Debug)]
pub struct ReleaseSummary {
    pub name: String,
    #[serde(rename = "corePackages")]
    pub core_packages: Vec<String>,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "displayVersion")]
    pub display_version: String,
    pub packages: Vec<String>,
    pub previous_version: Option<String>,
    #[serde(rename = "supportedDirectUpdates")]
    pub supported_direct_updates: Vec<String>,
    #[serde(rename = "systemPackages")]
    pub system_packages: Vec<String>,
    #[serde(rename = "systemVersion")]
    pub system_version: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "installedVersion")]
    pub installed_version: String,
}

impl ReleaseSummary {
    pub async fn get_release_summary(dnac: &DNAC) -> Result<Self> {
        let path = "/dna/intent/api/v1/dnac-release";

        let site_data = dnac.get(path, None, None).await;
        match site_data {
            Ok(site_data) => match site_data.response {
                super::dnac::ResponseType::Item(data) => Ok(data),
                _ => Err(anyhow!("Unexpected Result!")),
            },
            Err(err) => Err(err),
        }
    }
}
