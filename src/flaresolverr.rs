use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::OnceCell;
use tracing::{debug, info, instrument};

#[derive(Serialize)]
struct FsRequest<'a> {
    cmd: &'static str,
    url: &'a str,
    session: &'a str,
    session_ttl_minutes: u32,
    #[serde(rename = "maxTimeout")]
    max_timeout: u32,
    #[serde(rename = "disableMedia")]
    disable_media: bool,
}

#[derive(Deserialize)]
struct FsResponse {
    solution: Solution,
}

#[derive(Deserialize)]
struct Solution {
    response: String,
}

pub struct FlareSolverrClient {
    client: Client,
    endpoint: String,
    session: String,
    session_ready: OnceCell<()>,
}

impl FlareSolverrClient {
    pub fn new(client: Client, base_url: String, session: String) -> Self {
        Self {
            client,
            endpoint: format!("{base_url}/v1"),
            session,
            session_ready: OnceCell::new(),
        }
    }

    #[instrument(skip(self), fields(url))]
    pub async fn get(&self, url: &str) -> Result<String> {
        self.ensure_session().await;
        info!(endpoint = %self.endpoint, session = %self.session, "sending request to FlareSolverr");
        let body = FsRequest {
            cmd: "request.get",
            url,
            session: &self.session,
            session_ttl_minutes: 30,
            max_timeout: 300_000,
            disable_media: true,
        };

        let resp: FsResponse = self
            .client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .context("FlareSolverr send failed")?
            .error_for_status()
            .context("FlareSolverr non-2xx")?
            .json()
            .await
            .context("FlareSolverr JSON parse failed")?;

        debug!(
            response_bytes = resp.solution.response.len(),
            "received FlareSolverr HTML response"
        );
        Ok(resp.solution.response)
    }

    async fn ensure_session(&self) {
        let _ = self
            .session_ready
            .get_or_init(|| async {
                #[derive(Serialize)]
                struct SessionCreateRequest<'a> {
                    cmd: &'static str,
                    session: &'a str,
                }

                let body = SessionCreateRequest {
                    cmd: "sessions.create",
                    session: &self.session,
                };

                match self.client.post(&self.endpoint).json(&body).send().await {
                    Ok(resp) => match resp.json::<Value>().await {
                        Ok(json) => {
                            debug!(response = %json, session = %self.session, "initialized FlareSolverr session");
                        }
                        Err(error) => {
                            debug!(error = %error, session = %self.session, "FlareSolverr session init response was not JSON");
                        }
                    },
                    Err(error) => {
                        debug!(error = %error, session = %self.session, "FlareSolverr session init failed");
                    }
                }
            })
            .await;
    }
}
