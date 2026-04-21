use crate::types::{SlackMessageTarget, SlackPostedMessage, SlackThreadStatus};
use slack_morphism::prelude::*;

#[cfg(test)]
pub(crate) fn parse_thread_reply(envelope: crate::types::SlackEnvelope) -> Option<crate::types::SlackThreadReply> {
    if envelope.bot_id.is_some() {
        return None;
    }

    if let Some(ref subtype) = envelope.subtype {
        if subtype != "thread_broadcast" {
            return None;
        }
    }

    Some(crate::types::SlackThreadReply {
        channel_id: envelope.channel?,
        thread_ts: envelope.thread_ts?,
        text: envelope.text?,
        user_id: envelope.user?,
    })
}

pub fn parse_push_thread_reply(event: &SlackPushEventCallback) -> Option<crate::types::SlackThreadReply> {
    let SlackEventCallbackBody::Message(message) = &event.event else {
        return None;
    };

    let channel_id = message.origin.channel.as_ref()?.to_string();
    let thread_ts = message.origin.thread_ts.as_ref()?.to_string();
    let text = message.content.as_ref()?.text.clone()?;
    let user_id = message.sender.user.as_ref()?.to_string();

    if message.sender.bot_id.is_some() {
        return None;
    }

    if let Some(subtype) = &message.subtype {
        if *subtype != SlackMessageEventType::ThreadBroadcast {
            return None;
        }
    }

    Some(crate::types::SlackThreadReply {
        channel_id,
        thread_ts,
        text,
        user_id,
    })
}

pub fn build_thread_message_request(
    target: &SlackMessageTarget,
    text: impl Into<String>,
) -> SlackApiChatPostMessageRequest {
    SlackApiChatPostMessageRequest::new(
        SlackChannelId(target.channel_id.clone()),
        SlackMessageContent::new().with_text(text.into()),
    )
    .with_thread_ts(SlackTs(target.thread_ts.clone()))
}

pub fn build_thread_message_request_with_blocks(
    target: &SlackMessageTarget,
    text: impl Into<String>,
    blocks: Vec<SlackBlock>,
) -> SlackApiChatPostMessageRequest {
    SlackApiChatPostMessageRequest::new(
        SlackChannelId(target.channel_id.clone()),
        SlackMessageContent {
            text: Some(text.into()),
            blocks: Some(blocks),
            attachments: None,
            upload: None,
            files: None,
            reactions: None,
            metadata: None,
        },
    )
    .with_thread_ts(SlackTs(target.thread_ts.clone()))
}

pub fn build_channel_message_request(
    channel_id: impl Into<String>,
    text: impl Into<String>,
) -> SlackApiChatPostMessageRequest {
    SlackApiChatPostMessageRequest::new(
        SlackChannelId(channel_id.into()),
        SlackMessageContent::new().with_text(text.into()),
    )
}

const SLACK_FINAL_REPLY_TEXT_LIMIT: usize = 2_500;

/// Returns true if `line` is a CommonMark fenced code block delimiter.
/// Handles ``` (3+ backticks) and ~~~ (3+ tildes), with optional indentation.
fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let backticks = trimmed.chars().take_while(|&c| c == '`').count();
    let tildes = trimmed.chars().take_while(|&c| c == '~').count();
    backticks >= 3 || tildes >= 3
}

/// Returns the byte offset of the last open (unclosed) fence before `up_to`,
/// or `None` if the prefix is not inside a code block.
fn code_block_open_byte(text: &str, up_to: usize) -> Option<usize> {
    let prefix = &text[..up_to];
    let mut in_block = false;
    let mut block_start: Option<usize> = None;
    let mut byte_pos = 0;

    for line in prefix.lines() {
        if is_fence_line(line) {
            if in_block {
                in_block = false;
                block_start = None;
            } else {
                in_block = true;
                block_start = Some(byte_pos);
            }
        }
        byte_pos += line.len() + 1; // +1 for the '\n'
    }

    if in_block { block_start } else { None }
}

/// Find the byte offset just after the closing fence, or `None` if unclosed.
fn code_block_close_byte(text: &str) -> Option<usize> {
    let mut in_block = false;
    let mut byte_pos = 0;

    for line in text.lines() {
        let end = byte_pos + line.len() + 1;
        if is_fence_line(line) {
            if in_block {
                return Some(end.min(text.len()));
            }
            in_block = true;
        }
        byte_pos = end;
    }
    None
}

