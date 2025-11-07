default:
    @just --list

setup:
    rustup component add rust-src
    pip install -U git+https://github.com/TuxSH/firmtool.git

clippy:
    cargo clippy -p threemu -p oxidiz3ds-hw
    RUSTC_BOOTSTRAP=1 cargo clippy -Z build-std=core --target thumbv5te-none-eabi -p threemu-test-arm9
    RUSTC_BOOTSTRAP=1 cargo clippy -Z build-std=core --target tests/threemu-test-arm11/armv6k-none-eabihf.json -p threemu-test-arm11

emu *ARGS:
    cargo run --bin threemu -- {{ARGS}}

emu-linux IMG:
    cargo run --bin threemu -- \
    luma/payloads/firm_linux_loader.firm \
    --sd-card {{IMG}} \
    --entry-firm-in-sd-card

build-arm9-tests:
    RUSTC_BOOTSTRAP=1 cargo build -Z build-std=core --target thumbv5te-none-eabi -p threemu-test-arm9

build-arm11-tests:
    RUSTC_BOOTSTRAP=1 cargo build -Z build-std=core --target tests/threemu-test-arm11/armv6k-none-eabihf.json -p threemu-test-arm11

build-tests: build-arm9-tests build-arm11-tests

test-firm NAME: build-tests
    @mkdir -p target/firm
    firmtool build target/firm/{{NAME}}.firm \
        -D target/thumbv5te-none-eabi/debug/{{NAME}} target/armv6k-none-eabihf/debug/{{NAME}} \
        -C NDMA XDMA -i
    cargo run --bin threemu-cli -- \
        --arm9-stop-pc 0xF0000000 \
        --arm11-stop-pc 0xF0000000 \
        --max-instructions 100000 \
        target/firm/{{NAME}}.firm

test-firms: (test-firm "minimal_pass")

test-linux-loader IMG:
    cargo run --bin threemu-cli -- \
        --arm9-stop-pc 0x08080000 \
        --arm11-stop-pc 0x20008000 \
        --max-instructions 1000000000 \
        luma/payloads/firm_linux_loader.firm \
        --sd-card {{IMG}} \
        --entry-firm-in-sd-card
