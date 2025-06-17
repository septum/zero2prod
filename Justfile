_default:
    @just --list
    @echo You probably want to run \`setup\` first.

# Install all the dependencies used in the recipes
setup:
    rustup component add rustfmt
    rustup component add clippy
    rustup component add llvm-tools-preview
    cargo install cargo-watch
    cargo install cargo-llvm-cov
    cargo install cargo-expand
    cargo install cargo-audit
    cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres
    cargo install cargo-udeps
    cargo install bunyan

# dev workflow
dev:
    @-just init_db # `-` prefix means to ignore if there's a non-zero status
    @-just init_redis
    @just watch_no_test

# watch multiple cargo commands
watch:
    cargo watch -x check -x 'test --features="mocks"' -x run | bunyan

# watch without tests
watch_no_test:
    cargo watch -x check -x run | bunyan

# watch without tests
watch_test:
    cargo watch -x 'test --features="mocks"'

# Run tests with logs
test_w_logs test='':
    RUST_LOG="sqlx=error,info" TEST_LOG=true cargo test {{test}} | bunyan

# Lint the files in a similar fashion as the CI pipeline
lint:
    SQLX_OFFLINE=true cargo clippy -- -D warnings

# Check files format
format:
    cargo fmt --check

# Precommit git hook
precommit:
    @just format lint check_query_cache test_w_logs

# Test coverage
coverage:
    cargo llvm-cov

# Dependencies audit
audit:
    cargo audit

# Expand macros
expand:
    cargo expand

# Start DB in docker
init_db:
    scripts/init_db.sh

# Start redis in docker
init_redis:
    scripts/init_redis.sh

# Run DB migrations
migrate:
    SKIP_DOCKER=true scripts/init_db.sh

# Stop and remove DB container
drop_db:
    docker kill $(docker ps --filter 'name=postgres' --format '{{'{{.ID}}'}}')

# Drop and init DB
cycle_db:
    @just drop_db init_db

# Generate SQLX query cache
gen_query_cache:
    cargo sqlx prepare --workspace -- --all-targets

# Check SQLX query cache
check_query_cache:
    cargo sqlx prepare --check --workspace -- --all-targets

# Remove unused dependencies
rm_udeps:
    cargo +nightly udeps

# Build docker image
docker_build:
    docker build --tag zero2prod .

# Run docker container
docker_run:
    docker run -p 8000:8000 zero2prod | bunyan

# Create app in digital ocean according to spec
doctl_create:
    doctl apps create --project-id zero2prod --spec spec.yaml

# Update app in DO
doctl_update:
    doctl apps update --spec=spec.yaml $(doctl app list --format ID --no-header)

# Delete app in DO
doctl_delete:
    doctl apps delete $(doctl app list --format ID --no-header)

# Poll until the ingress URL for the DO app appears
poll_ingress:
    ./scripts/poll_ingress.sh

# Send a request to subscriptions
subscribe name='septum' email='me@septum.computer':
    curl -i -X POST \
    --data 'name={{name}}&email={{email}}' \
    http://localhost:8000/subscriptions

# Send a request to newsletters
publish title='New Issue!':
    curl -i -X POST \
    -H 'Content-Type: application/json' \
    --data '{"title":"{{title}}","content":{"html":"<h1>Hello World</h1>","text":"Hello World"}}' \
    http://localhost:8000/newsletters
