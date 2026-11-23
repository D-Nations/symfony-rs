<?php
// What Symfony\Component\Translation\Loader\XliffFileLoader and
// Util\XliffUtils answer, for crates/translation/tests/data/xliff.json
// (scripts/record.sh translation-xliff).
//
// Recorded beyond XliffFileLoaderTest and XliffUtilsTest:
// - load() of every upstream XLIFF fixture and of written documents, as
//   files and as strings: the catalogue (messages by domain, metadata,
//   catalogue metadata, resources) or the exception (class, message, code,
//   previous exceptions). The written documents cover what SimpleXML does
//   in the loader: which children and attributes it sees (prefixes,
//   namespaces), its string casts (text and CDATA of direct children
//   only), truth of attributes, numeric keys, the needs-translation rule,
//   notes, XLIFF 2 segments, metadata and pgs units, and the document
//   encodings php-dom reads with what mb_convert_encoding() makes of them;
// - resources that are not XML: wrappers, missing files, directories;
// - getFileUrl() over paths, getVersionNumber() over documents.
//
// Paths: files are written under $base and the cases run there. The replay
// substitutes its own directory.

require '/demo/vendor/autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\Translation\Loader\XliffFileLoader;
use Symfony\Component\Translation\MessageCatalogue;
use Symfony\Component\Translation\Util\XliffUtils;

$base = '/tmp/xliff';
@mkdir($base);
@mkdir("$base/dir");
@mkdir("$base/sub");
@mkdir("$base/foo:");
chdir($base);

$files = [];
$put = function (string $name, string $content) use ($base, &$files) {
    file_put_contents("$base/$name", $content);
    $files[$name] = base64_encode($content);
};

$fixtures = '/work/upstream/symfony/translation/Tests/Fixtures/';
$fixtureNames = [];
foreach (glob($fixtures.'*') as $f) {
    if (is_file($f) && preg_match('/\.(xlf|xliff|php)$/', $f)) {
        $put(basename($f), file_get_contents($f));
        $fixtureNames[] = basename($f);
    }
}
$put('sub/rel.xlf', file_get_contents($fixtures.'resources.xlf'));
$put('foo:/bar', file_get_contents($fixtures.'resources.xlf'));

/** A throwable and its previous ones: [class, message, code] each. */
function chain(\Throwable $e): array
{
    $out = [];
    for (; $e; $e = $e->getPrevious()) {
        $out[] = [$e::class, base64_encode(preg_replace('/, called in .*$/s', '', $e->getMessage())), $e->getCode()];
    }

    return $out;
}

function dump_catalogue(MessageCatalogue $c): array
{
    $get = fn (string $p) => (new \ReflectionProperty(MessageCatalogue::class, $p))->getValue($c);

    return [
        'locale' => $c->getLocale(),
        'messages' => typed_value($get('messages')),
        'metadata' => typed_value($get('metadata')),
        'catalogue_metadata' => typed_value($get('catalogueMetadata')),
        'resources' => array_map(fn ($r) => (string) $r, $c->getResources()),
    ];
}

function load_case(string $resource, string $locale, string $domain): array
{
    $before = libxml_use_internal_errors();
    try {
        $c = (new XliffFileLoader())->load($resource, $locale, $domain);
        $out = ['ok', dump_catalogue($c)];
    } catch (\Throwable $e) {
        $out = ['throws', chain($e)];
    }
    $after = libxml_use_internal_errors();
    $out[] = [$before, $after, \count(libxml_get_errors())];
    libxml_clear_errors();

    return $out;
}

$cases = [];
$file = function (string $name, string $locale = 'en', string $domain = 'domain1') use (&$cases) {
    $cases[] = ['file', $name, $locale, $domain, load_case($name, $locale, $domain)];
};
$string = function (string $xml, string $locale = 'en', string $domain = 'domain1') use (&$cases) {
    $cases[] = ['string', base64_encode($xml), $locale, $domain, load_case($xml, $locale, $domain)];
};
$absolute = function (string $name, string $locale = 'en', string $domain = 'domain1') use (&$cases, $base) {
    $cases[] = ['absolute', $name, $locale, $domain, load_case("$base/$name", $locale, $domain)];
};
$raw = function (string $resource) use (&$cases) {
    $cases[] = ['raw', base64_encode($resource), 'en', 'domain1', load_case($resource, 'en', 'domain1')];
};

