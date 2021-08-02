FROM rust:1-slim as base

USER root
RUN apt-get update && apt-get install -y \
	apt-transport-https \
	ca-certificates \
	curl \
	gnupg \
	--no-install-recommends \
	&& curl -sSL https://dl.google.com/linux/linux_signing_key.pub | apt-key add - \
	&& echo "deb https://dl.google.com/linux/chrome/deb/ stable main" > /etc/apt/sources.list.d/google-chrome.list \
	&& apt-get update && apt-get install -y \
	google-chrome-stable \
	fontconfig \
	fonts-ipafont-gothic \
	fonts-wqy-zenhei \
	fonts-thai-tlwg \
	fonts-kacst \
	fonts-symbola \
	fonts-noto \
	fonts-freefont-ttf \
	--no-install-recommends \
	&& apt-get purge --auto-remove -y curl gnupg \
	&& rm -rf /var/lib/apt/lists/*

FROM base as cargo

WORKDIR /code
RUN cargo init
COPY Cargo.toml /code/Cargo.toml
COPY Cargo.lock /code/Cargo.lock
RUN cargo fetch

FROM cargo AS builder

COPY src /code/src
RUN cargo build --release --jobs 1

FROM base

COPY --from=builder /code/target/release/svg_render_server /app/svg_render_server
RUN chmod o+x /app/svg_render_server

RUN groupadd -r agent && useradd -r -g agent -G audio,video agent \
	&& mkdir -p /home/agent && chown -R agent:agent /home/agent

WORKDIR /app
USER agent
EXPOSE 8080

ENTRYPOINT ["/app/svg_render_server"]