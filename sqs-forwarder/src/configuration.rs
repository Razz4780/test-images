use std::os::unix::ffi::OsStrExt;

use aws_sdk_sqs::Client;
use serde::{Deserialize, Serialize};

/// (Legacy) Name of the environment variable that holds the name of the SQS queue to read from.
///
/// TODO remove once E2E suite in mirrord is updated to use [`QUEUES_ENV_VAR`].
const QUEUE_NAME_ENV_VAR1: &str = "SQS_TEST_Q_NAME1";

/// (Legacy) Name of the environment variable that holds the name of the SQS queue to write to.
///
/// TODO remove once E2E suite in mirrord is updated to use [`QUEUES_ENV_VAR`].
const ECHO_QUEUE_NAME_ENV_VAR1: &str = "SQS_TEST_ECHO_Q_NAME1";

/// (Legacy) Name of the environment variable that holds the url of the second SQS queue to read from.
/// Using URL, not name here, to test that functionality.
///
/// TODO remove once E2E suite in mirrord is updated to use [`QUEUES_ENV_VAR`].
const QUEUE2_URL_ENV_VAR: &str = "SQS_TEST_Q2_URL";

/// (Legacy) Name of the environment variable that holds the name of the SQS queue to write to.
///
/// TODO remove once E2E suite in mirrord is updated to use [`QUEUES_ENV_VAR`].
const ECHO_QUEUE_NAME_ENV_VAR2: &str = "SQS_TEST_ECHO_Q_NAME2";

/// Name of the environment variable that holds [`QueuesConfig`] serialized as JSON.
///
/// If the variable is not set, the app falls back to using legacy environment variables.
const QUEUES_ENV_VAR: &str = "SQS_TEST_QUEUES";

pub type QueuesConfig = Vec<QueueConfig>;

#[derive(Serialize, Deserialize, Debug)]
pub struct QueueConfig {
    pub input: NameOrUrl,
    pub output: NameOrUrl,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename = "camelCase")]
pub enum NameOrUrl {
    Name(String),
    Url(String),
}

impl NameOrUrl {
    pub async fn resolve_url(&self, client: &Client) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Name(name) => client
                .get_queue_url()
                .queue_name(name)
                .send()
                .await
                .unwrap()
                .queue_url
                .unwrap(),
        }
    }
}

pub fn from_env() -> QueuesConfig {
    if let Some(queues) = std::env::var_os(QUEUES_ENV_VAR) {
        let queues = serde_json::from_slice(queues.as_bytes()).unwrap();
        return queues;
    }

    let input_1_name = std::env::var(QUEUE_NAME_ENV_VAR1).unwrap();
    let output_1_name = std::env::var(ECHO_QUEUE_NAME_ENV_VAR1).unwrap();

    let input_2_url = std::env::var(QUEUE2_URL_ENV_VAR).unwrap();
    let output_2_name = std::env::var(ECHO_QUEUE_NAME_ENV_VAR2).unwrap();

    vec![
        QueueConfig {
            input: NameOrUrl::Name(input_1_name),
            output: NameOrUrl::Name(output_1_name),
        },
        QueueConfig {
            input: NameOrUrl::Url(input_2_url),
            output: NameOrUrl::Name(output_2_name),
        },
    ]
}
