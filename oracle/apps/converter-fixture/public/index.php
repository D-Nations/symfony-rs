<?php

// Front controller of the converter fixture app (oracle only).
// No symfony/runtime: its autoload_runtime.php belongs to the demo's
// project directory, not to this one.

use Fixture\Kernel;
use Symfony\Component\HttpFoundation\Request;

$loader = require '/demo/vendor/autoload.php';
$loader->addPsr4('Fixture\\', dirname(__DIR__).'/src/');

$kernel = new Kernel('prod', false);
$request = Request::createFromGlobals();
$response = $kernel->handle($request);
$response->send();
$kernel->terminate($request, $response);
