<?php

// Table renders on generated tables (scripts/record.sh console): headers,
// rows of strings, numbers, nulls, multi-line and styled text, TableCells
// with colspan, rowspan and cell styles, TableSeparators, the seven
// styles, the three orientations, column widths and maximum widths,
// header and footer titles, decorated or not. Each case: its input as
// typed JSON and the output, or the exception thrown. Deterministic: a
// fixed seed. crates/console/tests/table_vectors.rs replays them.
//
//   php console_table.php

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Console\Helper\Table;
use Symfony\Component\Console\Helper\TableCell;
use Symfony\Component\Console\Helper\TableCellStyle;
use Symfony\Component\Console\Helper\TableSeparator;
use Symfony\Component\Console\Output\StreamOutput;

mt_srand(20261122);
$pick = static fn (array $a) => $a[mt_rand(0, count($a) - 1)];

$texts = ['', 'a', 'foo', 'Divine Comedy', 'x y z', "two\nlines", "a\r\nb", "1\n2\n3", '<info>styled</info>', '<fg=red>red</>',
    'é', '表示', '🎭 x', 'back\\', '1234\\6', 'long text that will need wrapping somewhere', '<comment>c</comment> d', '0', ' pad '];
$cell = static function () use ($pick, $texts) {
    switch (mt_rand(0, 9)) {
        case 0: return null;
        case 1: return mt_rand(-5, 500);
        case 2: return new TableCell($pick($texts), ['colspan' => mt_rand(1, 3)]);
        case 3: return new TableCell($pick($texts), ['rowspan' => mt_rand(1, 3)]);
        case 4: return new TableCell($pick($texts), ['rowspan' => mt_rand(1, 2), 'colspan' => mt_rand(1, 2)]);
        case 5: return new TableCell($pick($texts), ['style' => new TableCellStyle(['align' => $pick(['left', 'center', 'right']), 'fg' => $pick(['red', 'default']), 'bg' => $pick(['default', 'blue'])])]);
        case 6: return new TableCell($pick($texts), ['style' => new TableCellStyle(['cellFormat' => '<info>%s</info>', 'align' => 'right'])]);
        default: return $pick($texts);
    }
};
$row = static function (int $width) use ($cell) {
    $r = [];
    for ($i = 0; $i < $width; ++$i) {
        $r[] = $cell();
    }

    return $r;
};

$cases = [];
for ($n = 0; $n < 1500; ++$n) {
    $columns = mt_rand(1, 4);
    $headers = [];
    if (mt_rand(0, 3)) {
        $headers = mt_rand(0, 4) ? array_map(static fn () => $pick(['ISBN', 'Title', 'Author', 'H', '表', '<info>i</info>']), range(1, mt_rand(1, $columns)))
            : [$row(mt_rand(1, $columns)), $row(mt_rand(1, $columns))];
    }
    $rows = [];
    for ($i = 0, $count = mt_rand(0, 5); $i < $count; ++$i) {
        $rows[] = mt_rand(0, 7) ? $row(mt_rand(1, $columns)) : new TableSeparator();
    }
    $case = [
        'headers' => $headers,
        'rows' => $rows,
        'style' => $pick(['default', 'default', 'markdown', 'borderless', 'compact', 'symfony-style-guide', 'box', 'box-double']),
        'orientation' => $pick(['default', 'default', 'default', 'horizontal', 'vertical']),
        'widths' => mt_rand(0, 3) ? [] : [mt_rand(0, $columns - 1) => mt_rand(0, 25)],
        'max_widths' => mt_rand(0, 3) ? [] : [mt_rand(0, $columns - 1) => mt_rand(1, 12)],
        'header_title' => mt_rand(0, 4) ? null : $pick(['Title', 'A much longer title than the table is wide', '0']),
        'footer_title' => mt_rand(0, 4) ? null : $pick(['Page 1/2', 'Footer']),
        'decorated' => 0 === mt_rand(0, 3),
    ];

    $stream = fopen('php://memory', 'r+');
    $output = new StreamOutput($stream, StreamOutput::VERBOSITY_NORMAL, $case['decorated']);
    $table = new Table($output);
    try {
        $table->setHeaders($case['headers'])->setRows($case['rows'])->setStyle($case['style']);
        if ('horizontal' === $case['orientation']) {
            $table->setHorizontal();
        } elseif ('vertical' === $case['orientation']) {
            $table->setVertical();
        }
        foreach ($case['widths'] as $c => $w) {
            $table->setColumnWidth($c, $w);
        }
        foreach ($case['max_widths'] as $c => $w) {
            $table->setColumnMaxWidth($c, $w);
        }
        $table->setHeaderTitle($case['header_title'])->setFooterTitle($case['footer_title']);
        $table->render();
        rewind($stream);
        $result = ['output', base64_encode(stream_get_contents($stream))];
    } catch (\Throwable $e) {
        $result = ['throws', $e::class, $e->getMessage()];
    }
    $cases[] = ['case' => typed_value($case), 'result' => $result];
}

echo json_encode($cases, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
