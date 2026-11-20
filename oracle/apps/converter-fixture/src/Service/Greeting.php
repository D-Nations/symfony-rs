<?php

namespace Fixture\Service;

final readonly class Greeting
{
    /**
     * @param list<string> $tags
     */
    public function __construct(
        public string $text,
        public int $length,
        public float $ratio,
        public array $tags,
        public int $visits,
    ) {
    }

    public function render(): string
    {
        return implode("\n", [
            'text: '.$this->text,
            'length: '.$this->length,
            'ratio: '.$this->ratio,
            'tags: '.implode(', ', $this->tags),
            'visits: '.$this->visits,
        ])."\n";
    }
}
