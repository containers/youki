FROM mcr.microsoft.com/vscode/devcontainers/rust:1

WORKDIR /workspaces
RUN curl -fsSL get.docker.com -o get-docker.sh && sh get-docker.sh && rm get-docker.sh
RUN rustup component add rust-src clippy
RUN curl -L https://github.com/rust-analyzer/rust-analyzer/releases/latest/download/rust-analyzer-x86_64-unknown-linux-gnu.gz \
    -o /usr/local/cargo/bin/rust-analyzer.gz && gzip -d /usr/local/cargo/bin/rust-analyzer.gz \
    && chmod +x /usr/local/cargo/bin/rust-analyzer
RUN wget https://dl.google.com/go/go1.13.5.linux-amd64.tar.gz && tar -C /usr/local -xzf go1.13.5.linux-amd64.tar.gz
RUN mkdir /workspaces/go
ENV PATH $PATH:/usr/local/go/bin
ENV GOPATH /workspaces/go
ENV YOUKI_LOGLEVEL debug
ENV RUNTIME /workspaces/youki/target/x86_64-unknown-linux-gnu/debug/youki
ENTRYPOINT ["sh"]
