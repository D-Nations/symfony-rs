<?php
// Loads symfony/dependency-injection for the di_* recorders: the demo's
// vendor/ when it is mounted, otherwise the pinned checkouts in upstream/
// (symfony/dependency-injection, symfony/service-contracts, psr/container,
// and symfony/deprecation-contracts when fetched).

if (is_file('/demo/vendor/autoload.php')) {
    require '/demo/vendor/autoload.php';

    return;
}

if (is_file('/work/upstream/symfony/deprecation-contracts/function.php')) {
    require '/work/upstream/symfony/deprecation-contracts/function.php';
}

spl_autoload_register(static function (string $class): void {
    foreach ([
        'Symfony\\Component\\DependencyInjection\\' => '/work/upstream/symfony/dependency-injection/',
        'Symfony\\Contracts\\Service\\' => '/work/upstream/symfony/service-contracts/',
        'Psr\\Container\\' => '/work/upstream/psr/container/src/',
    ] as $prefix => $dir) {
        if (str_starts_with($class, $prefix)) {
            $file = $dir.str_replace('\\', '/', substr($class, \strlen($prefix))).'.php';
            if (is_file($file)) {
                require $file;
            }

            return;
        }
    }
});
