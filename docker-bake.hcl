# Docker Bake file for Android cross-compilation

variable "RUST_VERSION" {
  default = "1.87"
}

variable "ANDROID_NDK_VERSION" {
  default = "r26c"
}

group "default" {
  targets = ["android-builder"]
}

target "android-builder" {
  dockerfile = "Dockerfile.android"
  tags = ["http-epub-android:latest"]
  platforms = ["linux/amd64"]
  output = ["type=docker"]
}

target "android-build" {
  contexts = {
    builder = "target:android-builder"
  }
  dockerfile-inline = <<-EOT
    # Build stage
    FROM builder AS build-stage
    WORKDIR /workspace

    # Copy dependency files first for better caching
    COPY Cargo.toml Cargo.lock ./
    RUN cargo fetch --target aarch64-linux-android

    # Copy source code
    COPY src/ src/

    # Build with cache mount for faster rebuilds
    RUN --mount=type=cache,target=/usr/local/cargo/registry \
        --mount=type=cache,target=/workspace/target \
        LIBXML2=/opt/android-libxml2/lib/libxml2.a \
        cargo build --target aarch64-linux-android --release && \
        cp target/aarch64-linux-android/release/http-epub /tmp/http-epub-android

    # Final minimal stage with just the binary
    FROM scratch AS final
    COPY --from=build-stage /tmp/http-epub-android /http-epub-android
  EOT
  target = "final"
  output = ["type=local,dest=./dist"]
}
