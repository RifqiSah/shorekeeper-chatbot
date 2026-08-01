# Shorekeeper Chatbot

A chatbot module used by [shorekeeper-rs](https://github.com/RifqiSah/shorekeeper-rs).

This module handles chatbot functionality, including message processing and generating responses. It is separated from the main bot code to keep the project structure cleaner and easier to maintain.

## Features

- Chatbot message handling
- AI response generation
- Conversation context management
- Easy to integrate into the main application

## Usage

This module is only intended to be used by **Shorekeeper**.

Example:

```rust
let chatbot = match config.modules.app_bot_chatbot_enable {
  true => {
    match shorekeeper_chatbot::Chatbot::new().await {
      Ok(c) => Some(Arc::new(c)),
      Err(e) => {
        tracing::error!("Failed to initialize chatbot, running without it: {e}");
        None
      }
    }
  },
  false => None,
};

...

match chatbot.handle_message(&message.author.id.to_string(), Some(&guild), &clean_content, false).await {
  Ok(res) => { message.reply(&ctx.http, res.reply).await.ok(); }
  Err(e) => {
    ...
  }
}
```

## Notes

This is an internal module and is not intended as a standalone chatbot framework.
