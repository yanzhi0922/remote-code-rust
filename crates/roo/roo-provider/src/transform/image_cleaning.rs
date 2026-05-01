//! Removes image blocks from messages when the provider doesn't support images.
//!
//! Derived from `src/api/transform/image-cleaning.ts`.

use roo_types::api::{ApiMessage, ContentBlock};
use roo_types::model::ModelInfo;

/// Removes image blocks from messages if they are not supported by the provider's model.
///
/// Source: `src/api/transform/image-cleaning.ts` — `maybeRemoveImageBlocks`
pub fn maybe_remove_image_blocks(messages: Vec<ApiMessage>, model_info: &ModelInfo) -> Vec<ApiMessage> {
    // Check model capability ONCE instead of for every message
    let supports_images = model_info.supports_images.unwrap_or(false);

    messages
        .into_iter()
        .map(|message| {
            let content = if !supports_images {
                message
                    .content
                    .into_iter()
                    .map(|block| {
                        if matches!(block, ContentBlock::Image { .. }) {
                            // Convert image blocks to text descriptions.
                            // Note: We can't access the actual image content/url due to API limitations,
                            // but we can indicate that an image was present in the conversation.
                            ContentBlock::Text {
                                text: "[Referenced image in conversation]".to_string(),
                            }
                        } else {
                            block
                        }
                    })
                    .collect()
            } else {
                message.content
            };

            ApiMessage { content, ..message }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::api::{ImageSource, MessageRole};

    #[test]
    fn test_removes_images_when_not_supported() {
        let model_info = ModelInfo {
            supports_images: Some(false),
            ..Default::default()
        };

        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: vec![
                ContentBlock::Text {
                    text: "Look at this".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image.png".to_string(),
                    },
                },
            ],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];

        let result = maybe_remove_image_blocks(messages, &model_info);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 2);
        // First block should be unchanged
        assert!(matches!(&result[0].content[0], ContentBlock::Text { text } if text == "Look at this"));
        // Second block should be converted to text
        assert!(matches!(&result[0].content[1], ContentBlock::Text { text } if text == "[Referenced image in conversation]"));
    }

    #[test]
    fn test_keeps_images_when_supported() {
        let model_info = ModelInfo {
            supports_images: Some(true),
            ..Default::default()
        };

        let messages = vec![ApiMessage {
            role: MessageRole::User,
            content: vec![
                ContentBlock::Text {
                    text: "Look at this".to_string(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: "https://example.com/image.png".to_string(),
                    },
                },
            ],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }];

        let result = maybe_remove_image_blocks(messages, &model_info);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content.len(), 2);
        assert!(matches!(&result[0].content[1], ContentBlock::Image { .. }));
    }
}