// Upstream fixtures --------------------------------------------------------

foreach ($fixtureNames as $name) {
    $file($name);
    $absolute($name);
}
$file('resources.xlf', 'fr', 'messages');
$file('resources.xlf', 'en', 'messages+intl-icu');
$file('resources-2.2-pgs-plural.xlf', 'fr', 'messages+intl-icu');
$file('resources-2.0+intl-icu.xlf', 'en', 'messages');
$file('sub/rel.xlf');
$file('./resources.xlf');
$file('sub/../resources.xlf');

// Resources that are not XML strings ----------------------------------------

foreach (['http://example.com/resources.xlf', 'https://example.com/r.xlf', 'ftp://example.com/r.xlf', 'ftps://example.com/r.xlf',
    'HTTP://example.com/r.xlf', 'data://text/plain,abc', 'data:text/plain,abc', 'php://memory', 'php://stdin',
    'glob://*.xlf', 'compress.zlib://resources.xlf', 'zip://x', 'foo://bar', 'foo:/bar', "file://$base/resources.xlf",
    "FILE://$base/resources.xlf", 'file://resources.xlf', "file://localhost$base/resources.xlf", "file:$base/resources.xlf",
    "file://$base/sub/../resources.xlf", "phar://$base/none.phar/a.xlf", 'phar://none.phar', "1http://x", 'a_b://x', 'x://', "file://$base/dir", "file://$base/none.xlf",
    'non-existing.xlf', 'dir', 'dir/', "$base/dir", '', ' ', '.', '..', 'resources.xlf ', ' <?xml version="1.0"?><a/>',
    '<?XML version="1.0"?>', "\xEF\xBB\xBF<?xml version=\"1.0\"?><a/>", '<xliff/>', 'resources.php', 'empty.xlf',
] as $r) {
    $raw($r);
}

// XML strings ---------------------------------------------------------------

$x12 = fn (string $units, string $fileAttrs = 'source-language="en" datatype="plaintext" original="file.ext"', string $root = 'xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"', string $decl = '<?xml version="1.0" encoding="utf-8"?>')
    => $decl."\n<xliff $root>\n  <file $fileAttrs>\n    <body>\n$units\n    </body>\n  </file>\n</xliff>\n";
$x2 = fn (string $units, string $version = '2.0', string $extra = '', string $decl = '<?xml version="1.0" encoding="utf-8"?>')
    => $decl."\n<xliff xmlns=\"urn:oasis:names:tc:xliff:document:2.0\" $extra version=\"$version\" srcLang=\"en\" trgLang=\"fr\">\n  <file id=\"f1\">\n$units\n  </file>\n</xliff>\n";
$tu = fn (string $id, string $inner, string $attrs = '') => "<trans-unit id=\"$id\" $attrs>$inner</trans-unit>";

