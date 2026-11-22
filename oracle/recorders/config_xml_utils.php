<?php
// What Symfony\Component\Config\Util\XmlUtils answers, for
// crates/config/tests/data/xml_utils.json (scripts/record.sh config-xml-utils).
//
// Recorded beyond XmlUtilsTest:
// - phpize() over a value corpus (upstream's provider, edge cases of every
//   branch, and generated strings over the characters the branches test);
// - convertDomElementToArray() over documents, with and without the prefix
//   check, including the node kinds the tests leave out (CDATA, processing
//   instructions, entity references, several text nodes);
// - parse() and loadFile() outcomes: the exception class, message, code and
//   previous exception, for malformed documents (the error-list format),
//   document types, schema files (valid, invalid, malformed, imports by
//   relative, absolute and file: locations, from two working directories)
//   and callables (their results tested for truth, exceptions, libxml errors
//   they leave behind, the internal-errors flag they receive).
//
// Paths: files are written under $base; cases run in $cwd unless they name
// another directory. The replay substitutes its own directories.

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\Config\Util\XmlUtils;

$fixtures = '/work/upstream/symfony/config/Tests/Fixtures/Util/';
$base = '/tmp/xml-utils';
@mkdir($base);
@mkdir("$base/imp");
@mkdir("$base/dir");
chdir($base);

$files = [];
$put = function (string $name, string $content) use ($base, &$files) {
    file_put_contents("$base/$name", $content);
    $files[$name] = base64_encode($content);
};

foreach (['document_type.xml', 'invalid.xml', 'invalid_schema.xml', 'schema.xsd', 'valid.xml'] as $f) {
    $put($f, file_get_contents($fixtures.$f));
}
$put('empty.xml', '');
$put('ws.xml', " \n\t\r\0\x0B");
$put('ws2.xml', " \x0C ");
$put('malformed.xml', "<?xml version=\"1.0\"?>\n<a xmlns=\"rel\">\n  <b>\n</a>\n");
$put('warn_only.xml', '<a xmlns="rel"/>');
$put('two_roots.xml', "<a/>\n<b/>");
$put('bad_schema.xsd', '<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema"><xsd:element name="root"');
$put('not_schema.xsd', '<root/>');
$put('dup_schema.xsd', '<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema"><xsd:element name="root"/><xsd:element name="root"/></xsd:schema>');
$part = '<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" targetNamespace="urn:part"><xsd:element name="p" type="xsd:positiveInteger"/></xsd:schema>';
$put('imp/part.xsd', $part);
$put('imp/a b.xsd', $part);
$main = fn (string $location) => '<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:p="urn:part" targetNamespace="urn:main" elementFormDefault="qualified">'
    .'<xsd:import namespace="urn:part" schemaLocation="'.htmlspecialchars($location).'"/>'
    .'<xsd:element name="root"><xsd:complexType><xsd:sequence><xsd:element ref="p:p"/></xsd:sequence></xsd:complexType></xsd:element></xsd:schema>';
$put('imp/rel.xsd', $main('part.xsd'));
$put('imp/rel_dir.xsd', $main('imp/part.xsd'));
$put('imp/abs.xsd', $main("$base/imp/part.xsd"));
$put('imp/url.xsd', $main("file://$base/imp/part.xsd"));
$put('imp/space.xsd', $main('imp/a b.xsd'));
$put('imp/escaped.xsd', $main('imp/a%20b.xsd'));
$put('imp/missing.xsd', $main('imp/none.xsd'));
$put('main_valid.xml', '<root xmlns="urn:main"><p xmlns="urn:part">12</p></root>');
$put('main_invalid.xml', '<root xmlns="urn:main"><p xmlns="urn:part">x</p></root>');

function outcome(callable $f): array
{
    try {
        $r = $f();

        return ['ok', $r instanceof \DOMDocument ? $r->documentElement?->nodeName : typed_value($r)];
    } catch (\Throwable $e) {
        $p = $e->getPrevious();

        return ['throws', $e::class, $e->getMessage(), $e->getCode(), $p ? [$p::class, $p->getMessage()] : null];
    }
}

