use crate::error::{
    Error, InvalidFunctionArgument, ResponseError, UnknownResponseStatus,
    UnsupportedQueryResultType,
};
use crate::response::*;
use crate::selector::Selector;
use crate::util::{validate_duration, RuleType, TargetState};
use std::collections::HashMap;
use url::Url;

/// A client used to execute queries. It uses a [reqwest::Client] internally
/// that manages connections for us.
///
/// Note that possible errors regarding domain name resolution or
/// connection establishment will only be propagated from the underlying
/// [reqwest::Client] when a query is executed.
#[derive(Clone)]
pub struct Client {
    pub(crate) client: reqwest::Client,
    pub(crate) base_url: String,
}

impl Default for Client {
    /// Create a standard Client that sends requests to "http://127.0.0.1:9090/api/v1".
    ///
    /// ```rust
    /// use prometheus_http_query::Client;
    ///
    /// let client = Client::default();
    /// ```
    fn default() -> Self {
        Client {
            client: reqwest::Client::new(),
            base_url: String::from("http://127.0.0.1:9090/api/v1"),
        }
    }
}

impl std::str::FromStr for Client {
    type Err = crate::error::Error;

    /// Create a Client from a custom base URL, which *may* be useful if requests
    /// are handled by i.e. a reverse proxy.
    ///
    /// ```rust
    /// use prometheus_http_query::Client;
    /// use std::str::FromStr;
    ///
    /// let client = Client::from_str("http://proxy.example.com/prometheus");
    /// assert!(client.is_ok());
    /// ```
    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(url).map_err(Error::UrlParse)?;
        let client = Client {
            base_url: format!("{}/api/v1", url),
            client: reqwest::Client::new(),
        };
        Ok(client)
    }
}

impl std::convert::TryFrom<&str> for Client {
    type Error = crate::error::Error;

    /// Create a Client from a custom base URL, which *may* be useful if requests
    /// are handled by i.e. a reverse proxy.
    ///
    /// ```rust
    /// use prometheus_http_query::Client;
    /// use std::convert::TryFrom;
    ///
    /// let client = Client::try_from("http://proxy.example.com/prometheus");
    /// assert!(client.is_ok());
    /// ```
    fn try_from(url: &str) -> Result<Self, Self::Error> {
        let url = Url::parse(url).map_err(Error::UrlParse)?;
        let client = Client {
            base_url: format!("{}/api/v1", url),
            client: reqwest::Client::new(),
        };
        Ok(client)
    }
}

impl std::convert::TryFrom<String> for Client {
    type Error = crate::error::Error;

    /// Create a Client from a custom base URL, which *may* be useful if requests
    /// are handled by i.e. a reverse proxy.
    ///
    /// ```rust
    /// use prometheus_http_query::Client;
    /// use std::convert::TryFrom;
    ///
    /// let url = String::from("http://proxy.example.com/prometheus");
    /// let client = Client::try_from(url);
    /// assert!(client.is_ok());
    /// ```
    fn try_from(url: String) -> Result<Self, Self::Error> {
        let url = Url::parse(&url).map_err(Error::UrlParse)?;
        let client = Client {
            base_url: format!("{}/api/v1", url),
            client: reqwest::Client::new(),
        };
        Ok(client)
    }
}

impl Client {
    /// Return a reference to the wrapped [reqwest::Client], i.e. to
    /// use it for other requests unrelated to the Prometheus API.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error};
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     // An amittedly bad example, but that is not the point.
    ///     let response = client
    ///         .inner()
    ///         .head("http://127.0.0.1:9090")
    ///         .send()
    ///         .await
    ///         .map_err(Error::Reqwest)?;
    ///
    ///     // Prometheus does not allow HEAD requests.
    ///     assert_eq!(response.status(), reqwest::StatusCode::METHOD_NOT_ALLOWED);
    ///     Ok(())
    /// }
    /// ```
    pub fn inner(&self) -> &reqwest::Client {
        &self.client
    }