$units12 = [
    // the upstream raw case
    $tu('1', '<source>foo</source><target>bar</target>').$tu('2', '<source>extra</source>').$tu('3', '<source>key</source><target></target>')
        .$tu('4', '<source>test</source><target state="needs-translation">with</target><note>note</note>')
        .$tu('5', '<source>baz</source><target state="needs-translation">baz</target>')
        .$tu('6', '<source>baz</source><target state="needs-translation">buz</target>', 'resname="buz"'),
    // resname
    $tu('1', '<source>s</source><target>t</target>', 'resname=""'),
    $tu('1', '<source>s</source><target>t</target>', 'resname="0"'),
    $tu('1', '<source>s</source><target>t</target>', 'resname=" "'),
    $tu('1', '<source>s</source><target>t</target>', 'resname="12"').$tu('2', '<source>012</source><target>u</target>').$tu('3', '<source>-5</source><target>v</target>').$tu('4', '<source>9223372036854775808</source><target>w</target>'),
    $tu('1', '<source>1.5</source><target>a</target>').$tu('2', '<source>1</source><target>b</target>').$tu('3', '<source> 1</source><target>c</target>'),
    $tu('1', '<source></source><target>t</target>'),
    $tu('1', '<source/>'),
    // duplicates
    $tu('1', '<source>a</source><target>first</target><note>n1</note>').$tu('2', '<source>a</source><target>second</target>'),
    $tu('1', '<source>a</source><target>first</target>', 'resname="k"').$tu('2', '<source>k</source><target>second</target>'),
    // needs-translation
    $tu('1', '<source>s</source><target state="needs-translation">s</target>', 'resname="r"'),
    $tu('1', '<source>s</source><target state="needs-translation">r</target>', 'resname="r"'),
    $tu('1', '<source>s</source><target state="needs-translation"></target>'),
    $tu('1', '<source></source><target state="needs-translation"></target>'),
    $tu('1', '<source>s</source><target state="new">s</target>'),
    $tu('1', '<source>s</source><target state="needs-translation">s<g id="1">x</g></target>'),
    $tu('1', '<source>s<g id="1">x</g></source><target state="needs-translation">s</target>'),
    // text: inline elements, CDATA, entities, comments
    $tu('1', '<source>a<g id="1">b</g>c</source><target>d<g id="2">e</g>f<x id="3"/>g</target>'),
    $tu('1', '<source><![CDATA[<b>x</b>]]></source><target>t<![CDATA[ & ]]>u</target>'),
    $tu('1', '<source>a &amp; b &lt; &#233; &#x1F600;</source><target>x<!-- c -->y<?pi z?>w</target>'),
    $tu('1', "<source>  spaced\n  text  </source><target>\n</target>"),
    $tu('1', '<source>s</source><target><g id="1">only</g></target>'),
    $tu('1', '<source><bpt id="1">[</bpt>b<ept id="1">]</ept></source><target><ph id="2">{p}</ph></target>'),
    $tu('1', '<source><mrk mtype="term">m</mrk></source><target>t<sub>no</sub></target>'),
    // notes
    $tu('1', '<source>s</source><target>t</target><note priority="1">a</note><note from="me">b</note><note priority="10" from="x" annotates="source">c</note><note lang="en">d</note>'),
    $tu('1', '<source>s</source><note></note><note priority="2"><![CDATA[cd]]></note>'),
    $tu('1', '<source>s</source><target>t</target><note xml:lang="fr">n</note>'),
    // target attributes
    $tu('1', '<source>s</source><target state="translated" state-qualifier="exact-match" xml:lang="fr" resname="r" phase-name="p">t</target>'),
    $tu('1', '<source>s</source><target xml:lang="fr">t</target>'),
    $tu('1', '<source>s</source><target xml:space="preserve">t</target>'),
    $tu('1', '<source xml:lang="en">s</source><target coord="1;2;3;4">t</target>'),
    // alt-trans, context, other children
    $tu('1', '<source>s</source><target>t</target><alt-trans><source>as</source><target>at</target></alt-trans><context-group><context context-type="sourcefile">f</context></context-group>'),
    $tu('1', '<source>s</source><seg-source>seg</seg-source><target>t</target>'),
    // attributes of the trans-unit
    $tu('007', '<source>s</source><target>t</target>', 'translate="no" approved="yes" xml:space="preserve" extradata="e"'),
    // groups
    '<group id="g1" resname="grp"><trans-unit id="1"><source>in group</source><target>g</target></trans-unit><group id="g2"><trans-unit id="2"><source>nested</source><target>n</target></trans-unit></group></group>',
    // bin-unit
    '<bin-unit id="b1" mime-type="image/png"><bin-source><external-file href="a.png"/></bin-source></bin-unit>'.$tu('1', '<source>s</source><target>t</target>'),
    // empty body
    '',
];
foreach ($units12 as $u) {
    $string($x12($u));
}

