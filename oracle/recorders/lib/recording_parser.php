<?php
// The Parser the upstream tests see under yaml_test_calls.php: the real
// upstream Parser (renamed RecordedParser there) with its public entry
// points recorded through CallRecorder.

namespace Symfony\Component\Yaml;

class Parser extends RecordedParser
{
    private array $limits;

    public function __construct(int $maxNestingLevel = self::DEFAULT_MAX_NESTING_LEVEL, int $maxAliasesForCollections = self::DEFAULT_MAX_ALIASES_FOR_COLLECTIONS)
    {
        $this->limits = [$maxNestingLevel, $maxAliasesForCollections];
        parent::__construct($maxNestingLevel, $maxAliasesForCollections);
    }

    public function parse(string $value, int $flags = 0): mixed
    {
        return \CallRecorder::record($this->limits, 'parse', $value, $flags, fn () => parent::parse($value, $flags));
    }

    public function parseFile(string $filename, int $flags = 0): mixed
    {
        return \CallRecorder::record($this->limits, 'parseFile', $filename, $flags, fn () => parent::parseFile($filename, $flags));
    }
}