/** The callables the cases pass, by name. */
function callable_named(string $name, array &$seen): callable
{
    return match ($name) {
        'false' => fn () => false,
        'true' => fn () => true,
        'null' => fn () => null,
        'zero' => fn () => 0,
        'zero-string' => fn () => '0',
        'one' => fn () => 1,
        'empty-array' => fn () => [],
        'array' => fn () => [0],
        'string' => fn () => 'yes',
        'exception' => fn () => throw new \RuntimeException('boom', 7),
        'logic-exception' => fn () => throw new \LogicException('logic'),
        'error' => fn () => throw new \Error('bad'),
        'schema-false' => fn (\DOMDocument $d) => @$d->schemaValidateSource(file_get_contents('/tmp/xml-utils/schema.xsd')),
        'schema-true' => function (\DOMDocument $d) {
            @$d->schemaValidateSource(file_get_contents('/tmp/xml-utils/schema.xsd'));

            return true;
        },
        'schema-throw' => function (\DOMDocument $d) {
            @$d->schemaValidateSource(file_get_contents('/tmp/xml-utils/schema.xsd'));

            throw new \RuntimeException('after errors');
        },
        'args' => function (\DOMDocument $d, $internal) use (&$seen) {
            $seen[] = [\func_num_args(), $internal];

            return true;
        },
    };
}

// phpize ------------------------------------------------------------------

$values = ['', 'null', 'true', 'false', 'Null', 'True', 'False', '0', '1', '-1', '0777', '-0777', '0877', '0xFF', '1e2', '-1.2E2',
    '-10100.1', '-10,100.1', '1234 5678 9101 1121 3141', '1,2,3,4', '11,22,33,44', '11,222,333,4', '1,222,333,444',
    '11,222,333,444', '111,222,333,444', '1111,2222,3333,4444,5555', 'foo', '0b0110',
    'NULL', 'nULL', 'TRUE', 'fAlSe', 'nul', 'true ', ' true', "null\n",
    '00', '000', '-0', '-00', '-', '+1', '+0', '+0777', '-+1', '--1', '0778', '08', '09', '010', '-010', '0001',
    '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809', '99999999999999999999',
    '0777777777777777777777', '01777777777777777777777', '0777777777777777777770', '01000000000000000000000',
    '-0777777777777777777777', '-01000000000000000000000', '007', '0123456789',
    '0x', '0X', '0xg', '0xFFg', '0x0', '0x7FFFFFFFFFFFFFFF', '0x8000000000000000', '0xFFFFFFFFFFFFFFFFFF', '0XFF', '0xff',
    "0xFF\n", "0xFF\n\n", '0x FF', '-0xFF', '+0xFF', ' 0xFF',
    '0b', '0B01', '0b2', '0b012', "0b01\n", "0b01\n\n", '-0b1', '0b'.str_repeat('1', 63), '0b'.str_repeat('1', 64),
    '0b1'.str_repeat('0', 64), '0b'.str_repeat('1', 70),
    '1.0', '1.', '.5', '-.5', '+.5', '5.', '1e', 'e1', '1e+2', '1e-2', '1E5', '1e999', '-1e999', '1e-400', '0.1', '-0.0',
    ' 1', '1 ', "1\n", "\t1.5\n", " 1.5 \n", "1.5\n", "12\n", "0777\n", "-1\n", "\n1", "1\n\n", "\x0B1", "1\x0B", "1\x0C",
    "1\r", "1\0", '1_000', '0x1p3', 'INF', 'NAN', 'inf', '-inf', '١', '１', '1,5', '1.2.3', '1e2.5', '..1', '1..',
    '123456789012345678901234567890', '0.000001', '1e308', '2.2250738585072014E-308', '4.9e-324', '123456789012345678.5',
    '-123.456', '+123.456', '00.5', '-00.5', '0e0', '0e', '0x1e2', '1e0x', ' ', "\n", 'x', 'yes', 'no', 'on', 'off',
];
mt_srand(20261122);
$alphabet = ['0', '1', '7', '8', '9', '-', '+', '.', 'e', 'E', 'x', 'X', 'b', 'B', 'f', ' ', "\n", 'a', 'n', 'u', 'l'];
while (\count($values) < 4000) {
    $s = '';
    $n = mt_rand(1, 6);
    for ($i = 0; $i < $n; ++$i) {
        $s .= $alphabet[mt_rand(0, \count($alphabet) - 1)];
    }
    $values[] = $s;
}
$values = array_values(array_unique($values));
$phpize = [];
foreach ($values as $v) {
    $phpize[] = [base64_encode($v), typed_call(fn () => XmlUtils::phpize($v))];
}