// prefixes, foreign elements and attributes
$string('<?xml version="1.0"?><x:xliff xmlns:x="urn:oasis:names:tc:xliff:document:1.2" version="1.2"><x:file source-language="en" datatype="plaintext" original="o"><x:body><x:trans-unit id="1"><x:source>prefixed</x:source><x:target>p</x:target></x:trans-unit></x:body></x:file></x:xliff>');
$string('<?xml version="1.0"?><x:xliff xmlns:x="urn:oasis:names:tc:xliff:document:1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"><file source-language="en" datatype="plaintext" original="o"><body><trans-unit id="1"><x:source>ps</x:source><target>t</target></trans-unit><trans-unit id="2"><source>s2</source><x:target>pt</x:target></trans-unit><x:trans-unit id="3" resname="r3"><source>s3</source><target>t3</target><x:note>pn</x:note><note>n</note></x:trans-unit></body></file></x:xliff>');
$string($x12($tu('1', '<source>s</source><target>t</target>', 'xmlns:o="urn:other" o:resname="other"')));
$string($x12($tu('1', '<source>s</source><target xmlns:o="urn:other" o:a="1" b="2">t</target>')));
$string($x12($tu('1', '<source>s</source><target>t</target><o:note xmlns:o="urn:other">foreign</o:note>')));
$string($x12($tu('1', '<source>s</source><target>t</target>'), 'source-language="en" datatype="plaintext" original="12"'));
$string($x12($tu('1', '<source>s</source><target>t</target>'), 'source-language="en" datatype="plaintext" original=""'));
$string($x12($tu('1', '<source>s</source><target>t</target>'), 'source-language="en" datatype="plaintext" original="o" xmlns:o="urn:other" o:original="x"'));
// props
$withHeader = fn (string $header, string $units) => '<?xml version="1.0"?>'."\n".'<xliff xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"><file source-language="en" datatype="plaintext" original="o"><header>'.$header.'</header><body>'.$units.'</body></file></xliff>';
$string($withHeader('<prop-group name="p"><prop prop-type="a">1</prop><prop prop-type="b" xml:lang="en">two</prop><prop prop-type="a">3</prop></prop-group>', $tu('1', '<source>s</source><target>t</target>')));
$string($withHeader('<prop-group><prop prop-type="">e</prop></prop-group>', $tu('1', '<source>s</source>')));
$string($withHeader('<prop-group><prop prop-type="7">seven</prop></prop-group>', $tu('1', '<source>s</source>')));
$string($withHeader('<tool tool-id="t" tool-name="n"/><note>header note</note>', $tu('1', '<source>s</source>')));
// several files
$string('<?xml version="1.0"?><xliff xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"><file source-language="en" datatype="plaintext" original="a"><body>'.$tu('1', '<source>s</source><target>a</target>').'</body></file><file source-language="en" datatype="plaintext" original="b"><header><prop-group><prop prop-type="k">v</prop></prop-group></header><body>'.$tu('1', '<source>s</source><target>b</target>').'</body></file></xliff>');
// versions
foreach (['', 'version="1.2"', 'version="1.1"', 'version="1.0"', 'version=""', 'version="2.0"', 'version="3.0"', 'version="1.2 "'] as $v) {
    $string('<?xml version="1.0"?><xliff xmlns="urn:oasis:names:tc:xliff:document:1.2" '.$v.'><file source-language="en" datatype="plaintext" original="o"><body>'.$tu('1', '<source>s</source><target>t</target>').'</body></file></xliff>');
}
$string('<?xml version="1.0"?><xliff version="1.2"><file source-language="en" datatype="plaintext" original="o"><body>'.$tu('1', '<source>s</source>').'</body></file></xliff>');
$string('<?xml version="1.0"?><xliff xmlns="urn:oasis:names:tc:xliff:document:2.0"><file id="f"><unit id="u"><segment><source>s</source></segment></unit></file></xliff>');
$string('<?xml version="1.0"?><x:xliff xmlns:x="urn:oasis:names:tc:xliff:document:2.0" xmlns:v="urn:v" v:version="2.0" version="2.0" srcLang="en"><x:file id="f"><x:unit id="u"><x:segment><x:source>s</x:source></x:segment></x:unit></x:file></x:xliff>');
$string('<?xml version="1.0"?><x:xliff xmlns:x="urn:oasis:names:tc:xliff:document:1.2" xmlns:v="urn:v" v:version="2.0"><x:file source-language="en" datatype="plaintext" original="o"><x:body/></x:file></x:xliff>');
$string('<?xml version="1.0"?><root><xliff version="2.0"/></root>');
$string('<?xml version="1.0"?><a/>');
// not well-formed, document types, invalid
$string('<?xml version="1.0"?><xliff');
$string('<?xml version="1.0"?>');
$string("<?xml version=\"1.0\"?>\n<!DOCTYPE xliff>".substr($x12(''), \strlen('<?xml version="1.0" encoding="utf-8"?>')));
$string($x12($tu('1', '<target>t</target>')));
$string($x12($tu('1', '<source>s</source><target>t</target><bogus/>')));
$string($x12('<trans-unit><source>s</source></trans-unit>'));
$string($x12($tu('1', '<source>a</source>').$tu('1', '<source>b</source>')));
$string($x12($tu('1', '<source>s</source><target state="bogus">t</target>')));
$string($x12($tu('1', '<source>s</source><note priority="x">n</note>')));
$string($x12($tu('1', '<source>s</source><note priority="11">n</note>')));

