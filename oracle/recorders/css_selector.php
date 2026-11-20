<?php
// symfony/css-selector vectors (scripts/record.sh css-selector), as typed
// JSON (lib/typed.php):
//
//   providers  every #[DataProvider] of every upstream test class, by class
//              path (e.g. "Parser/ParserTest"), as test_providers.php
//              prints them, except that objects keep their private
//              properties (the Node and Token classes keep all their state
//              there): ["object", class, [[name, value], ...]].
//   dom        for the TranslatorTest rows that evaluate the XPath on a
//              document: the XPath PHP generated and what it selected.
//   cases      cases the tests leave open: CssSelectorConverter::toXPath()
//              with and without the HTML extension, or the exception.
//
//   php css_selector.php

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\CssSelector\CssSelectorConverter;
use Symfony\Component\CssSelector\XPath\Extension\HtmlExtension;
use Symfony\Component\CssSelector\XPath\Translator;

$tests = '/work/upstream/symfony/css-selector/Tests';
spl_autoload_register(static function (string $name) use ($tests): void {
    $prefix = 'Symfony\\Component\\CssSelector\\Tests\\';
    if (str_starts_with($name, $prefix) && is_file($file = $tests . '/' . str_replace('\\', '/', substr($name, \strlen($prefix))) . '.php')) {
        require_once $file;
    }
});

/** typed_value(), with every initialized property of an object. */
function css_typed(mixed $v): array
{
    if (\is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $item) {
            $pairs[] = [typed_value($k), css_typed($item)];
        }

        return ['array', $pairs];
    }
    if (!\is_object($v)) {
        return typed_value($v);
    }
    $pairs = [];
    foreach ((new ReflectionObject($v))->getProperties() as $property) {
        if (!$property->isStatic() && $property->isInitialized($v)) {
            $pairs[] = [typed_value($property->getName()), css_typed($property->getValue($v))];
        }
    }

    return ['object', $v::class, $pairs];
}

$providers = [];
$files = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($tests, FilesystemIterator::SKIP_DOTS));
$paths = [];
foreach ($files as $file) {
    if (str_ends_with($file->getFilename(), 'Test.php')) {
        $paths[] = substr($file->getPathname(), \strlen($tests) + 1, -4);
    }
}
sort($paths);
foreach ($paths as $path) {
    $reflection = new ReflectionClass('Symfony\\Component\\CssSelector\\Tests\\' . str_replace('/', '\\', $path));
    $out = [];
    foreach ($reflection->getMethods() as $method) {
        foreach ($method->getAttributes(PHPUnit\Framework\Attributes\DataProvider::class) as $attribute) {
            $provider = $attribute->getArguments()[0];
            $rows = [];
            foreach ($reflection->getMethod($provider)->invoke(null) as $name => $arguments) {
                $rows[] = [(string) $name, array_map('css_typed', array_values($arguments))];
            }
            $out[$method->getName()] = ['provider' => $provider, 'rows' => $rows];
        }
    }
    ksort($out);
    if ($out) {
        $providers[$path] = $out;
    }
}

// The DOM rows: the XPath and what it selects, as TranslatorTest runs them.
function html_translator(): Translator
{
    $translator = new Translator();
    $translator->registerExtension(new HtmlExtension($translator));

    return $translator;
}

