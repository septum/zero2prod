_default:
    @just --list
    @echo You probably want to run \`setup\` first.

setup:
    rustup component add rustfmt
    rustup component add clippy
    rustup component add llvm-tools-preview
    cargo install cargo-watch
    cargo install cargo-llvm-cov
    cargo install cargo-audit
    cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres
    cargo install cargo-udeps
    cargo install bunyan

dev:
    cargo watch -x check -x test -x run | bunyan

test:
    cargo test

format:
    cargo fmt -- --check

lint:
    SQLX_OFFLINE=true cargo clippy -- -D warnings

coverage:
    cargo llvm-cov

audit:
    cargo audit

expand:
    cargo expand

init_db:
    scripts/init_db.sh

migrate:
    SKIP_DOCKER=true scripts/init_db.sh

drop_db:
    docker kill $(docker ps --filter 'name=postgres' --format '{{'{{.ID}}'}}')

cycle_db:
    @just drop_db init_db

gen_query_cache:
    cargo sqlx prepare --workspace -- --all-targets

rm_udeps:
    cargo +nightly udeps

test_logs:
    TEST_LOG=true cargo test | bunyan

docker_build:
    docker build --tag zero2prod .

docker_run:
    docker run -p 8000:8000 zero2prod | bunyan

doctl_create:
    doctl apps create --spec spec.yaml

doctl_update:
    doctl apps update --spec=spec.yaml $(doctl app list --format ID --no-header)

doctl_delete:
    doctl apps delete $(doctl app list --format ID --no-header)

poll_ingress:
    ./scripts/poll_ingress.sh
