<?php
// What Symfony\Component\Translation\Loader\QtFileLoader and the DOMXPath
// evaluate() calls it makes answer, for crates/translation/tests/data/qt.json
// (scripts/record.sh translation-qt).
//
// Recorded beyond QtFileLoaderTest:
// - load() of upstream's resources.ts and of written documents: the
//   catalogue (messages, resources) or the throwable chain (class, message,
//   code), and libxml_use_internal_errors() before and after. The documents
//   cover the XPath '//TS/context/name[text()="<domain>"]' (depth of TS,
//   namespaces and prefixes, text split by comments, CDATA, entity
//   references, whitespace, several matches), the walk from the name to
//   every <message> under TS (all contexts, prefixed elements by local
//   name, a name with no next sibling), and the values (missing elements,
//   '0', '', textContent of children, integer keys, duplicates);
// - domains that change the XPath expression (quotes, brackets);
// - resources: wrappers, missing files, directories, empty and invalid
//   files;
// - evaluate() of location paths over a document: the names of the nodes
//   found, in document order, or false.
//
// Paths: files are written under $base and the cases run there. The replay
// substitutes its own directory.

require '/demo/vendor/autoload.php';

use Symfony\Component\Translation\Loader\QtFileLoader;
use Symfony\Component\Translation\MessageCatalogue;

$base = '/tmp/qt';
@mkdir($base);
@mkdir("$base/dir");
chdir($base);

$files = [];
$put = function (string $name, string $content) use ($base, &$files) {
    file_put_contents("$base/$name", $content);
    $files[$name] = base64_encode($content);
};

$fixtures = '/work/upstream/symfony/translation/Tests/Fixtures/';
foreach (['resources.ts', 'empty.xlf', 'invalid-xml-resources.xlf'] as $f) {
    $put($f, file_get_contents($fixtures.$f));
}

// Warnings (null property reads, an invalid XPath) are not recorded.
set_error_handler(fn () => true);

/** A throwable and its previous ones: [class, message, code] each. */
function chain(\Throwable $e): array
{
    $out = [];
    for (; $e; $e = $e->getPrevious()) {
        $out[] = [$e::class, base64_encode($e->getMessage()), $e->getCode()];
    }

    return $out;
}

/** messages as [domain, [[key type, key, value], ...]] pairs, in order. */
function dump_catalogue(MessageCatalogue $c): array
{
    $messages = (new \ReflectionProperty(MessageCatalogue::class, 'messages'))->getValue($c);
    $out = [];
    foreach ($messages as $domain => $pairs) {
        $list = [];
        foreach ($pairs as $k => $v) {
            $list[] = [\is_int($k) ? 'int' : 'string', base64_encode((string) $k), base64_encode($v)];
        }
        $out[] = [base64_encode((string) $domain), $list];
    }

    return [
        'locale' => $c->getLocale(),
        'messages' => $out,
        'resources' => array_map(fn ($r) => base64_encode((string) $r), $c->getResources()),
    ];
}

function load_case(string $resource, string $locale, string $domain, bool $internal): array
{
    libxml_use_internal_errors($internal);
    try {
        $c = (new QtFileLoader())->load($resource, $locale, $domain);
        $out = ['ok', dump_catalogue($c)];
    } catch (\Throwable $e) {
        $out = ['throws', chain($e)];
    }
    $out[] = [$internal, libxml_use_internal_errors()];
    libxml_clear_errors();
    libxml_use_internal_errors(false);

    return $out;
}

$cases = [];
$load = function (string $kind, string $resource, string $domain = 'resources', bool $internal = false, string $locale = 'en') use (&$cases) {
    $cases[] = [$kind, base64_encode($resource), $locale, base64_encode($domain), load_case($resource, $locale, $domain, $internal)];
};
$n = 0;
$doc = function (string $xml, string $domain = 'resources') use ($put, $load, &$n) {
    $name = sprintf('doc%03d.ts', $n++);
    $put($name, $xml);
    $load('file', $name, $domain);
};

// Upstream fixture ----------------------------------------------------------

$load('file', 'resources.ts');
$load('file', 'resources.ts', 'resources', true);
$load('absolute', "$base/resources.ts");
$load('file', './resources.ts');
$load('file', 'dir/../resources.ts');
$load('file', 'resources.ts', 'domain1');
$load('file', 'resources.ts', '');
$load('file', 'resources.ts', 'Resources');
$load('file', 'resources.ts', 'resources ');
$load('file', 'resources.ts', 'resources', false, 'fr_FR');

// Resources -----------------------------------------------------------------

