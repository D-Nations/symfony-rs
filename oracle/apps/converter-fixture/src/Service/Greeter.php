<?php

namespace Fixture\Service;

use Symfony\Component\DependencyInjection\Attribute\Autowire;

final class Greeter
{
    /**
     * @param list<string> $tags
     */
    public function __construct(
        #[Autowire('%app.greeting%')] private readonly string $greeting,
        #[Autowire('%app.max_length%')] private readonly int $maxLength,
        #[Autowire('%app.tags%')] private readonly array $tags,
        private readonly VisitCounter $visits,
    ) {
    }

    public function greet(string $name, string $punctuation): Greeting
    {
        if (\strlen($name) > $this->maxLength) {
            throw new GreetingException(\sprintf('Name "%s" is longer than %d characters.', $name, $this->maxLength));
        }

        return new Greeting(
            $this->greeting.', '.ucfirst($name).$punctuation,
            \strlen($name),
            \strlen($name) / $this->maxLength,
            $this->tags,
            $this->visits->increment(),
        );
    }
}