// encodings (strings are bytes; files below)
foreach (['ISO-8859-1', 'iso-8859-1', 'latin1', 'Latin1', 'ISO_8859-1', 'ISO8859-1', 'ISO-LATIN-1', 'L1', 'US-ASCII', 'ascii', 'UTF-8', 'utf8', 'UTF8'] as $enc) {
    $body = $tu('1', '<source>f&#246;&#246;</source><target>b&#228;r &#8364; &#x1F600; &#128;</target><note>n&#252;</note>', 'resname="r&#233;s"');
    $string($x12($body, 'source-language="en" datatype="plaintext" original="&#233;"', 'xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"', "<?xml version=\"1.0\" encoding=\"$enc\"?>"));
}
$string($x12($tu('1', "<source>caf\xE9</source><target>\xE9t\xE9</target>"), 'source-language="en" datatype="plaintext" original="o"', 'xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"', '<?xml version="1.0" encoding="ISO-8859-1"?>'));
$string($x2('<unit id="u" name="n&#233;"><notes><note>&#233;</note></notes><segment><source>s</source><target>&#233;t&#233; &#8364;</target></segment></unit>', '2.0', '', '<?xml version="1.0" encoding="ISO-8859-1"?>'));
$string($x2('<unit id="u" name="n"><segment><source>s</source><target>&#233;</target></segment></unit>', '2.0', '', '<?xml version="1.0" encoding="latin1"?>'));
$string($x2('<unit id="u" name="n" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>&#233;&#8364;</target></segment></unit>', '2.2', 'xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"', '<?xml version="1.0" encoding="ISO-8859-1"?>'));
$string($x2('<unit id="u" name="n" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>&#233;</target></segment></unit>', '2.2', 'xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"', '<?xml version="1.0" encoding="L1"?>'));
// encodings as files: UTF-16 and byte-order marks
$utf16 = $x12($tu('1', '<source>s&#233;</source><target>t&#233; &#x1F600;</target><note>n</note>'), 'source-language="en" datatype="plaintext" original="o"', 'xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"', '<?xml version="1.0" encoding="UTF-16"?>');
$put('utf16le.xlf', "\xFF\xFE".mb_convert_encoding($utf16, 'UTF-16LE', 'UTF-8'));
$put('utf16be.xlf', "\xFE\xFF".mb_convert_encoding($utf16, 'UTF-16BE', 'UTF-8'));
$put('utf16nobom.xlf', mb_convert_encoding(str_replace('UTF-16', 'UTF-16LE', $utf16), 'UTF-16LE', 'UTF-8'));
$put('utf16be-decl.xlf', mb_convert_encoding(str_replace('UTF-16', 'UTF-16BE', $utf16), 'UTF-16BE', 'UTF-8'));
$put('bom.xlf', "\xEF\xBB\xBF".$x12($tu('1', '<source>s</source><target>t&#233;</target>')));
$put('nodecl.xlf', substr($x12($tu('1', '<source>s</source><target>t&#233;</target>')), \strlen('<?xml version="1.0" encoding="utf-8"?>')));
$put('ws.xlf', " \n\t");
$put('ascii.xlf', $x12($tu('1', '<source>s</source><target>t&#233;</target>'), 'source-language="en" datatype="plaintext" original="o"', 'xmlns="urn:oasis:names:tc:xliff:document:1.2" version="1.2"', '<?xml version="1.0" encoding="US-ASCII"?>'));
foreach (['utf16le.xlf', 'utf16be.xlf', 'utf16nobom.xlf', 'utf16be-decl.xlf', 'bom.xlf', 'nodecl.xlf', 'ws.xlf', 'ascii.xlf'] as $f) {
    $file($f);
}

