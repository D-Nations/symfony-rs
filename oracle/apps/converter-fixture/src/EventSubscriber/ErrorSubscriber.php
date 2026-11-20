<?php

namespace Fixture\EventSubscriber;

use Fixture\Service\GreetingException;
use Symfony\Component\EventDispatcher\EventSubscriberInterface;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Event\ExceptionEvent;
use Symfony\Component\HttpKernel\Exception\HttpExceptionInterface;
use Symfony\Component\HttpKernel\KernelEvents;

final class ErrorSubscriber implements EventSubscriberInterface
{
    public static function getSubscribedEvents(): array
    {
        return [KernelEvents::EXCEPTION => 'onException'];
    }

    public function onException(ExceptionEvent $event): void
    {
        $throwable = $event->getThrowable();
        [$status, $headers] = match (true) {
            $throwable instanceof GreetingException => [422, []],
            $throwable instanceof HttpExceptionInterface => [$throwable->getStatusCode(), $throwable->getHeaders()],
            default => [null, []],
        };
        if (null === $status) {
            return;
        }

        $event->setResponse(new Response(
            \sprintf("error %d: %s\n", $status, $throwable->getMessage()),
            $status,
            ['Content-Type' => 'text/plain'] + $headers,
        ));
    }
}