$dom = [];
$test = Symfony\Component\CssSelector\Tests\XPath\TranslatorTest::class;
foreach ($test::getXmlLangTestData() as [$css]) {
    $xpath = (new Translator())->cssToXPath($css);
    $document = new SimpleXMLElement(file_get_contents("$tests/XPath/Fixtures/lang.xml"));
    $ids = array_map(static fn ($e) => (string) $e->attributes()->id, $document->xpath($xpath));
    $dom['testXmlLang'][] = [typed_value($css), typed_value($xpath), typed_value($ids)];
}
foreach ($test::getHtmlIdsTestData() as [$css]) {
    $xpath = html_translator()->cssToXPath($css);
    $document = new DOMDocument();
    $document->strictErrorChecking = false;
    $internalErrors = libxml_use_internal_errors(true);
    $document->loadHTMLFile("$tests/XPath/Fixtures/ids.html");
    $ids = array_map(static fn ($e) => (string) $e->attributes()->id, simplexml_import_dom($document)->xpath($xpath));
    libxml_clear_errors();
    libxml_use_internal_errors($internalErrors);
    $dom['testHtmlIds'][] = [typed_value($css), typed_value($xpath), typed_value($ids)];
}
foreach ($test::getHtmlShakespearTestData() as [$css]) {
    $xpath = html_translator()->cssToXPath($css);
    $document = new DOMDocument();
    $document->strictErrorChecking = false;
    @$document->loadHTMLFile("$tests/XPath/Fixtures/shakespear.html");
    $bodies = simplexml_import_dom($document)->xpath('//body');
    $dom['testHtmlShakespear'][] = [typed_value($css), typed_value($xpath), typed_value(\count($bodies[0]->xpath($xpath)))];
}
foreach (['testOnlyOfTypeFindsSingleChildrenOfGivenType' => ['span:only-of-type'], 'testDisabledAndEnabledInsideFieldsets' => [':disabled', ':enabled']] as $name => $selectors) {
    foreach ($selectors as $css) {
        $dom[$name][] = [typed_value($css), typed_value(html_translator()->cssToXPath($css)), ['null']];
    }
}