// XLIFF 2 -------------------------------------------------------------------

$pgs = 'xmlns:pgs="urn:oasis:names:tc:xliff:pgs:1.0"';
$mda = 'xmlns:mda="urn:oasis:names:tc:xliff:metadata:2.0"';
$units2 = [
    ['<unit id="u1"><segment><source>s1</source><target>t1</target></segment></unit><unit id="u2" name="n2"><segment><source>s2</source></segment></unit>', '2.0', ''],
    ['<unit id="u1" name=""><segment><source>s</source><target>t</target></segment></unit>', '2.0', ''],
    ['<unit id="u1" name="0"><segment><source>s</source><target>t</target></segment></unit>', '2.0', ''],
    ['<unit id="u1" name="5"><segment><source>s</source><target>t</target></segment></unit><unit id="u2"><segment><source>6</source><target>six</target></segment></unit>', '2.0', ''],
    ['<unit id="u1"><segment><source>a</source><target>1</target></segment><segment><source>b</source><target>2</target></segment><ignorable><source> </source></ignorable></unit>', '2.0', ''],
    ['<unit id="u1" name="n"><segment><source>a</source><target>1</target></segment><segment id="s2" state="final"><source>b</source><target order="2">2</target></segment></unit>', '2.0', ''],
    ['<unit id="u1"><segment id="x" state="translated" subState="my:sub" canResegment="no"><source>s</source><target xml:lang="fr" xml:space="preserve" order="1">t</target></segment></unit>', '2.0', ''],
    ['<unit id="u1"><notes><note>plain</note><note id="n1" appliesTo="source" category="c" priority="3">full</note><note></note></notes><segment><source>s</source><target>t</target></segment></unit>', '2.0', ''],
    ['<unit id="u1"><notes><note>n</note></notes><segment><source>a</source></segment><segment><source>b</source></segment></unit>', '2.0', ''],
    ['<unit id="u1"><segment><source>a<pc id="1">b</pc>c<ph id="2"/>d</source><target>x<mrk id="m1" type="term">y</mrk>z<cp hex="41"/>w<![CDATA[cd]]></target></segment></unit>', '2.0', ''],
    ['<unit id="u1"><originalData><data id="d1">&lt;b&gt;</data></originalData><segment><source><ph id="1" dataRef="d1"/>s</source><target>t</target></segment></unit>', '2.0', ''],
    ['<group id="g"><unit id="u1"><segment><source>in group</source><target>g</target></segment></unit><group id="g2"><unit id="u2" name="deep"><segment><source>x</source></segment></unit></group></group>', '2.0', ''],
    ['<mda:metadata><mda:metaGroup category="c"><mda:meta type="k">v</mda:meta><mda:meta type="k2">v2</mda:meta><mda:meta type="k">again</mda:meta></mda:metaGroup></mda:metadata><unit id="u1"><mda:metadata><mda:metaGroup><mda:meta type="u">unit meta</mda:meta></mda:metaGroup></mda:metadata><segment><source>s</source></segment></unit>', '2.0', $mda],
    ['<mda:metadata><mda:metaGroup><mda:meta type="">empty</mda:meta><mda:meta type="3">three</mda:meta></mda:metaGroup></mda:metadata><unit id="u1"><segment><source>s</source></segment></unit>', '2.0', $mda],
    ['<unit id="u1"><segment><source>s</source><target>t</target></segment></unit>', '2.1', ''],
    ['<unit id="u1"><segment><source>s</source><target>t</target></segment></unit>', '2.2', ''],
    ['<unit id="u1"><segment><source>s</source><target>t</target></segment></unit>', '2.3', ''],
    ['<unit id="u1"><segment><source>s</source><target>t</target></segment></unit>', '', ''],
    // pgs
    ['<unit id="u" name="files" pgs:switch="plural:n"><segment pgs:case="0"><source>none</source><target>aucun</target></segment><segment pgs:case="one"><source>one</source><target>un</target></segment><segment pgs:case="other"><source>#</source><target><ph id="1" disp="n"/> fichiers</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" pgs:switch="ordinal:pos"><segment pgs:case="1"><source>s</source><target>1er</target></segment><segment pgs:case="other"><source>s</source><target><ph id="1" disp="pos"/>e</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="g" pgs:switch="gender:who"><segment pgs:case="feminine"><source>s</source><target>elle <ph id="1" disp="who"/></target></segment><segment pgs:case="0"><source>s</source><target>zero</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="c" pgs:switch="select:a plural:b"><segment pgs:case="x 0"><source>s</source><target>x0</target></segment><segment pgs:case="x other"><source>s</source><target>xo <ph id="1" disp="b"/> <ph id="2" disp="a"/></target></segment><segment pgs:case="y"><source>s</source><target>y</target></segment><segment pgs:case="y one two"><source>s</source><target>y12</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="c3" pgs:switch="select:a select:b plural:c"><segment pgs:case="p q 1"><source>s</source><target>pq1</target></segment><segment pgs:case="p q other"><source>s</source><target>pqo</target></segment><segment pgs:case="p r"><source>s</source><target>pr</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="m" pgs:switch="  plural:n   select:g  "><segment pgs:case="1 a"><source>s</source><target>t</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="nocase" pgs:switch="plural:n"><segment><source>s</source><target>t</target></segment><segment pgs:case="1.5"><source>s</source><target>f</target></segment><segment pgs:case="-1"><source>s</source><target>m</target></segment><segment pgs:case="1e2"><source>s</source><target>e</target></segment><segment pgs:case=" 1"><source>s</source><target>sp</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="dup" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>first</target></segment><segment pgs:case="a"><source>s</source><target>second</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="src" pgs:switch="select:g"><segment pgs:case="a"><source>only <ph id="1" disp="g"/> source</source></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="inl" pgs:switch="plural:n"><segment pgs:case="other"><source>s</source><target>a<pc id="1">b<ph id="2" disp="n"/><mrk id="m" type="term">c<cp hex="1F600"/></mrk></pc><sc id="3"/>d<ec startRef="3"/><sm id="4" type="term"/>e<em startRef="4"/><ph id="5"/><ph id="6" disp=""/><ph id="7" disp="other"/></target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="cp" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target><cp hex="0041"/><cp hex="00e9"/><cp hex="110000"/><cp hex="D800"/><cp hex="DFFF"/><cp hex="E000"/><cp hex="10FFFF"/><cp hex="1"/><cp hex="FFFE"/></target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="cpx" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target><cp hex="FFFFFFFFFFFFFFFFFF"/>.</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="cd" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>x<![CDATA[ {y} ]]>z &amp; w</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="notes" pgs:switch="select:g"><notes><note category="c">n</note></notes><segment pgs:case="a"><source>s</source><target>t</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="nonotes" pgs:switch="select:g"><notes><note>n</note></notes><segment pgs:case="a"><source>s</source><target>t</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="nocases" pgs:switch="select:g"><segment><source>s</source><target>t</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>t</target></segment></unit><unit id="u2" name="u"><segment><source>s</source><target>plain</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u2" name="k"><segment><source>s</source><target>plain</target></segment></unit><unit id="u" name="k" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>icu</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="k" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>icu</target></segment></unit><unit id="u2" name="k"><segment><source>s</source><target>plain</target></segment></unit>', '2.2', $pgs],
    ['<unit id="u" name="v" pgs:switch="plural:n"><segment pgs:case="other"><source>s</source><target>t</target></segment></unit>', '2.0', $pgs],
    ['<unit id="u" name="v" pgs:switch="plural:n"><segment pgs:case="other"><source>s</source><target>t</target></segment></unit>', '2.1', $pgs],
];
foreach (['', '   ', "\t", 'plural', 'plural:', ':n', 'a:b:c', 'plural:n :x', 'plural:n x', 'a::b'] as $sw) {
    $units2[] = ['<unit id="u" name="bad" pgs:switch="'.$sw.'"><segment pgs:case="other"><source>s</source><target>t</target></segment></unit>', '2.2', $pgs];
}
foreach ($units2 as [$u, $v, $extra]) {
    $string($x2($u, $v, $extra));
}
$string($x2('<unit id="u1"><segment><source>s</source><target>t</target></segment></unit>', '2.0', '', '<?xml version="1.0" encoding="utf-8"?>'), 'en', 'messages+intl-icu');
$string($x2('<unit id="u" name="k" pgs:switch="select:g"><segment pgs:case="a"><source>s</source><target>icu</target></segment></unit>', '2.2', $pgs), 'en', 'd+intl-icu');
$string($x12($tu('1', '<source>s</source><target>t</target>')), '', '');
$string($x12($tu('1', '<source>s</source><target>t</target>')), 'en_US', '0');