// convertDomElementToArray -----------------------------------------------

$convertCases = [
    // XmlUtilsTest::getDataForConvertDomToArray()
    ['', false], ['bar', false], ['<foo bar="foobar" />', true], ['<foo />', false], ['<foo>bar</foo>', false],
    ['<foo foo="bar"/>', false], ['<foo><foo>0</foo></foo>', false], ['<foo><foo>bar</foo></foo>', false],
    ['<foo foo="bar">text</foo>', false], ['<foo attr="bar"><foo>text</foo></foo>', false],
    ['<foo>bar</foo><foo>text</foo>', false], ['<foo foo="bar"/><foo foo="text" />', false],
    ['<foo foo="bar"><foo>text</foo></foo>', false], ['<foo><!-- Comment -->bar</foo>', false],
    ['<foo xmlns:h="http://www.example.org/bar" h:bar="bar">text</foo>', false],
    ['<foo:a xmlns:foo="http://www.example.org/foo" xmlns:h="http://www.example.org/bar" attr="1" h:bar="bar"><foo:b>hello</foo:b><h:c>2</h:c></foo:a>', true],
    // more
    ['<a><![CDATA[ cdata ]]></a>', true], ['<a>x<![CDATA[y]]></a>', true], ['<a>one<b/>two</a>', true], ['<a>one<b/>  </a>', true],
    ['<a> <b/> </a>', true], ['<a>  </a>', true], ['<a><!-- c --></a>', true], ['<a><?pi data?></a>', true], ['<a>x<?pi data?></a>', true],
    ['<!DOCTYPE a [<!ENTITY e "v">]><a>&e;</a>', true], ['<!DOCTYPE a [<!ENTITY e "v">]><a x="&e;"/>', true],
    ['<a>a&amp;b</a>', true], ['<a>&#65;&#x42;</a>', true],
    ['<a><b/><b/></a>', true], ['<a><b/><b>1</b></a>', true], ['<a><b>1</b><b/></a>', true], ['<a><b>1</b><b>2</b><b>3</b></a>', true],
    ['<a><b><c>1</c></b><b>2</b></a>', true], ['<a><b><c>1</c></b><b><c>2</c></b><b>3</b></a>', true],
    ['<a><b x="1"/><b>2</b></a>', true], ['<a b="1"><b>2</b><b>3</b></a>', true], ['<a b=""><b/></a>', true],
    ['<a value="x">t</a>', true], ['<a><value>v</value>t</a>', true], ['<a value="x"/>', true], ['<a><value>1</value><value>2</value>t</a>', true],
    ['<a x="null" y="true" z="0x1F" w="010" v="1e3" u="-5" t="0b11" s=" 7 "/>', true], ['<a>  null  </a>', true], ['<a> 12 </a>', true],
    ['<a xmlns="urn:d"><b>1</b></a>', true], ['<a xmlns="urn:d"><b xmlns="">1</b></a>', true],
    ['<p:a xmlns:p="urn:p"><b>1</b><p:c>2</p:c></p:a>', true], ['<p:a xmlns:p="urn:p"><b>1</b><p:c>2</p:c></p:a>', true, false],
    ['<p:a xmlns:p="urn:p"><!-- c -->t</p:a>', true], ['<p:a xmlns:p="urn:p"><?pi d?>t</p:a>', true], ['<p:a xmlns:p="urn:p"><?pi d?>t</p:a>', true, false],
    ['<p:a xmlns:p="urn:p" xmlns:q="urn:q" p:x="1" q:x="2" x="3"/>', true], ['<p:a xmlns:p="urn:p" xmlns:q="urn:q" p:x="1" q:x="2" x="3"/>', true, false],
    ['<a xmlns:q="urn:q" q:x="2" x="3"/>', true, false], ['<a xmlns:q="urn:q" x="3" q:x="2"/>', true, false],
    ['<a xml:lang="en" lang="fr"/>', true], ['<a xml:lang="en" lang="fr"/>', true, false],
    ['<q:a xmlns:q="urn:q"><q:b q:k="1" k="2">t</q:b></q:a>', true], ['<a xmlns:q="urn:q"><q:b>1</q:b><b>2</b></a>', true],
    ['<a xmlns:q="urn:q"><q:b>1</q:b><b>2</b></a>', true, false],
    ['<a><b c="1"><d>2</d></b><b>x</b><b><d>3</d></b></a>', true], ['<a><x><y><z>deep</z></y></x></a>', true],
    ['<a b="1" c="2" d="3"><e>4</e>t</a>', true], ['<a>first<b/>second<c/>third</a>', true],
    ['<a><b>1</b><c>2</c><b>3</b></a>', true], ['<a><b123>1</b123></a>', true], ['<a b-c="1" b.d="2"/>', true],
    ["<a>\n  <b>1</b>\n  <b>2</b>\n</a>", true], ['<a>é ü</a>', true], ['<a b="é"/>', true],
];
$convert = [];
foreach ($convertCases as $case) {
    [$xml, $root] = $case;
    $check = $case[2] ?? true;
    $dom = new \DOMDocument();
    $source = $root ? $xml : '<root>'.$xml.'</root>';
    $dom->loadXML($source);
    $r = typed_call(fn () => XmlUtils::convertDomElementToArray($dom->documentElement, $check));
    if ('throws' === $r[0]) {
        // drop ", called in <file> on line <n>"
        $r[2] = preg_replace('/, called in .*$/s', '', $r[2]);
    }
    $convert[] = [base64_encode($source), $check, $r];
}

