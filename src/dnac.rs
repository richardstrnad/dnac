use core::fmt;
use std::{error::Error, fs};

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tracing::{event, Level};

use super::platform::ReleaseSummary;

const SUPPORTED_VERSIONS: [&str; 2] = ["2.3.7.5", "2.3.7.6"];

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Token {
    #[serde(rename = "Token")]
    token: String,
    exp: Option<u64>,
}

#[derive(Debug)]
pub struct DNAC {
    pub client: reqwest::Client,
    pub token: Token,
    pub token_file: String,
    pub dnac: String,
    pub user: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response<T> {
    pub response: ResponseType<T>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ResponseType<T> {
    Array(Vec<T>),
    Item(T),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: Vec<String>,
    pub response: ApiErrorResponse,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiErrorResponse {
    #[serde(rename = "errorCode")]
    pub error_code: String,
    pub message: String,
    pub href: String,
}

#[derive(Clone, Copy)]
pub struct Pagination {
    offset: u64,
    limit: u64,
}

pub struct PaginationBuilder {
    offset: u64,
    limit: u64,
}

#[derive(Debug, Deserialize)]
pub struct TaskInfo {
    #[serde(rename = "taskId")]
    pub task_id: String,
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(rename = "additionalStatusURL")]
    pub additional_status_url: Option<String>,
    pub data: Option<String>,
    #[serde(rename = "endTime")]
    pub end_time: Option<u64>,
    #[serde(rename = "errorCode")]
    pub error_code: Option<String>,
    #[serde(rename = "errorKey")]
    pub error_key: Option<String>,
    #[serde(rename = "failureReason")]
    pub failure_reason: Option<String>,
    #[serde(rename = "instanceTenantId")]
    pub instance_tenant_id: String,
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(rename = "lastUpdate")]
    pub last_update: Option<u64>,
    #[serde(rename = "operationIdList")]
    pub operation_id_list: Option<Value>,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    pub progress: String,
    #[serde(rename = "rootId")]
    pub root_id: Option<String>,
    #[serde(rename = "serviceType")]
    pub service_type: String,
    #[serde(rename = "startTime")]
    pub start_time: u64,
    pub username: Option<String>,
    pub version: u64,
}

impl DNAC {
    pub async fn new(
        token_file: String,
        dnac: String,
        user: String,
        password: String,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let token = Token::default();

        let dnac = if let Some(dnac) = dnac.strip_suffix("/") {
            dnac.to_string()
        } else {
            dnac
        };

        let mut dnac = Self {
            client,
            token,
            token_file,
            dnac,
            user,
            password,
        };

        let token = {
            if let Ok(mut token) = dnac.load_token() {
                token.parse();
                // if the token is still valid and valid for more than 10 min we use it
                if token.valid() && token.valid_for() > 60 * 10 {
                    event!(
                        Level::INFO,
                        "Loaded token is still valid for {} sec and will be used",
                        token.valid_for()
                    );
                    token
                } else {
                    event!(
                        Level::INFO,
                        "Loaded token is no longer valid, generate a new one"
                    );
                    dnac.get_token().await.unwrap()
                }
            } else {
                // if we can't load a token and don't get one from the API we fail hard
                event!(Level::INFO, "Token file not found, generate a new one");
                dnac.get_token().await.unwrap()
            }
        };

        dnac.token = token;

        dnac.verify_version().await?;

        Ok(dnac)
    }

    // We make sure that the client is run against a supported Version
    pub async fn verify_version(&self) -> Result<&str> {
        let release_summary = ReleaseSummary::get_release_summary(self).await?;

        SUPPORTED_VERSIONS
            .into_iter()
            .find(|v| release_summary.installed_version.contains(v))
            .ok_or(anyhow!(
                "Version {} not supported",
                release_summary.installed_version
            ))
    }

    pub async fn get_token(&self) -> Result<Token> {
        let path = "/dna/system/api/v1/auth/token";

        let mut token = self
            .client
            .post(format!("{}{}", self.dnac, path))
            .basic_auth(&self.user, Some(&self.password))
            .send()
            .await?
            .json::<Token>()
            .await?;

        token.parse();
        token.save()?;

        Ok(token)
    }

    pub fn load_token(&self) -> Result<Token> {
        let file = fs::File::open(&self.token_file)?;
        let token = serde_json::from_reader(file)?;

        Ok(token)
    }

    pub async fn get<T>(
        &self,
        path: &str,
        input_query: Option<&[(&str, String)]>,
        pagination: Option<Pagination>,
    ) -> Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        let query = {
            let mut query = vec![];
            if let Some(pagination) = pagination {
                query.push(("offset", pagination.offset.to_string()));
                query.push(("limit", pagination.limit.to_string()));
            }

            match input_query {
                Some(input_query) => query.extend_from_slice(input_query),
                None => (),
            }
            query
        };

