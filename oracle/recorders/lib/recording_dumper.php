<?php
// The Dumper the upstream tests see under yaml_test_calls.php: the real
// upstream Dumper (renamed RecordedDumper there) with its constructor and
// dump() recorded through CallRecorder.

namespace Symfony\Component\Yaml;

class Dumper extends RecordedDumper
{
    private int $recordedIndentation;

    public function __construct(int $indentation = 4)
    {
        $this->recordedIndentation = $indentation;
        try {
            parent::__construct($indentation);
        } catch (\Throwable $e) {
            \CallRecorder::record(['method' => 'new Dumper', 'indentation' => $indentation], static fn () => throw $e);
        }
    }

    public function dump(mixed $input, int $inline = 0, int $indent = 0, int $flags = 0): string
    {
        return \CallRecorder::record(
            ['method' => 'dump', 'indentation' => $this->recordedIndentation, 'input' => $input, 'inline' => $inline, 'indent' => $indent, 'flags' => $flags],
            fn () => parent::dump($input, $inline, $indent, $flags),
        );
    }
}
