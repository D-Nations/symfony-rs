<?php
// EventDispatcher behaviour that the upstream tests leave open, printed one
// fact per line. scripts/record.sh event-dispatcher writes the output to
// crates/event-dispatcher/tests/data/edges.txt; each line is pinned by the
// Rust test named in its comment (crates/event-dispatcher/tests/event_dispatcher.rs).
// Each part runs on the base class and on a subclass, like the Rust tests.

require '/demo/vendor/autoload.php';

use Symfony\Component\EventDispatcher\EventDispatcher;
use Symfony\Contracts\EventDispatcher\Event;

class O { function preFoo($e) {} function __invoke($e) {} }
class Child extends EventDispatcher {}

function line(string $label, $value): void
{
    echo $label, ': ', is_string($value) ? $value : var_export($value, true), "\n";
}

// closures_of_the_same_method_are_equal, anonymous_closures_are_equal_only_to_themselves
$o = new O();
$p = new O();
$make = static fn () => static function () {};
line('closure == closure from one declaration', $make() == $make());
line('$o(...) == $o->__invoke(...)', $o(...) == $o->__invoke(...));
line('$o(...) == $o->__INVOKE(...)', $o(...) == $o->__INVOKE(...));
line('$o(...) == $p(...)', $o(...) == $p(...));
line('$o(...) == $o->preFoo(...)', $o(...) == $o->preFoo(...));

foreach ([EventDispatcher::class, Child::class] as $class) {
    echo "# $class\n";

    // anonymous_closures_are_equal_only_to_themselves
    $d = new $class();
    $d->addListener('x', static function () {});
    $d->removeListener('x', static function () {});
    line('kept after removing another closure', $d->hasListeners('x'));

    // events_are_listed_in_first_sorted_order
    $d = new $class();
    $d->addListener('pre', $o);
    $d->addListener('post', $o);
    $d->getListeners('post');
    line('events after sorting post first', implode(',', array_keys($d->getListeners())));
    $d->addListener('pre', $p);
    line('events after adding to pre', implode(',', array_keys($d->getListeners())));

    // priority_lookup_scans_buckets_in_insertion_order_until_sorted
    $d = new $class();
    $d->addListener('e', $o, -5);
    $d->addListener('e', $o, 7);
    line('priority before sorting', $d->getListenerPriority('e', $o));
    $d->getListeners('e');
    line('priority after sorting', $d->getListenerPriority('e', $o));
    $d->removeListener('e', $o);
    line('listeners after removing both', $d->hasListeners('e'));

    // the_default_event_name_is_the_event_class
    $d = new $class();
    $seen = null;
    $d->addListener(Event::class, static function ($e, $name) use (&$seen) { $seen = $name; });
    $d->dispatch(new Event());
    line('default event name', $seen);

    // an_event_without_propagation_control_reaches_every_listener
    $d = new $class();
    $n = 0;
    $d->addListener('s', static function () use (&$n) { ++$n; });
    $d->addListener('s', static function () use (&$n) { ++$n; });
    $d->dispatch(new stdClass(), 's');
    line('stdClass listeners called', $n);

    // a_listener_exception_ends_the_dispatch
    $d = new $class();
    $after = false;
    $d->addListener('t', static function () { throw new RuntimeException('boom'); }, 1);
    $d->addListener('t', static function () use (&$after) { $after = true; });
    try {
        $d->dispatch(new Event(), 't');
    } catch (RuntimeException $e) {
        line('thrown', $e->getMessage());
    }
    line('later listener called', $after);

    // an_undefined_method_is_an_error
    $d = new $class();
    $d->addListener('u', [$o, 'nope']);
    try {
        $d->dispatch(new Event(), 'u');
    } catch (Error $e) {
        line(get_class($e), $e->getMessage());
    }

    // a_dispatch_calls_the_listeners_of_its_start
    $d = new $class();
    $log = [];
    $d->addListener('r', static function ($e, $n, $disp) use (&$log) {
        $log[] = 'first';
        $disp->addListener('r', static function () use (&$log) { $log[] = 'added'; });
    });
    $d->addListener('r', static function () use (&$log) { $log[] = 'second'; });
    $d->dispatch(new Event(), 'r');
    line('added during dispatch', implode(',', $log));
    $log = [];
    $d->dispatch(new Event(), 'r');
    line('next dispatch', implode(',', $log));
    $log = [];
    $later = static function () use (&$log) { $log[] = 'later'; };
    $d->addListener('q', static function ($e, $n, $disp) use (&$log, $later) {
        $log[] = 'first';
        $disp->removeListener('q', $later);
    });
    $d->addListener('q', $later);
    $d->dispatch(new Event(), 'q');
    line('removed during dispatch', implode(',', $log));
}