    /// Return a reference to the base URL that is used in requests to
    /// the Prometheus API.
    ///
    /// ```rust
    /// use prometheus_http_query::Client;
    /// use std::str::FromStr;
    ///
    /// let client = Client::default();
    ///
    /// assert_eq!(client.base_url(), "http://127.0.0.1:9090/api/v1");
    ///
    /// let client = Client::from_str("https://proxy.example.com:8443/prometheus").unwrap();
    ///
    /// assert_eq!(client.base_url(), "https://proxy.example.com:8443/prometheus/api/v1");
    /// ```
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Create a Client from a custom [reqwest::Client] and URL.
    /// This way you can account for all extra parameters (e.g. x509 authentication)
    /// that may be needed to connect to Prometheus or an intermediate proxy,
    /// by building it into the [reqwest::Client].
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error};
    ///
    /// fn main() -> Result<(), Error> {
    ///     let client = {
    ///         let c = reqwest::Client::builder()
    ///             .no_proxy()
    ///             .build()
    ///             .map_err(Error::Reqwest)?;
    ///         Client::from(c, "https://prometheus.example.com")
    ///     };
    ///
    ///     assert!(client.is_ok());
    ///     Ok(())
    /// }
    /// ```
    pub fn from(client: reqwest::Client, url: &str) -> Result<Self, Error> {
        let base_url = format!("{}/api/v1", Url::parse(url).map_err(Error::UrlParse)?);
        Ok(Client { base_url, client })
    }

    /// Perform an instant query using a [crate::RangeVector] or [crate::InstantVector].
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, InstantVector, Selector, Aggregate, Error};
    /// use prometheus_http_query::aggregations::sum;
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let v: InstantVector = Selector::new()
    ///         .metric("node_cpu_seconds_total")
    ///         .try_into()?;
    ///
    ///     let s = sum(v, Some(Aggregate::By(&["cpu"])));
    ///
    ///     let response = client.query(s, None, None).await?;
    ///
    ///     assert!(response.as_instant().is_some());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn query(
        &self,
        vector: impl std::fmt::Display,
        time: Option<i64>,
        timeout: Option<&str>,
    ) -> Result<QueryResultType, Error> {
        let url = format!("{}/query", self.base_url);

        let query = vector.to_string();
        let mut params = vec![("query", query.as_str())];

        let time = time.map(|t| t.to_string());

        if let Some(t) = &time {
            params.push(("time", t.as_str()));
        }

        if let Some(t) = timeout {
            validate_duration(t)?;
            params.push(("timeout", t));
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response)
            .await
            .and_then(convert_query_response)
    }

    pub async fn query_range(
        &self,
        vector: impl std::fmt::Display,
        start: i64,
        end: i64,
        step: Option<&str>,
        timeout: Option<&str>,
    ) -> Result<QueryResultType, Error> {
        let url = format!("{}/query_range", self.base_url);

        let query = vector.to_string();
        let start = start.to_string();
        let end = end.to_string();

        let mut params = vec![
            ("query", query.as_str()),
            ("start", start.as_str()),
            ("end", end.as_str()),
        ];
        if let Some(step) = step {
            validate_duration(step)?;
            params.push(("step",step));
        }

        if let Some(t) = timeout {
            validate_duration(t)?;
            params.push(("timeout", t));
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response)
            .await
            .and_then(convert_query_response)
    }

    /// Find time series that match certain label sets ([Selector]s).
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Selector, Error};
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let s1 = Selector::new()
    ///         .with("handler", "/api/v1/query");
    ///
    ///     let s2 = Selector::new()
    ///         .with("job", "node")
    ///         .regex_match("mode", ".+");
    ///
    ///     let set = vec![s1, s2];
    ///
    ///     let response = client.series(&set, None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn series(
        &self,
        selectors: &[Selector<'_>],
        start: Option<i64>,
        end: Option<i64>,
    ) -> Result<Vec<HashMap<String, String>>, Error> {
        let url = format!("{}/series", self.base_url);

        let mut params = vec![];

        let start = start.map(|t| t.to_string());

        if let Some(s) = &start {
            params.push(("start", s.as_str()));
        }

        let end = end.map(|t| t.to_string());

        if let Some(e) = &end {
            params.push(("end", e.as_str()));
        }

        if selectors.is_empty() {
            return Err(Error::InvalidFunctionArgument(InvalidFunctionArgument {
                message: String::from("at least one match[] argument (Selector) must be provided in order to query the series endpoint")
            }));
        }

        let selectors: Vec<String> = selectors
            .iter()
            .map(|s| match s.to_string().as_str().split_once('}') {
                Some(split) => {
                    let mut s = split.0.to_owned();
                    s.push('}');
                    s
                }
                None => s.to_string(),
            })
            .collect();

        for selector in &selectors {
            params.push(("match[]", &selector));
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let result: Vec<HashMap<String, String>> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(result)
        })
    }

    /// Retrieve all label names (or use [Selector]s to select time series to read label names from).
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Selector, Error};
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     // To retrieve a list of all labels:
    ///     let response = client.label_names(None, None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // To retrieve a list of labels that appear in specific time series, use Selectors:
    ///     let s1 = Selector::new()
    ///         .with("handler", "/api/v1/query");
    ///
    ///     let s2 = Selector::new()
    ///         .with("job", "node")
    ///         .regex_match("mode", ".+");
    ///
    ///     let set = Some(vec![s1, s2]);
    ///
    ///     let response = client.label_names(set, None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn label_names(
        &self,
        selectors: Option<Vec<Selector<'_>>>,
        start: Option<i64>,
        end: Option<i64>,
    ) -> Result<Vec<String>, Error> {
        let url = format!("{}/labels", self.base_url);

        let mut params = vec![];

        let start = start.map(|t| t.to_string());

        if let Some(s) = &start {
            params.push(("start", s.as_str()));
        }

        let end = end.map(|t| t.to_string());

        if let Some(e) = &end {
            params.push(("end", e.as_str()));
        }

        let selectors: Option<Vec<String>> = selectors.map(|vec| {
            vec.iter()
                .map(|s| match s.to_string().as_str().split_once('}') {
                    Some(split) => {
                        let mut s = split.0.to_owned();
                        s.push('}');
                        s
                    }
                    None => s.to_string(),
                })
                .collect()
        });

        if let Some(ref selector_vec) = selectors {
            for selector in selector_vec {
                params.push(("match[]", &selector));
            }
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let result: Vec<String> = serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(result)
        })
    }

    /// Retrieve all label values for a label name (or use [Selector]s to select the time series to read label values from)
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Selector, Error};
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     // To retrieve a list of all label values for a specific label name:
    ///     let response = client.label_values("job", None, None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // To retrieve a list of label values of labels in specific time series instead:
    ///     let s1 = Selector::new()
    ///         .regex_match("instance", ".+");
    ///
    ///     let set = Some(vec![s1]);
    ///
    ///     let response = client.label_values("job", set, None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn label_values(
        &self,
        label: &str,
        selectors: Option<Vec<Selector<'_>>>,
        start: Option<i64>,
        end: Option<i64>,
    ) -> Result<Vec<String>, Error> {
        let url = format!("{}/label/{}/values", self.base_url, label);

        let mut params = vec![];

        let start = start.map(|t| t.to_string());

        if let Some(s) = &start {
            params.push(("start", s.as_str()));
        }

        let end = end.map(|t| t.to_string());

        if let Some(e) = &end {
            params.push(("end", e.as_str()));
        }

        let selectors: Option<Vec<String>> = selectors.map(|vec| {
            vec.iter()
                .map(|s| match s.to_string().as_str().split_once('}') {
                    Some(split) => {
                        let mut s = split.0.to_owned();
                        s.push('}');
                        s
                    }
                    None => s.to_string(),
                })
                .collect()
        });

        if let Some(ref selector_vec) = selectors {
            for selector in selector_vec {
                params.push(("match[]", &selector));
            }
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let result = serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(result)
        })
    }

    /// Query the current state of target discovery.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error, TargetState};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let response = client.targets(None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Filter targets by type:
    ///     let response = client.targets(Some(TargetState::Active)).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn targets(&self, state: Option<TargetState>) -> Result<Targets, Error> {
        let url = format!("{}/targets", self.base_url);

        let mut params = vec![];

        let state = state.map(|s| s.to_string());

        if let Some(s) = &state {
            params.push(("state", s.as_str()))
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let targets: Targets = serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(targets)
        })
    }

    /// Retrieve a list of rule groups of recording and alerting rules.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error, RuleType};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let response = client.rules(None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Filter rules by type:
    ///     let response = client.rules(Some(RuleType::Alert)).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn rules(&self, rule_type: Option<RuleType>) -> Result<Vec<RuleGroup>, Error> {
        let url = format!("{}/rules", self.base_url);

        let mut params = vec![];

        let rule_type = rule_type.map(|s| s.to_string());

        if let Some(s) = &rule_type {
            params.push(("type", s.as_str()))
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let groups = r
                .get("data")
                .ok_or(Error::MissingField)?
                .as_object()
                .unwrap()
                .get("groups")
                .ok_or(Error::MissingField)?
                .to_owned();

            let result: Vec<RuleGroup> =
                serde_json::from_value(groups).map_err(Error::ResponseParse)?;

            Ok(result)
        })
    }

    /// Retrieve a list of active alerts.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let response = client.alerts().await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn alerts(&self) -> Result<Vec<Alert>, Error> {
        let url = format!("{}/alerts", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let alerts = r
                .get("data")
                .ok_or(Error::MissingField)?
                .as_object()
                .unwrap()
                .get("alerts")
                .ok_or(Error::MissingField)?
                .to_owned();

            let result: Vec<Alert> =
                serde_json::from_value(alerts).map_err(Error::ResponseParse)?;

            Ok(result)
        })
    }

    /// Retrieve a list of flags that Prometheus was configured with.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let response = client.flags().await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn flags(&self) -> Result<HashMap<String, String>, Error> {
        let url = format!("{}/status/flags", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let flags: HashMap<String, String> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(flags)
        })
    }

    /// Query the current state of alertmanager discovery.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error, TargetState};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     let response = client.alertmanagers().await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn alertmanagers(&self) -> Result<Alertmanagers, Error> {
        let url = format!("{}/alertmanagers", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r
                .get("data")
                .ok_or(Error::MissingField)?
                .as_object()
                .unwrap();

            let mut active: Vec<Url> = vec![];

            let items = data
                .get("activeAlertmanagers")
                .ok_or(Error::MissingField)?
                .as_array()
                .unwrap();

            for item in items {
                let raw_url = item
                    .get("url")
                    .ok_or(Error::MissingField)?
                    .as_str()
                    .unwrap();

                let url = Url::parse(raw_url).map_err(Error::UrlParse)?;

                active.push(url);
            }

            let mut dropped: Vec<Url> = vec![];

            let items = data
                .get("droppedAlertmanagers")
                .ok_or(Error::MissingField)?
                .as_array()
                .unwrap();

            for item in items {
                let raw_url = item
                    .get("url")
                    .ok_or(Error::MissingField)?
                    .as_str()
                    .unwrap();

                let url = Url::parse(raw_url).map_err(Error::UrlParse)?;

                dropped.push(url);
            }

            let result = Alertmanagers { active, dropped };

            Ok(result)
        })
    }

    /// Retrieve metadata about metrics that are currently scraped from targets, along with target information.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error, Selector};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     // Retrieve metadata for a specific metric from all targets.
    ///     let response = client.target_metadata(Some("go_routines"), None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Retrieve metric metadata from specific targets.
    ///     let s = Selector::new().with("job", "prometheus");
    ///
    ///     let response = client.target_metadata(None, Some(&s), None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Retrieve metadata for a specific metric from targets that match a specific label set.
    ///     let s = Selector::new().with("job", "node");
    ///
    ///     let response = client.target_metadata(Some("node_cpu_seconds_total"), Some(&s), None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn target_metadata(
        &self,
        metric: Option<&str>,
        match_target: Option<&Selector<'_>>,
        limit: Option<usize>,
    ) -> Result<Vec<TargetMetadata>, Error> {
        let url = format!("{}/targets/metadata", self.base_url);

        let mut params = vec![];

        let metric = metric.map(|s| s.to_string());

        if let Some(m) = &metric {
            params.push(("metric", m.as_str()))
        }

        let match_target = match_target.map(|s| s.to_string());

        if let Some(m) = &match_target {
            params.push(("match_target", m.as_str()))
        }

        let limit = limit.map(|s| s.to_string());

        if let Some(l) = &limit {
            params.push(("limit", l.as_str()))
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let result: Vec<TargetMetadata> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(result)
        })
    }

    /// Retrieve metadata about metrics that are currently scraped from targets.
    ///
    /// ```rust
    /// use prometheus_http_query::{Client, Error};
    /// use std::convert::TryInto;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::default();
    ///
    ///     // Retrieve metadata for a all metrics.
    ///     let response = client.metric_metadata(None, None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Limit the number of returned metrics
    ///     let response = client.metric_metadata(None, Some(10)).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     // Retrieve metadata of a specific metric.
    ///     let response = client.metric_metadata(Some("go_routines"), None).await;
    ///
    ///     assert!(response.is_ok());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn metric_metadata(
        &self,
        metric: Option<&str>,
        limit: Option<usize>,
    ) -> Result<HashMap<String, Vec<MetricMetadata>>, Error> {
        let url = format!("{}/metadata", self.base_url);

        let mut params = vec![];

        let metric = metric.map(|s| s.to_string());

        if let Some(m) = &metric {
            params.push(("metric", m.as_str()))
        }

        let limit = limit.map(|s| s.to_string());

        if let Some(l) = &limit {
            params.push(("limit", l.as_str()))
        }

        let response = self
            .client
            .get(&url)
            .query(params.as_slice())
            .send()
            .await
            .map_err(Error::Reqwest)?
            .error_for_status()
            .map_err(Error::Reqwest)?;

        check_response(response).await.and_then(move |r| {
            let data = r.get("data").ok_or(Error::MissingField)?.to_owned();
            let result: HashMap<String, Vec<MetricMetadata>> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;

            Ok(result)
        })
    }
}