foreach (['non-existing.ts', 'http://domain1.com/resources.ts', 'https://x/r.ts', 'ftp://x/r.ts', 'data://text/plain,abc',
    'data:text/plain,abc', 'php://memory', 'glob://*.ts', 'compress.zlib://resources.ts', "file://$base/resources.ts",
    "FILE://$base/resources.ts", "file://localhost$base/resources.ts", 'file://host/r.ts', 'file://resources.ts',
    "file://$base/none.ts", "phar://$base/none.phar/a.ts", 'dir', 'dir/', "$base/dir", '', '.', 'empty.xlf',
    'invalid-xml-resources.xlf', 'resources.ts ',
] as $r) {
    $load('file', $r);
}
$load('file', 'empty.xlf', 'resources', true);
$load('file', 'invalid-xml-resources.xlf', 'resources', true);

// Documents -----------------------------------------------------------------

$ts = fn (string $contexts, string $root = 'TS', string $attrs = '') => "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<$root$attrs>\n$contexts\n</".explode(' ', $root)[0].">\n";
$ctx = fn (string $name, string $messages, string $tag = 'context') => "  <$tag>\n    <name>$name</name>\n$messages\n  </$tag>";
$msg = fn (string $source, string $translation) => "    <message>\n      <source>$source</source>\n      <translation>$translation</translation>\n    </message>";

// the walk: every <message> under TS
$doc($ts($ctx('resources', $msg('a', 'A')).$ctx('other', $msg('b', 'B'))));
$doc($ts($ctx('other', $msg('b', 'B')).$ctx('resources', $msg('a', 'A'))));
$doc($ts($ctx('resources', $msg('a', 'A')).$ctx('resources', $msg('b', 'B'))));
$doc($ts($ctx('resources', $msg('a', 'A'))."\n  <message><source>top</source><translation>level</translation></message>"));
$doc($ts($ctx('resources', "    <group>\n".$msg('deep', 'D')."\n    </group>")));
$doc("<?xml version=\"1.0\"?>\n<root><TS>".$ctx('resources', $msg('a', 'A'))."</TS><TS>".$ctx('other', $msg('b', 'B'))."</TS></root>");
$doc("<?xml version=\"1.0\"?>\n<root><x><TS>".$ctx('resources', $msg('a', 'A'))."</TS></x></root>");
$doc("<?xml version=\"1.0\"?>\n<TS><context><name>resources</name></context></TS>");
$doc("<?xml version=\"1.0\"?>\n<TS><context><name>resources</name><message><source>a</source><translation>A</translation></message></context></TS>");
$doc("<?xml version=\"1.0\"?>\n<TS><context><message><source>a</source><translation>A</translation></message><name>resources</name></context></TS>");
$doc("<?xml version=\"1.0\"?>\n<TS><context><name>resources</name><!-- c --></context><context><message><source>a</source><translation>A</translation></message></context></TS>");
$doc("<?xml version=\"1.0\"?>\n<TS><context><name>resources</name>\n</context></TS>");

// the XPath: names, namespaces, text nodes
$doc($ts($ctx('resources', $msg('a', 'A')), 'TS xmlns="urn:x"'));
$doc($ts($ctx('resources', $msg('a', 'A')), 'q:TS xmlns:q="urn:x"'));
$doc($ts("  <q:context xmlns:q=\"urn:x\"><name>resources</name>".$msg('a', 'A')."</q:context>"));
$doc($ts("  <context><q:name xmlns:q=\"urn:x\">resources</q:name>".$msg('a', 'A')."</context>"));
$doc($ts("  <context><name xmlns=\"urn:x\">resources</name>".$msg('a', 'A')."</context>"));
$doc($ts("  <context xmlns=\"\"><name>resources</name>".$msg('a', 'A')."</context>"));
$doc($ts($ctx('resources', $msg('a', 'A')), 'ts'));
$doc($ts($ctx('resources', $msg('a', 'A'), 'Context')));
$doc($ts($ctx(' resources ', $msg('a', 'A'))));
$doc($ts($ctx('<![CDATA[resources]]>', $msg('a', 'A'))));
$doc($ts($ctx('res<![CDATA[ources]]>', $msg('a', 'A'))));
$doc($ts($ctx('res<!-- c -->ources', $msg('a', 'A'))));
$doc($ts($ctx('resources<!-- c -->', $msg('a', 'A'))));
$doc($ts($ctx('resources<b/>other', $msg('a', 'A'))));
$doc($ts($ctx('other<b/>resources', $msg('a', 'A'))));
$doc($ts($ctx('<b>resources</b>', $msg('a', 'A'))));
$doc($ts($ctx('res&amp;ources', $msg('a', 'A'))), 'res&ources');
$doc($ts($ctx('&#114;esources', $msg('a', 'A'))));
$doc("<?xml version=\"1.0\"?>\n<!DOCTYPE TS [<!ENTITY r \"resources\">]>\n<TS><context><name>&r;</name>".$msg('a', 'A')."</context></TS>");
$doc($ts("  <context><name>resources</name><name>resources</name>".$msg('a', 'A')."</context>"));
$doc($ts("  <context><name>resources</name><name>other</name>".$msg('a', 'A')."</context>"));
$doc($ts("  <context><name>other</name><name>resources</name>".$msg('a', 'A')."</context>"));
$doc($ts("  <context name=\"resources\">".$msg('a', 'A')."</context>"));
$doc($ts($ctx('résources', $msg('a', 'A'))), 'résources');
$doc($ts($ctx('it\'s', $msg('a', 'A'))), 'it\'s');

