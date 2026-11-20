<?php

namespace Fixture\Service;

// A mutable shared service. Under PHP each request starts from a fresh
// container, so every response reports one visit.
final class VisitCounter
{
    private int $count = 0;

    public function increment(): int
    {
        return ++$this->count;
    }
}
