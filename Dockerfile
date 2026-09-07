# syntax=docker/dockerfile:1.7
FROM node:20-bookworm AS frontend-build
WORKDIR /workspace/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run typecheck && npm run build

FROM rust:1-bookworm AS desktop-build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential curl file libayatana-appindicator3-dev librsvg2-dev \
    libssl-dev libwebkit2gtk-4.1-dev libxdo-dev && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs ./src-tauri/
COPY src-tauri/icons/ ./src-tauri/icons/
COPY src-tauri/capabilities/ ./src-tauri/capabilities/
COPY src-tauri/tauri.conf.json ./src-tauri/tauri.conf.json
COPY src-tauri/resources/ ./src-tauri/resources/
COPY src-tauri/src/ ./src-tauri/src/
COPY --from=frontend-build /workspace/frontend/dist ./frontend/dist
WORKDIR /workspace/src-tauri
RUN cargo build --locked

FROM scratch AS checks
COPY --from=frontend-build /workspace/frontend/dist /frontend-dist
COPY --from=desktop-build /workspace/src-tauri/target/debug/lumina /linux-build/lumina