// values
$doc($ts($ctx('resources', $msg('a', '0').$msg('b', '').$msg('c', ' ').$msg('d', '00').$msg('e', '0.0'))));
$doc($ts($ctx('resources', "    <message><source>a</source></message>\n    <message><translation>T</translation></message>\n    <message/>")));
$doc($ts($ctx('resources', $msg('1', 'one').$msg('01', 'zero one').$msg('-1', 'minus').$msg('1.5', 'x').$msg(' 2', 'y').$msg('9223372036854775808', 'big'))));
$doc($ts($ctx('resources', $msg('a', 'first').$msg('a', 'second'))));
$doc($ts($ctx('resources', $msg('a<b>x</b>y', 'T<i>u</i>v<!-- c -->w'))));
$doc($ts($ctx('resources', $msg('<![CDATA[<a>]]>', '<![CDATA[&amp;]]>').$msg('&lt;b&gt;', '&#169; &#x263A;'))));
$doc($ts($ctx('resources', "    <message><source>a</source><source>b</source><translation>T1</translation><translation>T2</translation></message>")));
$doc($ts($ctx('resources', "    <message><extra><source>inner</source><translation>I</translation></extra><source>outer</source></message>")));
$doc($ts($ctx('resources', "    <message xmlns:q=\"urn:x\"><q:source>p</q:source><q:translation>P</q:translation></message>\n    <q:message xmlns:q=\"urn:x\"><source>m</source><translation>M</translation></q:message>")));
$doc($ts($ctx('resources', "    <message><source>a</source><translation type=\"unfinished\">U</translation></message>\n    <message><source>b</source><translation><numerusform>one</numerusform><numerusform>many</numerusform></translation></message>")));
$doc($ts($ctx('resources', $msg('a', "multi\nline\r\n  text"))));
$doc($ts($ctx('resources', ''), 'TS', ' version="2.1" language="fr"'));

// domains that change the expression
foreach (['a"b', '"', 'resources"]', 'x"] | //message/source[text()="a', 'resources"][1', '"] | //*[1', ']', "\xff", "res\x00", 'resources+intl-icu'] as $d) {
    $load('file', 'resources.ts', $d);
}
$doc($ts($ctx('x"] | //message/source[text()="a', $msg('a', 'A'))), 'x"] | //message/source[text()="a');

// evaluate() --------------------------------------------------------------

$xml = "<?xml version=\"1.0\"?>\n<!DOCTYPE r [<!ENTITY e \"ent\">]>\n<r xmlns:q=\"urn:q\"><TS><context><name>a</name><name>b<!--c-->c</name><name><![CDATA[d]]></name><name>&e;</name></context><q:context><name>a</name></q:context></TS><x><TS><context><name>a</name></context></TS></x><y xmlns=\"urn:d\"><TS><context><name>a</name></context></TS></y><TS/><ts/></r>";
$dom = new \DOMDocument();
$dom->loadXML($xml);
$xp = new \DOMXPath($dom);
$path = function (\DOMNode $n): string {
    $parts = [];
    for (; $n && !$n instanceof \DOMDocument; $n = $n->parentNode) {
        $i = 0;
        for ($s = $n; $s; $s = $s->previousSibling) {
            ++$i;
        }
        $parts[] = $i;
    }

    return implode('/', array_reverse($parts));
};
$evaluate = [];
foreach (['//TS', '/TS', '/r', '/r/TS', '//TS/context', '//TS/context/name', '//context/name', '/r//name', '//name[text()="a"]',
    '//name[text()="b"]', '//name[text()="c"]', '//name[text()="bc"]', '//name[text()="d"]', '//name[text()="ent"]',
    "//name[text()='a']", '//TS/context/name[text()="a"]', '//*', '//r', '/*', '/*/TS', '//TS//name', '//q:context',
    '// TS', '//TS /context', '//name[ text() = "a" ]', '//name[text()="a"][text()="a"]', '//name[text()=""]',
    '//TS/context/name[text()="a"b"]', '//TS/context/name[text()="', '//TS[', '//', '', 'TS', 'TS/context',
    '//name[text()="a"] | //TS', '//name[1]', '//name[@x]', '//text()',
] as $expr) {
    $r = $xp->evaluate($expr);
    $evaluate[] = [base64_encode($expr), $r instanceof \DOMNodeList ? array_map($path, iterator_to_array($r)) : (false === $r ? false : 'other')];
}

echo json_encode([
    'base' => $base,
    'files' => $files,
    'cases' => $cases,
    'xpath_document' => base64_encode($xml),
    'evaluate' => $evaluate,
], \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES);