        let data = self
            .client
            .get(format!("{}{}", self.dnac, path))
            .header("X-Auth-Token", &self.token.token)
            .query(&query)
            .send()
            .await?;

        match data.status() {
            StatusCode::INTERNAL_SERVER_ERROR => {
                let data = data.json::<ApiError>().await?;
                return Err(data.into());
            }
            _ => {
                let data = data.json().await?;
                Ok(data)
            }
        }
    }

    // poll is a flag to indicate if we should poll the API for the result
    pub async fn post<T>(&self, path: &str, data: T, poll: bool) -> Result<()>
    where
        T: Serialize,
    {
        let response = self
            .client
            .post(format!("{}{}", self.dnac, path))
            .header("X-Auth-Token", &self.token.token)
            .json(&data)
            .send()
            .await?;

        match response.status() {
            StatusCode::INTERNAL_SERVER_ERROR => {
                let data = response.json::<ApiError>().await?;
                return Err(data.into());
            }
            _ => {
                // we assume that we got a TaskInfo back
                if poll {
                    let response = response.json::<Response<TaskInfo>>().await?;
                    match response.response {
                        ResponseType::Item(task_info) => {
                            self.poll_task(task_info).await?;
                        }
                        _ => {
                            return Err(anyhow!("Unexpected response"));
                        }
                    }
                }
                Ok(())
            }
        }
    }

    async fn poll_task(&self, mut task_info: TaskInfo) -> Result<()> {
        event!(Level::DEBUG, "Polling Task: {:?}", task_info);
        task_info.url.push_str("/tree/");
        let mut task = self
            .get::<Task>(task_info.url.as_str(), None, None)
            .await?
            .response;

        loop {
            match task {
                ResponseType::Array(ref inner_tasks) => {
                    if inner_tasks.iter().all(|t| t.end_time.is_some()) {
                        if inner_tasks.iter().any(|t| t.is_error) {
                            inner_tasks.iter().for_each(|t| {
                                if t.is_error {
                                    event!(Level::ERROR, "{task:?}");
                                }
                            });

                            return Err(anyhow!("Task failed"));
                        }
                        break;
                    } else {
                        task = self.get::<Task>(&task_info.url, None, None).await?.response;
                    }
                }
                _ => {
                    return Err(anyhow!("Unexpected response"));
                }
            }

            event!(Level::DEBUG, "Task is still running, sleep for 5 sec");
            event!(Level::DEBUG, "Task: {:?}", task);
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }

        Ok(())
    }
}

impl Token {
    pub fn parse(&mut self) {
        let unverified: jwt::Token<jwt::Header, jwt::RegisteredClaims, _> =
            jwt::Token::parse_unverified(&self.token).unwrap();
        self.exp = Some(unverified.claims().expiration.unwrap());
    }

    pub fn save(&self) -> Result<()> {
        let token_file =
            std::env::var("DNAC_TOKEN_FILE").expect("Missing 'DNAC_TOKEN_FILE' env var!");
        let file = fs::File::create(token_file)?;
        serde_json::to_writer(file, self)?;

        Ok(())
    }

    pub fn valid(&self) -> bool {
        if let Some(exp) = self.exp {
            exp > chrono::offset::Local::now().timestamp() as u64
        } else {
            false
        }
    }

    pub fn valid_for(&self) -> u64 {
        if let Some(exp) = self.exp {
            let now = chrono::offset::Local::now().timestamp() as u64;
            if exp > now {
                exp - now
            } else {
                0
            }
        } else {
            0
        }
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 500,
            offset: 1,
        }
    }
}

impl Pagination {
    pub fn builder() -> PaginationBuilder {
        let pagination = Pagination::default();
        PaginationBuilder {
            offset: pagination.offset,
            limit: pagination.limit,
        }
    }

    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit;
    }

    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }
}

impl PaginationBuilder {
    pub fn with_limit(mut self, limit: u64) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    pub fn build(self) -> Pagination {
        Pagination {
            offset: self.offset,
            limit: self.limit,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error with the API request")
    }
}

impl Error for ApiError {}

#[async_trait::async_trait]
pub trait FetchableType: Sized {
    type Filter;
    type Error;

    async fn fetch_list(
        dnac: &DNAC,
        filter: Option<Self::Filter>,
        pagination: Option<Pagination>,
    ) -> Result<Vec<Self>, Self::Error>;
}

pub trait GetAll {
    fn get_all<T, E>(dnac: &DNAC) -> Result<Vec<T>, E>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task() {
        let task = r#"
          {"version":1732811427209,"progress":"Inventory service adding devices","startTime":1732811427209,"serviceType":"Inventory service","isError":false,"instanceTenantId":"6307971e4289f95403c86831","id":"0193739c-0d88-78e4-ba0f-d82889fca555"}
          "#;

        let task: Task = serde_json::from_str(task).unwrap();
        assert_eq!(task.id, "");
    }
}
