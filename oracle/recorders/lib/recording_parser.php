<?php
// The Parser the upstream tests see under yaml_test_calls.php: the real
// upstream Parser (renamed RecordedParser there) with its constructor and
// public entry points recorded through CallRecorder.

namespace Symfony\Component\Yaml;

class Parser extends RecordedParser
{
    private array $limits;

    public function __construct(int $maxNestingLevel = self::DEFAULT_MAX_NESTING_LEVEL, int $maxAliasesForCollections = self::DEFAULT_MAX_ALIASES_FOR_COLLECTIONS)
    {
        $this->limits = [$maxNestingLevel, $maxAliasesForCollections];
        try {
            parent::__construct($maxNestingLevel, $maxAliasesForCollections);
        } catch (\Throwable $e) {
            \CallRecorder::record(['method' => 'new Parser', 'limits' => $this->limits], static fn () => throw $e);
        }
    }

    public function parse(string $value, int $flags = 0): mixed
    {
        return \CallRecorder::record(
            ['method' => 'parse', 'limits' => $this->limits, 'input' => $value, 'flags' => $flags],
            fn () => parent::parse($value, $flags),
        );
    }

    public function parseFile(string $filename, int $flags = 0): mixed
    {
        $entry = ['method' => 'parseFile', 'limits' => $this->limits, 'input' => $filename, 'flags' => $flags];
        // a file the test wrote (not a fixture): its contents go along, and
        // a stable name replaces tempnam()'s random one in the recording
        if (is_file($filename) && !str_starts_with(\portable($filename), '{')) {
            $entry['contents'] = typed_value(file_get_contents($filename));
            \CallRecorder::$renames[$filename] ??= \dirname($filename) . '/yaml_' . (\count(\CallRecorder::$renames) + 1);
        }

        return \CallRecorder::record($entry, fn () => parent::parseFile($filename, $flags));
    }
}
