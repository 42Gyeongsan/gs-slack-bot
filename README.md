# Slack Bot for 42 Campus

This server interacts with Slack and the internal 42 campus server. Its primary purpose is to provide students with a safe and efficient way to handle infrastructure errors (e.g., PC not working, unable to log in to PC, etc.).

## Features

1. **Reboot**: Restart a specific PC or device.
2. **Home Close**: Securely close a home directory.
3. **Home Reset**: Reset the home directory to default settings.

## Technology

This server is written in Rust and will be executed in a separate VM as a Linux service.

## Build and Execute

### Prerequisites

- Latest version of Cargo
- Rust installed

### Build

To build the project, run:
```bash
cargo build
```

### Execute

To start the server, run:
```bash
cargo run
```

### Environment Variables

You need to provide the following environment variables:

- `SLACK_BOT_SCOPE`
- `SLACK_REDIRECT_HOST`
- `SLACK_TOKEN`
- `SLACK_APP_TOKEN`
- `SLACK_CLIENT_SECRET`
- `SLACK_SIGNING_SECRET`
- `SLACK_CLIENT_ID`
- `FT_API_CLIENT_SECRET`
- `FT_API_CLIENT_UID`
- `STUDENT_STORAGE_API_URL`
- `HOMEMAKER_SECRET_TOKEN`
- `ANSIBLE_CLUSTER_SSH_PORT`
- `STUDENT_STORAGE_SSH_PORT`

And change two const value in lib.rs
- `WAKEUP_WORD`: Your bot's slack internal ID
- `WAKEUP_WORD_FOR_USER`: what user will call when they try to use commend. 

### Server Location

The server must be located where it can access the internal server via SSH.

### Slack API

To send Slack API requests to the server, I used ngrok.

## Contributing

1. Fork the repository.
2. Create your feature branch (`git checkout -b feature/AmazingFeature`).
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`).
4. Push to the branch (`git push origin feature/AmazingFeature`).
5. Open a pull request.