// Convert the response object to an intermediary map, check the JSON's status field
// and map potential errors (if any) to a proper error type. Else return the map.
async fn check_response(
    response: reqwest::Response,
) -> Result<HashMap<String, serde_json::Value>, Error> {
    let map = response
        .json::<HashMap<String, serde_json::Value>>()
        .await
        .map_err(Error::Reqwest)?;

    let status = map
        .get("status")
        .ok_or(Error::MissingField)?
        .as_str()
        .unwrap();

    match status {
        "success" => Ok(map),
        "error" => {
            let kind = map
                .get("errorType")
                .ok_or(Error::MissingField)?
                .as_str()
                .unwrap()
                .to_string();

            let message = map
                .get("error")
                .ok_or(Error::MissingField)?
                .as_str()
                .unwrap()
                .to_string();

            Err(Error::ResponseError(ResponseError { kind, message }))
        }
        _ => Err(Error::UnknownResponseStatus(UnknownResponseStatus(
            status.to_string(),
        ))),
    }
}

// Parses the API response from a map to a Response enum that
// encapsulates a result type of "vector", "matrix", or "scalar".
fn convert_query_response(
    response: HashMap<String, serde_json::Value>,
) -> Result<QueryResultType, Error> {
    let data_obj = response
        .get("data")
        .ok_or(Error::MissingField)?
        .as_object()
        .unwrap();

    let data_type = data_obj
        .get("resultType")
        .ok_or(Error::MissingField)?
        .as_str()
        .unwrap();

    let data = data_obj
        .get("result")
        .ok_or(Error::MissingField)?
        .to_owned();

    match data_type {
        "vector" => {
            let result: Vec<InstantVector> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(QueryResultType::Vector(result))
        }
        "matrix" => {
            let result: Vec<RangeVector> =
                serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(QueryResultType::Matrix(result))
        }
        "scalar" => {
            let result: Sample = serde_json::from_value(data).map_err(Error::ResponseParse)?;
            Ok(QueryResultType::Scalar(result))
        }
        _ => Err(Error::UnsupportedQueryResultType(
            UnsupportedQueryResultType(data_type.to_string()),
        )),
    }
}
