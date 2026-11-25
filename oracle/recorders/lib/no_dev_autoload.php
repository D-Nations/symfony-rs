<?php
// The class loader `composer install --no-dev` would have written for a
// project whose vendor/ also holds its dev packages: the oracle's demo
// checkout is installed with them and mounted read-only, so the loader is
// built here instead of with Composer.
//
// From the maps Composer generated (vendor/composer/autoload_*.php), every
// entry that lies in a dev package (installed.json's "dev-package-names")
// or in the root package's autoload-dev directories is dropped; the rest is
// registered on Composer's own ClassLoader, and the "files" entries are
// included as Composer's autoload_real.php does. InstalledVersions is
// reloaded without the dev packages, because Symfony also asks it
// (ContainerBuilder::willBeAvailable()) whether a package is installed.
//
// Returns the registered Composer\Autoload\ClassLoader.

function no_dev_autoload(string $projectDir): Composer\Autoload\ClassLoader
{
    $vendorDir = $projectDir.'/vendor';
    $composerDir = $vendorDir.'/composer';
    require_once $composerDir.'/ClassLoader.php';
    require_once $composerDir.'/InstalledVersions.php';

    $installed = json_decode(file_get_contents($composerDir.'/installed.json'), true, 512, JSON_THROW_ON_ERROR);
    $devNames = array_flip($installed['dev-package-names'] ?? []);
    $excluded = [];
    foreach ($installed['packages'] as $package) {
        if (isset($devNames[$package['name']])) {
            $excluded[] = realpath($composerDir.'/'.$package['install-path']);
        }
    }
    $root = json_decode(file_get_contents($projectDir.'/composer.json'), true, 512, JSON_THROW_ON_ERROR);
    foreach ($root['autoload-dev'] ?? [] as $rules) {
        foreach ($rules as $paths) {
            foreach ((array) $paths as $path) {
                $excluded[] = realpath($projectDir.'/'.$path);
            }
        }
    }
    $excluded = array_values(array_filter($excluded));

    $isExcluded = static function (string $path) use ($excluded): bool {
        $real = realpath($path);
        if (false === $real) {
            return false;
        }
        foreach ($excluded as $dir) {
            if ($real === $dir || str_starts_with($real, $dir.DIRECTORY_SEPARATOR)) {
                return true;
            }
        }

        return false;
    };
    $keep = static fn (array $paths): array => array_values(array_filter($paths, static fn ($p) => !$isExcluded($p)));

    $loader = new Composer\Autoload\ClassLoader($vendorDir);
    foreach (require $composerDir.'/autoload_namespaces.php' as $prefix => $paths) {
        if ($paths = $keep($paths)) {
            $loader->set($prefix, $paths);
        }
    }
    foreach (require $composerDir.'/autoload_psr4.php' as $prefix => $paths) {
        if ($paths = $keep($paths)) {
            $loader->setPsr4($prefix, $paths);
        }
    }
    $classMap = array_filter(require $composerDir.'/autoload_classmap.php', static fn ($file) => !$isExcluded($file));
    $loader->addClassMap($classMap);
    $loader->register(true);

    $versions = require $composerDir.'/installed.php';
    $versions['versions'] = array_diff_key($versions['versions'], $devNames);
    Composer\InstalledVersions::reload($versions);

    foreach (require $composerDir.'/autoload_files.php' as $identifier => $file) {
        if (!$isExcluded($file) && empty($GLOBALS['__composer_autoload_files'][$identifier])) {
            $GLOBALS['__composer_autoload_files'][$identifier] = true;
            require $file;
        }
    }

    return $loader;
}
