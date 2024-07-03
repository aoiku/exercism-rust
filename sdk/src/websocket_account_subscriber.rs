use std::marker::PhantomData;

use async_trait::async_trait;
use futures_util::StreamExt;
use log::info;
use solana_account_decoder::{UiAccount, UiAccountEncoding};
use solana_client::{nonblocking::pubsub_client::PubsubClient, rpc_config::RpcAccountInfoConfig};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};

use crate::{
    accounts::AccountSubscriber,
    event_emitter::{Event, EventEmitter},
    types::SdkError,
    SdkResult,
};

#[derive(Clone, Debug)]
pub(crate) struct AccountUpdate {
    pub pubkey: String,
    pub data: UiAccount,
    pub slot: u64,
}

impl Event for AccountUpdate {
    fn box_clone(&self) -> Box<dyn Event> {
        Box::new((*self).clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone)]
pub struct WebsocketAccountSubscriber<T> {
    subscription_name: &'static str,
    url: String,
    pubkey: Pubkey,
    pub(crate) commitment: CommitmentConfig,
    pub subscribed: bool,
    pub event_emitter: EventEmitter,
    unsubscriber: Option<tokio::sync::mpsc::Sender<()>>,
    _phantom: PhantomData<T>,
}

impl<T> WebsocketAccountSubscriber<T> {
    pub fn new(
        subscription_name: &'static str,
        url: &str,
        pubkey: Pubkey,
        commitment: CommitmentConfig,
        event_emitter: EventEmitter,
    ) -> Self {
        WebsocketAccountSubscriber {
            subscription_name,
            url: url.to_string(),
            pubkey,
            commitment,
            subscribed: false,
            event_emitter,
            unsubscriber: None,
            _phantom: PhantomData,
        }
    }

    pub async fn subscribe(&mut self) -> SdkResult<()> {
        if self.subscribed {
            return Ok(());
        }

        self.subscribed = true;
        self.subscribe_ws().await?;
        Ok(())
    }

    async fn subscribe_ws(&mut self) -> SdkResult<()> {
        let account_config = RpcAccountInfoConfig {
            commitment: Some(self.commitment),
            encoding: Some(UiAccountEncoding::Base64),
            ..RpcAccountInfoConfig::default()
        };
        let (unsub_tx, mut unsub_rx) = tokio::sync::mpsc::channel::<()>(1);
        self.unsubscriber = Some(unsub_tx);

        let mut attempt = 0;
        let max_reconnection_attempts = 20;
        let base_delay = tokio::time::Duration::from_secs(2);

        let url = self.url.clone();

        info!("subscribing {}", self.subscription_name);

        tokio::spawn({
            let event_emitter = self.event_emitter.clone();
            let mut latest_slot = 0;
            let subscription_name = self.subscription_name;
            let pubkey = self.pubkey;
            async move {
                loop {
                    let pubsub = PubsubClient::new(&url).await?;

                    match pubsub
                        .account_subscribe(&pubkey, Some(account_config.clone()))
                        .await
                    {
                        Ok((mut account_updates, account_unsubscribe)) => loop {
                            attempt = 0;
                            tokio::select! {
                                message = account_updates.next() => {
                                    match message {
                                        Some(message) => {
                                            let slot = message.context.slot;
                                            if slot >= latest_slot {
                                                latest_slot = slot;
                                                let account_update = AccountUpdate {
                                                    pubkey: pubkey.to_string(),
                                                    data: message.value,
                                                    slot,
                                                };
                                                event_emitter.emit(subscription_name, Box::new(account_update));
                                            }
                                        }
                                        None => {
                                            log::warn!("{}: Account stream interrupted", subscription_name);
                                            account_unsubscribe().await;
                                            break;
                                        }
                                    }
                                }
                                unsub = unsub_rx.recv() => {
                                    if unsub.is_some() {
                                        log::debug!("{}: Unsubscribing from account stream", subscription_name);
                                        account_unsubscribe().await;
                                        return Ok(());

                                    }
                                }
                            }
                        },
                        Err(e) => {
                            log::error!("{subscription_name}: Failed to subscribe to account stream, retrying: {e}");
                            attempt += 1;
                            log::info!("Number of attempt: {attempt}");
                            if attempt >= max_reconnection_attempts {
                                log::error!("Max reconnection attempts {attempt} reached.");
                                return Err(SdkError::MaxReconnectionAttemptsReached);
                            }
                        }
                    }

                    if attempt >= max_reconnection_attempts {
                        log::error!("{}: Max reconnection attempts reached", subscription_name);
                        return Err(crate::SdkError::MaxReconnectionAttemptsReached);
                    }

                    let delay_duration = base_delay * 2_u32.pow(attempt);
                    log::debug!(
                        "{}: Reconnecting in {:?}",
                        subscription_name,
                        delay_duration
                    );
                    tokio::time::sleep(delay_duration).await;
                    attempt += 1;
                }
            }
        });
        Ok(())
    }

    pub async fn unsubscribe(&mut self) -> SdkResult<()> {
        if self.subscribed && self.unsubscriber.is_some() {
            if let Err(e) = self.unsubscriber.as_ref().unwrap().send(()).await {
                log::error!("Failed to send unsubscribe signal: {:?}", e);
                return Err(crate::SdkError::CouldntUnsubscribe(e));
            }
            self.subscribed = false;
        }
        Ok(())
    }
}

#[async_trait]
impl<T> AccountSubscriber<T> for WebsocketAccountSubscriber<T>
where
    T: Send + Sync,
{
    async fn subscribe<F: FnMut(T) + std::marker::Send>(&mut self, _on_change: F) {}

    async fn fetch(&mut self) -> SdkResult<()> {
        Ok(())
    }

    async fn unsubscribe(&self) {}

    fn set_data(&mut self, _user_account: T, slot: Option<u64>) {
        let _new_slot = slot.unwrap_or(0);

        // if self
    }
}
