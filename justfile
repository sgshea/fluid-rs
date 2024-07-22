# Run using dynamic linking
run:
    cargo run --features bevy/dynamic_linking

# Following instructions to comiple and run the project for WASM

# Install dependencies
install-deps:
    rustup target add wasm32-unknown-unknown
    cargo install wasm-bindgen-cli
    cargo install wasm-opt
    RUSTC_BOOTSTRAP=1 cargo install --git https://github.com/thecoshman/http

# Remove previous build
clean:
    rm -rf wasm

# Compile the wasm file
compile-wasm:
    RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build --profile wasm-release --target wasm32-unknown-unknown

# Use wasm-bindgen to generate JS bindings and move files
generate-js:
    wasm-bindgen --no-typescript --target web --out-dir ./wasm/ --out-name "fluid" ./target/wasm32-unknown-unknown/wasm-release/fluid_sim.wasm

# Move index.html and assets
move-files:
    cp index.html wasm/

# Optionally optimize the wasm file size
optimize-wasm:
    wasm-opt -Oz --output wasm/fluid_bg.wasm wasm/fluid_bg.wasm

# Run local server
run-server:
    cd wasm && http

# Full build and run sequence
build-wasm: clean compile-wasm generate-js move-files optimize-wasm

build-and-run-wasm: build-wasm run-server