// getFileUrl, getVersionNumber ----------------------------------------------

$getFileUrl = new \ReflectionMethod(XliffUtils::class, 'getFileUrl');
$urls = [];
foreach (['/tmp/symfony123', '/tmp/dir with space/symfony123', '/tmp/中文/symfony123', '', '/', 'rel/a', '/a/b/', '/a%20b', "/a\x00b", '/a+b&c=d#e?f', '/ä/ü', "/\xFF", '/a\\b', '/~user/.x'] as $p) {
    $urls[] = [base64_encode($p), base64_encode($getFileUrl->invoke(null, $p))];
}

$versions = [];
foreach ([
    '<xliff version="1.2"/>', '<xliff/>', '<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0"/>', '<x:xliff xmlns:x="urn:x" version="9"/>',
    '<xliff xmlns:v="urn:v" v:version="3"/>', '<xliff version=""/>', '<a><xliff version="2.1"/><xliff version="2.0"/></a>', '<a><xliff/><xliff version="2.0"/></a>',
    '<XLIFF version="2.0"/>', '<a/>', '<xliff xmlns:v="urn:v" v:version="3" version="2.0"/>', '<xliff xmlns:v="urn:v" version="2.0" v:version="3"/>',
    '<xliff xmlns:v="urn:v" xmlns:w="urn:w" w:version="4" v:version="3"/>',
] as $xml) {
    $dom = new \DOMDocument();
    $dom->loadXML($xml);
    $versions[] = [base64_encode($xml), typed_call(fn () => XliffUtils::getVersionNumber($dom))];
}

