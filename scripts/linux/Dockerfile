FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git build-essential pkg-config \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev \
    libvulkan1 mesa-vulkan-drivers libegl1 libgl1 \
    xvfb xauth x11-utils xdotool xclip \
    fonts-dejavu-core fonts-noto-cjk fonts-noto-core \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
RUN git clone --depth 1 --branch develop https://github.com/almide/almide.git /opt/almide \
    && cd /opt/almide && cargo build --release && ln -s /opt/almide/target/release/almide /usr/local/bin/almide
WORKDIR /src