pub(crate) fn split_for_slack_final_reply(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut remaining = text.trim();

    // char_indices().nth() short-circuits at LIMIT — O(LIMIT) per iteration
    // instead of the O(n) that chars().count() caused (O(n²) total).
    while let Some((byte_limit, _)) = remaining.char_indices().nth(SLACK_FINAL_REPLY_TEXT_LIMIT) {
        let slice = &remaining[..byte_limit];

        let natural_split = slice
            .rfind("\n\n")
            .map(|i| i + 2)
            .or_else(|| slice.rfind('\n').map(|i| i + 1))
            .or_else(|| slice.rfind(' ').map(|i| i + 1))
            .unwrap_or(byte_limit);

        // If the natural split point lands inside a fenced code block:
        // - If the fence starts before it, back up to before the fence.
        // - If the fence starts at 0 (whole limit is one code block), try to
        //   extend to the closing fence — but cap the extension so we never send
        //   a chunk that grossly exceeds the limit (Slack has block size caps).
        const CODE_BLOCK_EXTEND_CAP: usize = 1_000;
        let split_at = match code_block_open_byte(remaining, natural_split) {
            Some(0) => match code_block_close_byte(remaining) {
                Some(close) if close <= byte_limit + CODE_BLOCK_EXTEND_CAP => close,
                // Block is longer than limit + cap — we cannot keep it intact.
                // Split at the natural word/paragraph boundary inside the fence.
                // The chunk will contain an unclosed fence, which is a known
                // limitation for very large code samples (>3500 chars).
                _ => natural_split,
            },
            Some(fence_start) => fence_start,
            None => natural_split,
        };

        let chunk = remaining[..split_at].trim_end();
        if !chunk.is_empty() {
            chunks.push(chunk.to_string()); // single allocation per chunk
        }
        // Only skip newlines between chunks — preserve leading spaces/tabs that
        // are significant in CommonMark (indented code, nested lists).
        remaining = remaining[split_at..].trim_start_matches(['\n', '\r']);
    }

    if !remaining.is_empty() {
        chunks.push(remaining.to_string());
    }

    chunks
}

/// Strip inline markdown (`**bold**`, `` `code` ``, fenced blocks) in a single pass.
fn strip_markdown_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next(); // skip second *
            }
            '`' => {}
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn to_plain_fallback(text: &str) -> String {
    // Longest prefix first so "### " is matched before "# ".
    const HEADING_PREFIXES: &[&str] = &["### ", "## ", "# "];

    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if let Some(rest) = HEADING_PREFIXES.iter().find_map(|p| trimmed.strip_prefix(p)) {
                return rest.trim().to_string();
            }
            let mut s = strip_markdown_inline(line);
            s.truncate(s.trim_end().len());
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Convert Claude's CommonMark markdown to Slack mrkdwn format for Block Kit rendering.
///
/// Slack's `text` field has an inconsistent mrkdwn parser; Block Kit section blocks
/// with explicit `type: "mrkdwn"` are reliable. This function converts Claude output
/// into that format.
pub fn claude_md_to_slack_mrkdwn(text: &str) -> String {
    const HEADING_PREFIXES: &[&str] = &["### ", "## ", "# "];
    let mut result = String::with_capacity(text.len());
    let mut in_code_block = false;

    for line in text.lines() {
        // Preserve code blocks verbatim — don't alter content inside them.
        if in_code_block {
            result.push_str(line);
            result.push('\n');
            if line.trim_start().starts_with("```") {
                in_code_block = false;
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            in_code_block = true;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let trimmed = line.trim_start();

        // Headings → *bold*
        if let Some(rest) = HEADING_PREFIXES.iter().find_map(|p| trimmed.strip_prefix(p)) {
            result.push('*');
            result.push_str(rest.trim());
            result.push('*');
            result.push('\n');
            continue;
        }

        // Horizontal rules → empty line
        if matches!(trimmed, "---" | "***" | "___") {
            result.push('\n');
            continue;
        }

        // Star list items → bullet (avoids Slack parser confusion with bold *)
        if let Some(rest) = trimmed.strip_prefix("* ") {
            result.push_str("• ");
            result.push_str(&bold_to_slack_mrkdwn(rest));
            result.push('\n');
            continue;
        }

        result.push_str(&bold_to_slack_mrkdwn(line));
        result.push('\n');
    }

    result.trim_end().to_string()
}

/// Replace `**bold**` with `*bold*` for Slack mrkdwn.
/// Only converts complete pairs; unmatched `**` (e.g. `2 ** 8`) is preserved.
fn bold_to_slack_mrkdwn(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(open) = remaining.find("**") {
        out.push_str(&remaining[..open]);
        let after_open = &remaining[open + 2..];
        if let Some(close) = after_open.find("**") {
            // Complete pair found — convert.
            out.push('*');
            out.push_str(&after_open[..close]);
            out.push('*');
            remaining = &after_open[close + 2..];
        } else {
            // No closing ** — preserve literally.
            out.push_str("**");
            remaining = after_open;
        }
    }
    out.push_str(remaining);
    out
}


pub fn build_status_update_request(
    posted: &SlackPostedMessage,
    text: impl Into<String>,
) -> SlackApiChatUpdateRequest {
    SlackApiChatUpdateRequest::new(
        SlackChannelId(posted.channel_id.clone()),
        SlackMessageContent::new().with_text(text.into()),
        SlackTs(posted.message_ts.clone()),
    )
}

pub fn build_status_delete_request(status: &SlackThreadStatus) -> SlackApiChatDeleteRequest {
    SlackApiChatDeleteRequest {
        channel: SlackChannelId(status.channel_id.clone()),
        ts: SlackTs(status.status_message_ts.clone()),
        as_user: None,
    }
}