// DOMDocument::getElementsByTagName(), which getVersionNumber() uses
$tagNames = [];
foreach (['<a xmlns:x="urn:u" xmlns:y="urn:v"><x:b/><b/><y:b/><x:c/><z:b xmlns:z="urn:u"/></a>', '<a xmlns="urn:u"><b><b/></b></a>'] as $xml) {
    $dom = new \DOMDocument();
    $dom->loadXML($xml);
    foreach (['a', 'b', 'x:b', 'y:b', 'c', 'x:c', 'z:b', '*', 'B', ''] as $name) {
        $found = [];
        foreach ($dom->getElementsByTagName($name) as $e) {
            $found[] = $e->nodeName;
        }
        $tagNames[] = [base64_encode($xml), $name, $found];
    }
}

// validateSchema() on a document with an external entity declared (XliffUtilsTest)
$dom = new \DOMDocument();
$dom->loadXML('<?xml version="1.0"?><!DOCTYPE xliff [<!ENTITY xxe SYSTEM "http://127.0.0.1:1/payload.dtd">]><xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2"><file source-language="en" datatype="plaintext" original="file.ext"><body/></file></xliff>', \LIBXML_NONET);
$entityCase = typed_call(fn () => XliffUtils::validateSchema($dom));

echo json_encode([
    'base' => $base,
    'files' => $files,
    'cases' => $cases,
    'file_urls' => $urls,
    'versions' => $versions,
    'tag_names' => $tagNames,
    'external_entity' => $entityCase,
], \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
