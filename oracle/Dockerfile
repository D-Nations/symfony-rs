# The PHP oracle for symfony-rs: the PHP version symfony-demo-rs's vectors
# were recorded with, plus the extensions the pinned packages need.
# Recording and comparison only; never deployed.
FROM php:8.4.27-cli-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends libicu-dev libsqlite3-dev \
    && docker-php-ext-install intl pdo_sqlite \
    && rm -rf /var/lib/apt/lists/*

# /work is this repository (read-only). /demo is a symfony/demo checkout
# whose vendor/ holds the pinned packages (parity/packages.tsv, checked by
# scripts/fetch-upstream.sh --verify); recorders load /demo/vendor/autoload.php.
WORKDIR /work
ENTRYPOINT ["php", "-d", "display_errors=stderr"]
