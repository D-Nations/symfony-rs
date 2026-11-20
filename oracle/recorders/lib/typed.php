<?php
// Encodes PHP values as typed JSON for the Rust vector readers
// (php_value::vectors). Floats as IEEE 754 bits, strings as base64.

function typed_value(mixed $v): array
{
    return match (true) {
        null === $v => ['null'],
        \is_bool($v) => ['bool', $v],
        \is_int($v) => ['int', $v],
        \is_float($v) => ['float', bin2hex(pack('E', $v))],
        \is_string($v) => ['string', base64_encode($v)],
        \is_array($v) => ['array', typed_pairs($v)],
        $v instanceof \UnitEnum => ['enum', $v::class, $v->name, $v instanceof \BackedEnum ? typed_value($v->value) : null],
        $v instanceof \DateTimeInterface => ['datetime', $v->format('Y-m-d\TH:i:s.uP'), $v->getTimezone()->getName(), $v->format('U')],
        $v instanceof \ArrayObject => ['arrayobject', typed_pairs($v->getArrayCopy())],
        $v instanceof \Symfony\Component\Yaml\Tag\TaggedValue => ['tagged', $v->getTag(), typed_value($v->getValue())],
        // symfony/string's objects keep their text in a protected property
        $v instanceof \Symfony\Component\String\AbstractString => ['object', $v::class, [[typed_value('string'), typed_value((string) $v)]]],
        \is_object($v) => ['object', $v::class, typed_pairs(get_object_vars($v))],
        default => ['unsupported', get_debug_type($v)],
    };
}

function typed_pairs(array $a): array
{
    $pairs = [];
    foreach ($a as $k => $v) {
        $pairs[] = [typed_value($k), typed_value($v)];
    }

    return $pairs;
}

/// The result of $f, or ["throws", class, message].
function typed_call(callable $f): array
{
    try {
        return typed_value($f());
    } catch (\Throwable $e) {
        return ['throws', $e::class, $e->getMessage()];
    }
}
