# Build Stage
FROM ghcr.io/evanrichter/cargo-fuzz:latest AS BUILDER

# Add source code to the build stage.
ADD . /src
WORKDIR /src

# Compile the fuzzers
RUN cd boson && cargo +nightly fuzz build

# Package stage
FROM ubuntu:latest AS PACKAGE

# Copy the corpora to the final image
COPY --from=BUILDER /src/examples /corpus/programs

# Copy the fuzzers to the final image
COPY --from=BUILDER /src/boson/fuzz/target/x86_64-unknown-linux-gnu/release/fuzz_* /fuzzers/