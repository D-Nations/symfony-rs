<?php

namespace Fixture\Controller;

use Fixture\Service\Greeter;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\Routing\Attribute\Route;

final class GreetController
{
    public function __construct(private readonly Greeter $greeter)
    {
    }

    #[Route('/greet/{name}', name: 'greet', requirements: ['name' => '[a-z]+'], methods: ['GET'])]
    public function greet(string $name, Request $request): Response
    {
        $greeting = $this->greeter->greet($name, $request->query->getString('punctuation', '!'));

        return new Response($greeting->render(), 200, ['Content-Type' => 'text/plain']);
    }
}
