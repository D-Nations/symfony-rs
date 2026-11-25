# PHP-FPM for the recorders that need PHP's FastCGI SAPI
# (oracle/recorders/runtime_fpm.php): the oracle's PHP version and libc
# (Dockerfile: php:8.4.27-cli-bookworm), with no php.ini, as there. No
# extension is added: the recorder loads no package.
# Recording and comparison only; never deployed.
FROM php:8.4.27-fpm-bookworm

WORKDIR /work
ENTRYPOINT ["php", "-d", "display_errors=stderr"]