// parse / loadFile -------------------------------------------------------

$parseContents = [
    '<a/>', "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<a/>\n", "\xEF\xBB\xBF<a/>", '<a>', '<a></b>', "<a b='1' b='2'/>",
    '<a xmlns="rel"/>', "<a xmlns=\"rel\">\n<b>\n</a>", 'text', '   ', "\n", '<?xml version="1.0"?>', '<a/><b/>', '<a>&undefined;</a>',
    "<a>\x80</a>", '<!-- only -->', '<!DOCTYPE a><a/>', '<!DOCTYPE a [<!ELEMENT a EMPTY>]><a/>', "<!DOCTYPE a [\n<!ENTITY e \"v\">\n]>\n<a>&e;</a>",
    "<a>\n<b c=\"1\" c=\"2\">\n</b>\n<d>\n</a>", '<a b=c/>', '<1a/>', '<a:b/>', '<a xmlns:p="urn:p"><p:b/><q:c/></a>', '<a>]]></a>',
    '<?xml version="1.1"?><a/>', '<?xml version="2.0"?><a/>', '<?xml version="1.0" encoding="ISO-8859-1"?><a>'."\xE9".'</a>',
    '', "\0", '<a/>'."\0",
];
$parse = [];
foreach ($parseContents as $c) {
    $parse[] = [base64_encode($c), null, outcome(fn () => XmlUtils::parse($c))];
}
$schemaCases = [
    ['valid.xml', 'schema.xsd'], ['invalid_schema.xml', 'schema.xsd'], ['valid.xml', 'bad_schema.xsd'], ['valid.xml', 'not_schema.xsd'],
    ['valid.xml', 'dup_schema.xsd'], ['valid.xml', 'none.xsd'], ['valid.xml', 'dir'], ['valid.xml', 'invalid_callback_or_file'],
    ['valid.xml', ''], ['valid.xml', "$base/schema.xsd"],
    ['main_valid.xml', 'imp/rel.xsd'], ['main_valid.xml', 'imp/rel_dir.xsd'], ['main_valid.xml', 'imp/abs.xsd'],
    ['main_valid.xml', 'imp/url.xsd'], ['main_valid.xml', 'imp/space.xsd'], ['main_valid.xml', 'imp/escaped.xsd'],
    ['main_invalid.xml', 'imp/rel_dir.xsd'], ['main_invalid.xml', 'imp/abs.xsd'],
];
$seen = [];
foreach (['.', 'imp'] as $dir) {
    chdir("$base/$dir");
    foreach ($schemaCases as [$doc, $schema]) {
        $c = file_get_contents("$base/$doc");
        $parse[] = [base64_encode($c), ['schema', $schema, $dir], outcome(fn () => XmlUtils::parse($c, $schema))];
    }
    chdir($base);
}
foreach (['false', 'true', 'null', 'zero', 'zero-string', 'one', 'empty-array', 'array', 'string', 'exception', 'logic-exception',
    'error', 'schema-false', 'schema-true', 'schema-throw'] as $name) {
    foreach (['valid.xml', 'invalid_schema.xml'] as $doc) {
        $c = file_get_contents("$base/$doc");
        $parse[] = [base64_encode($c), ['callable', $name], outcome(fn () => XmlUtils::parse($c, callable_named($name, $seen)))];
    }
}
// the flag a callable receives: the caller's libxml_use_internal_errors()
$flags = [];
foreach ([false, true] as $setting) {
    $seen = [];
    $before = libxml_use_internal_errors($setting);
    XmlUtils::parse('<a/>', callable_named('args', $seen));
    $after = libxml_use_internal_errors($before);
    $flags[] = [$setting, $seen[0][0], $seen[0][1], $after];
}
// parse('') leaves the flag set: loadXML() throws before it is restored
libxml_use_internal_errors(false);
$emptyResult = outcome(fn () => XmlUtils::parse(''));
$leak = libxml_use_internal_errors(false);

