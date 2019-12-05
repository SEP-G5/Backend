FROM fedora:30

# General
RUN dnf update -y
RUN dnf install -y make gcc clang git libtool pkg-config openssl-devel bzip2-devel

# Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly -y
ENV PATH "/root/.cargo/bin:$PATH"
RUN rustup update

# Backend
RUN git clone https://github.com/SEP-G5/Backend
RUN cd Backend && cargo build --release

EXPOSE 35010
