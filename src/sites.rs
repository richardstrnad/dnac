use core::fmt;
use std::error::Error;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{event, Level};
use uuid::Uuid;

use crate::dnac::{ApiError, Pagination, DNAC};

pub struct Sites;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub id: Uuid,
    pub group_name_hierarchy: String,
    pub group_hierarchy: String,
    pub name: String,
    pub location: Option<Location>,
    pub additional_info: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    country: Option<String>,
    address: Option<String>,
    latitude: Option<String>,
    address_inherited_from: String,
    #[serde(rename = "type")]
    location_type: String,
    longitude: Option<String>,
}

// name: siteNameHierarchy (ex: global/groupName)
// id: Site id to which site details to retrieve.
// type (ex: area, building, floor)
pub enum SiteFilter {
    Name(String),
    SiteID(Uuid),
    Type(SiteType),
}

#[derive(Clone, Copy)]
pub enum SiteType {
    Area,
    Building,
    Floor,
}

#[derive(Debug)]
pub enum SiteError {
    GeneralError,
    InvalidSite,
}

impl fmt::Display for SiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiteError::GeneralError => write!(f, "General Site Error"),
            SiteError::InvalidSite => write!(f, "Invalid Site"),
        }
    }
}
impl Error for SiteError {}

impl ToString for SiteType {
    fn to_string(&self) -> String {
        match self {
            Self::Area => "area".to_string(),
            Self::Building => "building".to_string(),
            Self::Floor => "floor".to_string(),
        }
    }
}

impl Sites {
    pub async fn get_site(
        dnac: &DNAC,
        filter: Option<SiteFilter>,
        pagination: Option<Pagination>,
    ) -> Result<Vec<Site>, SiteError> {
        let path = "/dna/intent/api/v2/site";
        let query = {
            let mut query = vec![];

            if let Some(filter) = filter {
                match filter {
                    SiteFilter::Name(name) => query.push(("name", name)),
                    SiteFilter::SiteID(id) => query.push(("siteId", id.to_string())),
                    SiteFilter::Type(site_type) => query.push(("type", site_type.to_string())),
                }
            };

            query
        };

        let site_data = dnac
            .get::<Site>(path, Some(query.as_slice()), pagination)
            .await;

        match site_data {
            Ok(site_data) => match site_data.response {
                super::dnac::ResponseType::Array(data) => {
                    Ok(data.into_iter().map(|s| s.parse()).collect())
                }
                super::dnac::ResponseType::Item(data) => Ok(vec![data.parse()]),
            },
            Err(e) => {
                if let Some(api_error) = e.downcast_ref::<ApiError>() {
                    match api_error.response.error_code.as_str() {
                        "NCGR10008" => return Err(SiteError::InvalidSite),
                        _ => {
                            event!(Level::ERROR, "{}", api_error);
                            return Err(SiteError::GeneralError);
                        }
                    }
                } else {
                    event!(Level::ERROR, "{}", e);
                }
                Err(SiteError::GeneralError)
            }
        }
    }

    pub async fn get_all_sites(
        dnac: &DNAC,
        site_type: Option<SiteType>,
    ) -> Result<Vec<Site>, SiteError> {
        let mut offset = 1;
        let limit = 500;
        let mut sites: Vec<Site> = vec![];
        loop {
            event!(
                Level::DEBUG,
                "Fetching Sites with offset: {offset} and limit: {limit}"
            );
            let pagination = Pagination::builder()
                .with_offset(offset)
                .with_limit(limit)
                .build();

            let filter = match site_type {
                Some(site_type) => Some(SiteFilter::Type(site_type)),
                None => None,
            };
            let current_sites = Sites::get_site(dnac, filter, Some(pagination)).await?;
            if current_sites.len() <= 1 {
                if current_sites.len() == 1 {
                    if let None = sites.iter().find(|s| s.id == current_sites[0].id) {
                        sites.extend(current_sites);
                    }
                }
                break;
            }

            sites.extend(current_sites);
            offset += limit;
        }

        Ok(sites)
    }
}

impl Site {
    pub fn parse(mut self) -> Self {
        if let Some(data) = &self.additional_info {
            for entry in data {
                let name_space = entry["nameSpace"].as_str().unwrap();
                match name_space {
                    "Location" => {
                        let location: Location =
                            serde_json::from_value(entry["attributes"].clone()).unwrap();
                        self.location = Some(location);
                    }

                    _ => (),
                }
            }
        }

        self
    }

    // we provide various getters which return a location value or an empty string
    pub fn get_country(&self) -> String {
        match &self.location {
            Some(location) => match &location.country {
                Some(country) => country.clone(),
                None => "".to_string(),
            },
            None => "".to_string(),
        }
    }

    pub fn get_address(&self) -> String {
        match &self.location {
            Some(location) => match &location.address {
                Some(address) => address.clone(),
                None => "".to_string(),
            },
            None => "".to_string(),
        }
    }

    pub fn get_latitude(&self) -> String {
        match &self.location {
            Some(location) => match &location.latitude {
                Some(latitude) => latitude.clone(),
                None => "".to_string(),
            },
            None => "".to_string(),
        }
    }

    pub fn get_longitude(&self) -> String {
        match &self.location {
            Some(location) => match &location.longitude {
                Some(longitude) => longitude.clone(),
                None => "".to_string(),
            },
            None => "".to_string(),
        }
    }

    pub fn get_location_type(&self) -> String {
        match &self.location {
            Some(location) => location.location_type.clone(),
            None => "".to_string(),
        }
    }
}