$loadCases = [
    ['dir', null], ['missing.xml', null], ['empty.xml', null], ['ws.xml', null], ['ws2.xml', null], ['invalid.xml', null],
    ['malformed.xml', null], ['warn_only.xml', null], ['two_roots.xml', null], ['document_type.xml', null], ['valid.xml', null],
    ['invalid_schema.xml', ['schema', 'schema.xsd']], ['invalid_schema.xml', ['schema', 'invalid_callback_or_file']],
    ['valid.xml', ['schema', 'schema.xsd']], ['valid.xml', ['callable', 'false']], ['valid.xml', ['callable', 'true']],
    ['valid.xml', ['callable', 'exception']], ['valid.xml', ['callable', 'schema-false']], ['invalid_schema.xml', ['callable', 'schema-false']],
    ['invalid_schema.xml', ['callable', 'schema-throw']], ['main_invalid.xml', ['schema', 'imp/abs.xsd']],
];
$load = [];
foreach ($loadCases as [$file, $v]) {
    $arg = null === $v ? null : ('schema' === $v[0] ? $v[1] : callable_named($v[1], $seen));
    $load[] = ["$base/$file", $v, outcome(fn () => XmlUtils::loadFile("$base/$file", $arg))];
}
// relative file names resolve against the working directory
$load[] = ['./valid.xml', null, outcome(fn () => XmlUtils::loadFile('./valid.xml'))];
$load[] = ['invalid.xml', null, outcome(fn () => XmlUtils::loadFile('invalid.xml'))];

echo json_encode([
    'cwd' => $base,
    'base' => $base,
    'libxml' => LIBXML_DOTTED_VERSION,
    'files' => $files,
    'phpize' => $phpize,
    'convert' => $convert,
    'parse' => $parse,
    'flags' => $flags,
    'empty' => [$emptyResult, $leak],
    'load' => $load,
], \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR | \JSON_INVALID_UTF8_SUBSTITUTE), "\n";
