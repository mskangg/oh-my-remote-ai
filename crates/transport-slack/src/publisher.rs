use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hyper_rustls::HttpsConnectorBuilder;
use slack_morphism::prelude::*;

use crate::formatting::{
    build_channel_message_request, build_status_delete_request, build_status_update_request,
    build_thread_message_request, build_thread_message_request_with_blocks, split_for_slack_final_reply,
    to_plain_fallback,
};
use crate::ports::{SlackSessionPublisher, SlackWorkingStatusPublisher};
use crate::types::{SlackMessageTarget, SlackPostedMessage, SlackThreadStatus};

pub struct SlackWebApiPublisher {
    client: Arc<SlackClient<SlackClientHyperHttpsConnector>>,
    bot_token: SlackApiToken,
}

pub(crate) fn build_slack_https_connector() -> SlackClientHyperHttpsConnector {
    HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_only()
        .enable_http2()
        .build()
        .into()
}

impl SlackWebApiPublisher {
    pub fn new(bot_token: impl Into<String>) -> Result<Self> {
        Ok(Self {
            client: Arc::new(SlackClient::new(build_slack_https_connector())),
            bot_token: SlackApiToken::new(bot_token.into().into()),
        })
    }

    pub async fn post_thread_message(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String>,
    ) -> Result<SlackPostedMessage> {
        let session = self.client.open_session(&self.bot_token);
        let response = session
            .chat_post_message(&build_thread_message_request(target, text))
            .await?;

        Ok(SlackPostedMessage {
            channel_id: response.channel.to_string(),
            message_ts: response.ts.to_string(),
        })
    }

    pub async fn post_thread_message_with_blocks(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String>,
        blocks: Vec<SlackBlock>,
    ) -> Result<SlackPostedMessage> {
        let session = self.client.open_session(&self.bot_token);
        let response = session
            .chat_post_message(&build_thread_message_request_with_blocks(target, text, blocks))
            .await?;

        Ok(SlackPostedMessage {
            channel_id: response.channel.to_string(),
            message_ts: response.ts.to_string(),
        })
    }

    pub async fn post_channel_message(
        &self,
        channel_id: &str,
        text: impl Into<String>,
    ) -> Result<SlackPostedMessage> {
        let session = self.client.open_session(&self.bot_token);
        let response = session
            .chat_post_message(&build_channel_message_request(channel_id, text))
            .await?;

        Ok(SlackPostedMessage {
            channel_id: response.channel.to_string(),
            message_ts: response.ts.to_string(),
        })
    }

    pub async fn get_message_permalink(
        &self,
        channel_id: &str,
        message_ts: &str,
    ) -> Result<url::Url> {
        let session = self.client.open_session(&self.bot_token);
        let response = session
            .chat_get_permalink(&SlackApiChatGetPermalinkRequest {
                channel: SlackChannelId(channel_id.to_string()),
                message_ts: SlackTs(message_ts.to_string()),
            })
            .await?;
        Ok(response.permalink)
    }

    pub async fn update_message(
        &self,
        posted: &SlackPostedMessage,
        text: impl Into<String>,
    ) -> Result<()> {
        let session = self.client.open_session(&self.bot_token);
        session
            .chat_update(&build_status_update_request(posted, text))
            .await?;
        Ok(())
    }

    pub async fn post_working_status(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String>,
    ) -> Result<SlackThreadStatus> {
        let posted = self.post_thread_message(target, text).await?;

        Ok(SlackThreadStatus {
            channel_id: posted.channel_id,
            thread_ts: target.thread_ts.clone(),
            status_message_ts: posted.message_ts,
        })
    }

    pub async fn update_working_status(
        &self,
        status: &SlackThreadStatus,
        text: impl Into<String>,
    ) -> Result<()> {
        self.update_message(
            &SlackPostedMessage {
                channel_id: status.channel_id.clone(),
                message_ts: status.status_message_ts.clone(),
            },
            text,
        )
        .await
    }

    pub async fn delete_message(&self, status: &SlackThreadStatus) -> Result<()> {
        let session = self.client.open_session(&self.bot_token);
        session
            .chat_delete(&build_status_delete_request(status))
            .await?;
        Ok(())
    }

    pub async fn post_final_reply(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String>,
    ) -> Result<SlackPostedMessage> {
        let text = text.into();
        let mut last_posted = None;

        for chunk in split_for_slack_final_reply(&text) {
            // SlackMarkdownBlock serializes as { "type": "markdown" } which Slack
            // renders as CommonMark — the same format Claude produces. Passing the
            // original text avoids the need for mrkdwn conversion and correctly
            // handles **bold**, headings, code blocks, and inline code spans.
            let fallback = to_plain_fallback(&chunk);
            let posted = self
                .post_thread_message_with_blocks(
                    target,
                    &fallback,
                    vec![SlackMarkdownBlock {
                        block_id: None,
                        text: chunk,
                    }
                    .into()],
                )
                .await?;
            last_posted = Some(posted);
        }

        last_posted.ok_or_else(|| anyhow::anyhow!("final reply text is empty"))
    }
}

#[async_trait]
impl SlackWorkingStatusPublisher for SlackWebApiPublisher {
    async fn post_working_status(
        &self,
        target: &SlackMessageTarget,
        text: impl Into<String> + Send,
    ) -> Result<SlackThreadStatus> {
        SlackWebApiPublisher::post_working_status(self, target, text).await
    }
}

#[async_trait]
impl SlackSessionPublisher for SlackWebApiPublisher {
    async fn post_channel_message(&self, channel_id: &str, text: &str) -> Result<SlackPostedMessage> {
        SlackWebApiPublisher::post_channel_message(self, channel_id, text).await
    }

    async fn post_thread_message_with_blocks(
        &self,
        target: &SlackMessageTarget,
        text: &str,
        blocks: Vec<SlackBlock>,
    ) -> Result<SlackPostedMessage> {
        SlackWebApiPublisher::post_thread_message_with_blocks(self, target, text, blocks).await
    }

    async fn update_working_status(&self, status: &SlackThreadStatus, text: &str) -> Result<()> {
        SlackWebApiPublisher::update_working_status(self, status, text).await
    }

    async fn delete_message(&self, status: &SlackThreadStatus) -> Result<()> {
        SlackWebApiPublisher::delete_message(self, status).await
    }

    async fn get_message_permalink(&self, channel_id: &str, message_ts: &str) -> Result<String> {
        Ok(SlackWebApiPublisher::get_message_permalink(self, channel_id, message_ts)
            .await?
            .to_string())
    }

    async fn post_final_reply(
        &self,
        target: &SlackMessageTarget,
        text: &str,
    ) -> Result<SlackPostedMessage> {
        SlackWebApiPublisher::post_final_reply(self, target, text).await
    }
}