// Cases the tests leave open.
$cases = [
    '', ' ', '  h1  ', '*', '0', 'h1', 'H1', 'h1:', '1', '-a', '--a', '_a', 'a-', 'a_b9',
    'é', '.é', '#é', 'a.é', "\xff", ".a\xc1b", "\xc1", '#1', '.1', 'a#b#c', 'x|y.z', 'x|*#z', 'X|Y',
    'A[HREF]', 'a[B=C]', 'a[b|c]', 'a[*|c]', 'a[b|=c]', 'a[b|c=d]', 'a[0]', '[a="0"]', '[a=0]', '[a=1.5]', '[a=-1]', '[a=+.5]',
    '[a~="0"]', '[a~=""]', '[a~="b c"]', '[a^="0"]', '[a^=""]', '[a$=""]', '[a$="0"]', '[a$="bcd"]', '[a*="0"]', '[a*=""]',
    '[a!=b]', '[a!=""]', '[a!="0"]', '[a|=""]', '[a|="0"]', '[a="it\'s"]', '[a=\'say "hi"\']', '[a="it\'s \\"x\\""]',
    '[a="\\""]', "[a='\\'']", '[a="b\\\nc"]', "[a=\"b\\\r\nc\"]", '[a="\\41"]', '[a="\\41 b"]', '[a="\\0000411"]',
    '[a="\\E9"]', '[a="\\7F"]', '[a="\\80"]', '[a="\\7FF"]', '[a="\\800"]', '[a="\\D800"]', '[a="\\FFFF"]', '[a="\\10000"]',
    '[a="\\10FFFF"]', '[a="\\110000"]', '[a="\\200041"]', '[a="\\g"]', "[a=\"b\nc\"]", '[a="b', "[a='b", '[a="b\'', '"a"',
    'a\\:b', '.a\\.b', '#a\\ b', '\\31 23', '\\61', '\\0061 b', 'a /* c */ b', 'a/* c */', '/* unclosed', '/**/a',
    'a:hover:first-child', 'a:first-child:hover', 'a:invalid', 'a:visited.b', 'a:root', ':ROOT', 'a:scope', 'a :scope',
    ':scope:scope', ':scope :scope', 'a, :scope', 'a,:scope', 'a:checked', 'a:link', 'a:selected', 'a:disabled', 'a:enabled',
    'a:foo', 'a:foo(1)', 'a:foo()', 'E:NTH-CHILD(1)', 'a:nth-child(0n+1)', 'a:nth-child(0)', 'a:nth-child(-0)', 'a:nth-child(1e1)',
    'a:nth-child(3.5)', 'a:nth-child( 2n + 1 )', 'a:nth-child(-2n+3)', 'a:nth-child(-2n-3)', 'a:nth-child(-n+2)', 'a:nth-child(n-0)',
    'a:nth-child(foo)', 'a:nth-child("1")', 'a:nth-child(n+)', 'a:nth-child(2n+1 )', 'a:nth-child(1n0)', 'a:nth-child(+-n)',
    'a:nth-last-child(-n+2)', 'a:nth-last-child(3)', 'a:nth-last-child(odd)', 'a:nth-of-type(n)', 'a:nth-of-type(2n+1)',
    '*:nth-of-type(1)', '*:nth-last-of-type(1)', '*:first-of-type', '*:last-of-type', 'a:only-of-type', '*:only-of-type',
    'a:empty', 'a:contains(1)', 'a:contains(a, b)', 'a:contains("it\'s")', 'a:contains(+)', 'a:lang(1)', 'a:lang(EN)', 'a:lang("EN-us")',
    'a:lang(-)', 'a::before', 'a:first-line', 'a:FIRST-LETTER', 'a::0', 'a::before b', 'a::before:hover', 'a:not(b)', 'a:not(*)',
    'a:not(.b)', 'a:not(b c)', 'a:not(::before)', 'a:not(:not(b))', 'a:not(x|y)', 'a:not(:hover)', 'a:not(:nth-child(2))', ':not(#a)',
    ':is()', ':where()', ':is(a, , b)', ':is(a b, c > d)', 'a:is(b', ':is(::before)', 'a:where(:not(b))', 'a:is(*)', ':has()', ':has(a)',
    'a:has(b, c)', 'a:has(~ b, > c d)', 'a:has(:has(b))', 'a:has(> :scope)', 'a:has(:scope b)', 'a:has(b :scope)', 'a:has(+ b + c)',
    'a:has(b', 'a:has(*)', 'a:has(> *, ~ *)', 'a b > c + d ~ e', 'a>b', 'a+b', 'a~b', 'a > > b', 'a,', ',a', 'a, b, c', "a\t\n,\r\x0cb",
    '[', '[a', '[a=', '[a=b', '[a b]', '[a=b c]', '[|a]', '[*|a]', '[a|]', '[a^b]', 'a[b="c"][d]', '.a.b#c[d]:hover', 'a!', 'a|',
    '*|*', '*|a', 'a|*', 'ns|*.c', ' .a ', '. a', '#', '##a', ':', '::', 'a:(', 'a:nth-child(', 'a:nth-child(1', 'a:nth-child(1,2)',
    '[*|=x]', '[*|=x', '0:scope', '0.foo', '0#a', 'a, 0', 'a:nth-last-child(-99999999999999999999)', 'a:nth-last-child(n-99999999999999999999)',
    'a:nth-child(99999999999999999999n+1)', 'a:nth-last-child(-n-9223372036854775807)', 'a:nth-child(-9223372036854775808)', 'a:nth-child(0n-0)',
];
$results = [];
foreach ($cases as $css) {
    $results[] = [
        typed_value($css),
        typed_call(static fn () => (new CssSelectorConverter(true))->toXPath($css)),
        typed_call(static fn () => (new CssSelectorConverter(false))->toXPath($css)),
        typed_call(static fn () => (new CssSelectorConverter(true))->toXPath($css, '')),
    ];
}

$json = static fn ($v) => json_encode($v, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
$lines = [];
foreach ($providers as $path => $data) {
    $lines[] = '    ' . $json($path) . ': ' . $json($data);
}
echo "{\n  \"providers\": {\n", implode(",\n", $lines), "\n  },\n";
$lines = [];
foreach ($dom as $name => $rows) {
    $lines[] = '    ' . $json($name) . ': ' . $json($rows);
}
echo "  \"dom\": {\n", implode(",\n", $lines), "\n  },\n";
echo "  \"cases\": [\n", implode(",\n", array_map(static fn ($r) => '    ' . $json($r), $results)), "\n  ]\n}\n";
