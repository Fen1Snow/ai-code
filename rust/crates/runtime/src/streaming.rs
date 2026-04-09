// 流式响应支持 - SSE 流式输出
// 实现 token 级流式响应

use std::collections::VecDeque;
use tokio::sync::{mpsc, Mutex};

/// 流式响应块
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub is_done: bool,
    pub usage: Option<UsageInfo>,
}

/// 使用统计信息
#[derive(Debug, Clone)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 流式响应构建器
pub struct StreamBuilder {
    buffer: VecDeque<String>,
    done: bool,
    usage: Option<UsageInfo>,
}

impl StreamBuilder {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
            done: false,
            usage: None,
        }
    }

    pub fn push(&mut self, content: String) {
        self.buffer.push_back(content);
    }

    pub fn finish(&mut self, usage: UsageInfo) {
        self.done = true;
        self.usage = Some(usage);
    }

    pub fn next_chunk(&mut self) -> Option<StreamChunk> {
        if let Some(content) = self.buffer.pop_front() {
            Some(StreamChunk {
                content,
                is_done: false,
                usage: None,
            })
        } else if self.done {
            // 返回结束 chunk，并清除 done 标志以防止重复返回
            self.done = false;
            Some(StreamChunk {
                content: String::new(),
                is_done: true,
                usage: self.usage.take(),
            })
        } else {
            None
        }
    }
}

impl Default for StreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 流式响应发送器
pub struct StreamSender {
    tx: mpsc::Sender<StreamChunk>,
}

impl StreamSender {
    pub fn new(tx: mpsc::Sender<StreamChunk>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, content: String) -> Result<(), mpsc::error::SendError<StreamChunk>> {
        self.tx
            .send(StreamChunk {
                content,
                is_done: false,
                usage: None,
            })
            .await
    }

    pub async fn finish(
        &self,
        usage: UsageInfo,
    ) -> Result<(), mpsc::error::SendError<StreamChunk>> {
        self.tx
            .send(StreamChunk {
                content: String::new(),
                is_done: true,
                usage: Some(usage),
            })
            .await
    }
}

/// 流式响应接收器
pub struct StreamReceiver {
    rx: Mutex<mpsc::Receiver<StreamChunk>>,
}

impl StreamReceiver {
    pub fn new(rx: mpsc::Receiver<StreamChunk>) -> Self {
        Self {
            rx: Mutex::new(rx),
        }
    }

    pub async fn next(&self) -> Option<StreamChunk> {
        let mut rx = self.rx.lock().await;
        rx.recv().await
    }
}

/// 创建流式通道
pub fn create_stream_channel(
) -> (StreamSender, StreamReceiver) {
    let (tx, rx) = mpsc::channel(100);
    (StreamSender::new(tx), StreamReceiver::new(rx))
}

/// SSE 格式化工具
pub struct SseFormatter;

impl SseFormatter {
    /// 格式化为 SSE 格式
    pub fn format(chunk: &StreamChunk) -> String {
        if chunk.is_done {
            return "data: [DONE]\n\n".to_string();
        }

        let json = serde_json::json!({
            "choices": [{
                "delta": {
                    "content": chunk.content
                },
                "finish_reason": null,
                "index": 0
            }]
        });

        format!("data: {}\n\n", json)
    }

    /// 批量格式化
    pub fn format_batch(chunks: &[StreamChunk]) -> String {
        chunks
            .iter()
            .map(|chunk| Self::format(chunk))
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_stream_builder() {
        let mut builder = StreamBuilder::new();
        builder.push("Hello ".to_string());
        builder.push("World".to_string());
        builder.finish(UsageInfo {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        });

        let chunk1 = builder.next_chunk().unwrap();
        assert_eq!(chunk1.content, "Hello ");
        assert!(!chunk1.is_done);

        let chunk2 = builder.next_chunk().unwrap();
        assert_eq!(chunk2.content, "World");
        assert!(!chunk2.is_done);

        let chunk3 = builder.next_chunk().unwrap();
        assert!(chunk3.is_done);
        assert_eq!(chunk3.usage.unwrap().total_tokens, 30);
    }

    #[tokio::test]
    async fn test_stream_channel() {
        let (sender, receiver) = create_stream_channel();

        // 发送端
        let sender_task = tokio::spawn(async move {
            sender.send("Hello ".to_string()).await.unwrap();
            sender.send("World".to_string()).await.unwrap();
            sender
                .finish(UsageInfo {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                })
                .await
                .unwrap();
        });

        // 接收端
        let receiver_task = tokio::spawn(async move {
            let mut chunks = Vec::new();
            while let Some(chunk) = receiver.next().await {
                chunks.push(chunk);
            }
            chunks
        });

        sender_task.await.unwrap();
        let chunks = receiver_task.await.unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "Hello ");
        assert_eq!(chunks[1].content, "World");
        assert!(chunks[2].is_done);
    }

    #[tokio::test]
    async fn test_sse_formatter() {
        let chunk = StreamChunk {
            content: "Hello".to_string(),
            is_done: false,
            usage: None,
        };

        let sse = SseFormatter::format(&chunk);
        assert!(sse.starts_with("data: "));
        assert!(sse.contains("Hello"));
    }
}
