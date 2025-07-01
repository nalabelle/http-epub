.PHONY: android-build android-clean help

# Default target
help:
	@echo "Available targets:"
	@echo "  android-build      - Build Android binary (optimized with caching)"
	@echo "  android-clean      - Clean build cache and artifacts"

deps-install:
	@echo "Installing dependencies..."
	rustup default stable
	cargo fetch

# Standard Android build with caching
android-build:
	@echo "Building Android binary with optimizations..."
	DOCKER_BUILDKIT=1 docker buildx bake android-build
	@echo "Build complete! Binary available in ./dist/http-epub-android"

# Clean build cache and artifacts
android-clean:
	@echo "Cleaning Android build cache and artifacts..."
	rm -rf ./dist
	rm -rf ./build
	docker system prune -f --filter label=stage=android-build
	@echo "Clean complete!"

serve:
	static-web-server --port 8787 --root ./live
