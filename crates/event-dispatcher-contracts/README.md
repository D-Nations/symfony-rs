# Symfony EventDispatcher Contracts

Private workspace crate: a port of `symfony/event-dispatcher-contracts`
v3.7.1 (`c7de7a00`). MIT; Symfony source copyright Fabien Potencier.
Unofficial, not affiliated with or endorsed by Symfony.

| Contract | Source | Evidence |
|---|---|---|
| `Event`: propagation flag, `stopPropagation()`, class `Symfony\Contracts\EventDispatcher\Event` | `Event.php` | Unit tests; used by every symfony-event-dispatcher test |
| `EventDispatcherInterface::dispatch(event, ?name)` | `EventDispatcherInterface.php` | symfony-event-dispatcher |
| PSR-14 `StoppableEventInterface`, folded into `EventObject::is_propagation_stopped()` (default `false`, which dispatches like an object without the interface) | psr/event-dispatcher | symfony-event-dispatcher oracle vectors |

The package has no PHPUnit tests upstream. `EventObject::class()` is the
PHP class name, the default event name.
