<?php

namespace Fixture;

use Symfony\Bundle\FrameworkBundle\Kernel\MicroKernelTrait;
use Symfony\Component\HttpKernel\Kernel as BaseKernel;

final class Kernel extends BaseKernel
{
    use MicroKernelTrait;

    // No composer.json to find the project by.
    public function getProjectDir(): string
    {
        return \dirname(__DIR__);
    }

    // The app directory is mounted read-only.
    public function getCacheDir(): string
    {
        return sys_get_temp_dir().'/converter-fixture/cache/'.$this->environment;
    }

    public function getLogDir(): string
    {
        return sys_get_temp_dir().'/converter-fixture/log';
    }
}
