use aws_sdk_sqs::types::MessageSystemAttributeName;
use aws_sdk_sqs::{types::Message, Client};
use configuration::QueueConfig;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinSet;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

mod configuration;

/// Read messages from the input queue and forward them to the output queue.
async fn proxy_messages(
    config: QueueConfig,
    client: Client,
    cancellation_token: CancellationToken,
) {
    println!("Starting message proxy for {config:?}");

    let (input_url, output_url) = tokio::join!(
        config.input.resolve_url(&client),
        config.output.resolve_url(&client),
    );

    println!("Resolves queue URLs: input=[{input_url}], output=[{output_url}]");

    let receive_message_request = client
        .receive_message()
        .message_attribute_names(".*")
        .message_system_attribute_names(MessageSystemAttributeName::All)
        .wait_time_seconds(20)
        .queue_url(&input_url);

    loop {
        let res = tokio::select! {
            _ = cancellation_token.cancelled() => {
                return;
            },
            res = receive_message_request.clone().send() => res,
        };

        let messages = match res {
            Ok(res) => res.messages.unwrap_or_default(),
            Err(error) => {
                println!("Failed to read messages from input queue {input_url}: {error}");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };

        for Message {
            body,
            message_attributes,
            message_id,
            receipt_handle,
            mut attributes,
            ..
        } in messages
        {
            println!("Received message: queue_url=[{input_url}], id=[{message_id:?}], message_attributes=[{message_attributes:?}], body=[{body:?}]");

            let group_id = attributes
                .as_mut()
                .and_then(|attr_map| attr_map.remove(&MessageSystemAttributeName::MessageGroupId));

            let deduplication_id = attributes.and_then(|mut attr_map| {
                attr_map.remove(&MessageSystemAttributeName::MessageDeduplicationId)
            });

            println!("Forwaring message to {output_url}");
            if let Err(error) = client
                .send_message()
                .queue_url(&output_url)
                .set_message_attributes(message_attributes)
                .set_message_group_id(group_id)
                .set_message_deduplication_id(deduplication_id)
                .set_message_body(body)
                .send()
                .await
            {
                println!("Failed to forward message to {output_url}: {error}");
            } else if let Some(handle) = receipt_handle {
                client
                    .delete_message()
                    .queue_url(&output_url)
                    .receipt_handle(handle)
                    .send()
                    .await
                    .unwrap();
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let sdk_config = aws_config::load_from_env().await;
    let client = Client::new(&sdk_config);
    let queues = configuration::from_env();

    let mut tasks = JoinSet::new();
    let cancellation_token = CancellationToken::new();
    let signal_token = cancellation_token.clone();
    let mut signal = signal(SignalKind::terminate()).unwrap();
    tasks.spawn(async move {
        let _ = signal.recv().await.unwrap();
        signal_token.cancel();
    });

    for config in queues {
        tasks.spawn(proxy_messages(
            config,
            client.clone(),
            cancellation_token.clone(),
        ));
    }

    tasks.join_all().await;
}
