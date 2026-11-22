<?php
// What libxml (through PHP's DOM extension) answers for a corpus of valid
// and malformed XML, for crates/php-dom/tests/data/parse.json
// (scripts/record.sh php-dom). Each case is loaded three ways:
//
//   "utils"  Symfony's XmlUtils::parse(): the DOMDocument settings it uses
//            (validateOnParse, LIBXML_NONET | LIBXML_COMPACT), the errors
//            libxml collects, the tree after normalizeDocument(), and what
//            XmlUtils::parse() itself returns or throws;
//   "plain"  new DOMDocument() and loadXML($xml) with no flags (what
//            XmlUtilsTest's convertDomElementToArray cases do).
//
// Inputs travel as base64. Trees are nested arrays (see dump() below).
// The corpus is the hand-written cases below, then upstream's XML test
// fixtures (Config, Translation, Validator) and the demo's translations.

require '/demo/vendor/autoload.php';

use Symfony\Component\Config\Util\XmlUtils;

$cases = [];
$add = static function (string $name, string $xml) use (&$cases): void {
    $cases[] = [$name, $xml];
};

$decl = '<?xml version="1.0" encoding="UTF-8"?>';

// Well-formed documents.
$add('empty element', '<a/>');
$add('empty element with space', '<a />');
$add('start and end', '<a></a>');
$add('declaration', $decl . '<a/>');
$add('declaration and newline', $decl . "\n<a/>\n");
$add('declaration without encoding', '<?xml version="1.0"?><a/>');
$add('declaration standalone yes', '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a/>');
$add('declaration standalone no', '<?xml version="1.0" standalone="no"?><a/>');
$add('declaration single quotes', "<?xml version='1.0' encoding='utf-8'?><a/>");
$add('declaration version 1.1', '<?xml version="1.1"?><a/>');
$add('declaration version 2.0', '<?xml version="2.0"?><a/>');
$add('text', '<a>hello</a>');
$add('nested', '<a><b><c>x</c></b><d/></a>');
$add('attributes', '<a x="1" y=\'2\' z="three"/>');
$add('attribute spaces around equals', '<a x = "1"/>');
$add('attribute normalization', "<a x=\"t\tn\nr\r\nend\"/>");
$add('attribute char refs', '<a x="&#10;&#9;&#13;&#x20;"/>');
$add('attribute predefined entities', '<a x="&lt;&gt;&amp;&apos;&quot;"/>');
$add('attribute gt', '<a x="1>2"/>');
$add('text predefined entities', '<a>&lt;&gt;&amp;&apos;&quot;</a>');
$add('char refs', '<a>&#65;&#x42;&#x1F600;&#169;</a>');
$add('mixed content', '<a>one<b>two</b>three<c/>four</a>');
$add('whitespace text', "<a>\n  <b/>\n  <c> </c>\n</a>");
$add('crlf', "<a>\r\nx\ry\r\n</a>\r\n");
$add('cdata', '<a><![CDATA[<x> & y]]></a>');
$add('cdata empty', '<a><![CDATA[]]></a>');
$add('cdata and text', '<a>pre<![CDATA[mid]]>post</a>');
$add('cdata brackets', '<a><![CDATA[a]b]]c]]]></a>');
$add('comment', '<a><!-- note --></a>');
$add('comment before root', '<!-- top --><a/>');
$add('comment after root', '<a/><!-- end -->');
$add('pi', '<a><?target some data?></a>');
$add('pi before root', '<?xml-stylesheet href="x.xsl"?><a/>');
$add('pi no data', '<a><?t?></a>');
$add('trailing whitespace', "<a/>\n\n  \t");
$add('leading whitespace no decl', "  \n<a/>");
$add('utf-8 text', "<a>caf\u{e9} \u{65e5}\u{672c} \u{1F600}</a>");
$add('utf-8 name', "<caf\u{e9} \u{e9}t\u{e9}=\"1\"/>");
$add('utf-8 bom', "\xEF\xBB\xBF<a>x</a>");
$add('utf-8 bom and declaration', "\xEF\xBB\xBF" . $decl . '<a>x</a>');
$add('greater-than in text', '<a>1 > 0</a>');
$add('single bracket in text', '<a>]] ></a>');
$add('name chars', '<_a-b.c:d xmlns:_a-b.c="u"/>');
$add('colon-free names', '<a.b-c_d/>');
$add('long text', '<a>' . str_repeat('0123456789', 2000) . '</a>');
$add('many attributes', '<a ' . implode(' ', array_map(static fn ($i) => "a$i=\"$i\"", range(1, 50))) . '/>');
$add('depth 100', str_repeat('<d>', 100) . str_repeat('</d>', 100));
$add('depth 255', str_repeat('<d>', 255) . str_repeat('</d>', 255));
$add('depth 256', str_repeat('<d>', 256) . str_repeat('</d>', 256));
$add('depth 257', str_repeat('<d>', 257) . str_repeat('</d>', 257));
$add('depth 300', str_repeat('<d>', 300) . str_repeat('</d>', 300));
$add('multiline positions', "<a>\n<b>\n<c>text</c>\n</b>\n</a>");

// Namespaces.
$add('default namespace', '<a xmlns="urn:x"><b/></a>');
$add('prefixed namespace', '<p:a xmlns:p="urn:p"><p:b/><c/></p:a>');
$add('prefixed attribute', '<a xmlns:p="urn:p" p:x="1" x="2"/>');
$add('redeclared prefix', '<p:a xmlns:p="urn:1"><p:b xmlns:p="urn:2"/></p:a>');
$add('undeclare default', '<a xmlns="urn:x"><b xmlns=""/></a>');
$add('xml prefix', '<a xml:lang="en" xml:space="preserve"/>');
$add('xml prefix declared', '<a xmlns:xml="http://www.w3.org/XML/1998/namespace"/>');
$add('xml prefix misdeclared', '<a xmlns:xml="urn:x"/>');
$add('xmlns prefix declared', '<a xmlns:xmlns="urn:x"/>');
$add('unbound element prefix', '<p:a/>');
$add('unbound nested prefix', '<a><p:b/></a>');
$add('unbound attribute prefix', '<a p:x="1"/>');
$add('empty prefix declaration', '<a xmlns:p=""/>');
$add('relative namespace uri', '<a xmlns="relative"/>');
$add('invalid namespace uri', '<a xmlns="a b"/>');
$add('same attribute two prefixes', '<a xmlns:p="urn:x" xmlns:q="urn:x" p:y="1" q:y="2"/>');
$add('qname with two colons', '<a:b:c xmlns:a="u"/>');
$add('qname leading colon', '<:a/>');
$add('qname trailing colon', '<a:/>');
$add('xliff 1.2', $decl . '<xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2"><file source-language="en" datatype="plaintext" original="file.ext"><body><trans-unit id="1"><source>foo</source><target>bar</target></trans-unit></body></file></xliff>');
$add('xliff 2.0', $decl . '<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0" srcLang="en-US" trgLang="ja-JP"><file id="f1"><unit id="1"><segment><source>Hello</source><target>こんにちは</target></segment></unit></file></xliff>');
$add('xmlutils prefixes', '<foo:a xmlns:foo="http://www.example.org/foo" xmlns:h="http://www.example.org/bar" attr="1" h:bar="bar"><foo:b>hello</foo:b><h:c>2</h:c></foo:a>');

// Encodings.
$add('iso-8859-1', "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a x=\"\xE9\">caf\xE9 \xFF</a>");
$add('latin1 alias', "<?xml version=\"1.0\" encoding=\"latin1\"?><a>\xE9</a>");
$add('iso-8859-1 lowercase', "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><a>\xE9</a>");
$add('us-ascii', '<?xml version="1.0" encoding="US-ASCII"?><a>x</a>');
$add('us-ascii high byte', "<?xml version=\"1.0\" encoding=\"US-ASCII\"?><a>\xE9</a>");
$add('ascii alias', '<?xml version="1.0" encoding="ASCII"?><a>x</a>');
$add('windows-1252', "<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>\x80</a>");
$add('unknown encoding', '<?xml version="1.0" encoding="no-such-charset"?><a/>');
$add('utf-16 declared without bom', '<?xml version="1.0" encoding="UTF-16"?><a/>');
$utf16 = static fn (string $s, bool $le) => mb_convert_encoding($s, $le ? 'UTF-16LE' : 'UTF-16BE', 'UTF-8');
$add('utf-16le bom', "\xFF\xFE" . $utf16('<?xml version="1.0" encoding="UTF-16"?><a>caf' . "\u{e9}" . '</a>', true));
$add('utf-16be bom', "\xFE\xFF" . $utf16('<?xml version="1.0" encoding="UTF-16"?><a>caf' . "\u{e9}" . '</a>', false));
$add('utf-16le bom no declaration', "\xFF\xFE" . $utf16('<a>x</a>', true));
$add('utf-16be bom surrogate pair', "\xFE\xFF" . $utf16("<a>\u{1F600}</a>", false));
$add('utf-16le no bom', $utf16('<?xml version="1.0" encoding="UTF-16LE"?><a/>', true));
$add('utf-8 declared utf-16 bom', "\xFF\xFE" . $utf16('<?xml version="1.0" encoding="UTF-8"?><a/>', true));
$add('invalid utf-8', "<a>\xC3\x28</a>");
$add('invalid utf-8 lead', "<a>\xFF</a>");
$add('truncated utf-8', "<a>\xE2\x82</a>");
$add('overlong utf-8', "<a>\xC0\xAF</a>");
$add('utf-8 surrogate', "<a>\xED\xA0\x80</a>");
$add('invalid utf-8 in name', "<a\xFF/>");
$add('invalid utf-8 in attribute', "<a x=\"\xFF\"/>");

// Malformed documents.
$add('empty', '');
$add('whitespace only', "  \n ");
$add('text only', 'hello');
$add('declaration only', $decl);
$add('unclosed', '<a>');
$add('unclosed nested', '<a><b>');
$add('unclosed with text', '<a>text');
$add('mismatched end tag', '<a></b>');
$add('mismatched nested', '<a><b></a></b>');
$add('end tag only', '</a>');
$add('extra content', '<a/><b/>');
$add('extra text', '<a/>text');
$add('extra content after newline', "<a/>\n<b/>");
$add('unquoted attribute', '<a x=1/>');
$add('attribute without value', '<a x/>');
$add('attribute without space', '<a x="1"y="2"/>');
$add('duplicate attribute', '<a x="1" x="2"/>');
$add('lt in attribute', '<a x="<"/>');
$add('unterminated attribute', '<a x="1/>');
$add('bare ampersand', '<a>&</a>');
$add('ampersand space', '<a>a & b</a>');
$add('undefined entity', '<a>&foo;</a>');
$add('undefined entity in attribute', '<a x="&foo;"/>');
$add('entity without semicolon', '<a>&amp</a>');
$add('char ref zero', '<a>&#0;</a>');
$add('char ref control', '<a>&#1;</a>');
$add('char ref surrogate', '<a>&#xD800;</a>');
$add('char ref too big', '<a>&#x110000;</a>');
$add('char ref bad digits', '<a>&#xZZ;</a>');
$add('control char', "<a>\x01</a>");
$add('control char in attribute', "<a x=\"\x07\"/>");
$add('nul byte', "<a>\x00</a>");
$add('cdata end in text', '<a>]]></a>');
$add('unterminated cdata', '<a><![CDATA[x</a>');
$add('cdata outside root', '<![CDATA[x]]><a/>');
$add('double hyphen in comment', '<a><!-- a -- b --></a>');
$add('comment ending with hyphen', '<a><!-- a ---></a>');
$add('unterminated comment', '<a><!-- x</a>');
$add('pi target xml', '<a><?xml version="1.0"?></a>');
$add('pi target XmL', '<a><?XmL x?></a>');
$add('pi without target', '<a><? x?></a>');
$add('unterminated pi', '<a><?t x</a>');
$add('declaration after whitespace', ' ' . $decl . '<a/>');
$add('declaration after comment', '<!-- c -->' . $decl . '<a/>');
$add('declaration missing version', '<?xml encoding="UTF-8"?><a/>');
$add('declaration bad standalone', '<?xml version="1.0" standalone="maybe"?><a/>');
$add('declaration unterminated', '<?xml version="1.0"<a/>');
$add('name starting with digit', '<1a/>');
$add('name starting with hyphen', '<-a/>');
$add('invalid name char', '<a!/>');
$add('space before name', '< a/>');
$add('end tag with space', '<a></a >');
$add('end tag with attribute', '<a></a x="1">');
$add('missing gt', '<a</a>');
$add('lt in text', '<a>1 < 2</a>');
$add('two roots with text', '<a>x</a><b>y</b>');
$add('depth 10000', str_repeat('<d>', 10000) . str_repeat('</d>', 10000));
$add('long name', '<' . str_repeat('n', 60000) . '/>');
$add('attribute value 20 MB', '<a x="' . str_repeat('v', 20_000_000) . '"/>');

// Document types and entities.
$add('doctype', '<!DOCTYPE a><a/>');
$add('doctype system', '<!DOCTYPE a SYSTEM "a.dtd"><a/>');
$add('doctype public', '<!DOCTYPE a PUBLIC "-//X//DTD X//EN" "http://example.com/a.dtd"><a/>');
$add('doctype internal subset', '<!DOCTYPE a [<!ELEMENT a (#PCDATA)>]><a>x</a>');
$add('doctype invalid content', '<!DOCTYPE a [<!ELEMENT a EMPTY>]><a>x</a>');
$add('doctype default attribute', '<!DOCTYPE a [<!ATTLIST a x CDATA "dflt">]><a/>');
$add('internal entity', '<!DOCTYPE a [<!ENTITY e "value">]><a>&e;</a>');
$add('internal entity markup', '<!DOCTYPE a [<!ENTITY e "<b>in</b>">]><a>&e;</a>');
$add('internal entity in attribute', '<!DOCTYPE a [<!ENTITY e "v">]><a x="&e;"/>');
$add('external entity file', '<!DOCTYPE a [<!ENTITY e SYSTEM "file:///etc/debian_version">]><a>&e;</a>');
$add('external entity http', '<!DOCTYPE a [<!ENTITY e SYSTEM "http://127.0.0.1:9/x">]><a>&e;</a>');
$add('external entity php filter', '<?xml version="1.0" encoding="UTF-8"?>' . "\n" . '<!DOCTYPE scan [<!ENTITY test SYSTEM "php://filter/read=convert.base64-encode/resource=/etc/debian_version">]>' . "\n" . '<scan>&test;</scan>');
$add('external dtd file', '<!DOCTYPE a SYSTEM "file:///etc/debian_version"><a/>');
$add('external dtd http', '<!DOCTYPE a SYSTEM "http://127.0.0.1:9/a.dtd"><a/>');
$add('parameter entity', '<!DOCTYPE a [<!ENTITY % p "<!ENTITY e \'v\'>"> %p;]><a>&e;</a>');
$add('external parameter entity', '<!DOCTYPE a [<!ENTITY % p SYSTEM "file:///etc/debian_version"> %p;]><a/>');
$billion = '<!DOCTYPE lolz [<!ENTITY lol "lol">';
for ($i = 1; $i <= 9; ++$i) {
    $billion .= sprintf('<!ENTITY lol%d "%s">', $i, str_repeat($i > 1 ? '&lol' . ($i - 1) . ';' : '&lol;', 10));
}
$add('billion laughs', $billion . ']><lolz>&lol9;</lolz>');
$add('billion laughs attribute', $billion . ']><lolz x="&lol9;"/>');
$add('quadratic blowup', '<!DOCTYPE a [<!ENTITY e "' . str_repeat('x', 10000) . '">]><a>' . str_repeat('&e;', 1000) . '</a>');
$add('recursive entity', '<!DOCTYPE a [<!ENTITY e "&e;">]><a>&e;</a>');
$add('mutually recursive entities', '<!DOCTYPE a [<!ENTITY e "&f;"><!ENTITY f "&e;">]><a>&e;</a>');
$add('doctype after root', '<a/><!DOCTYPE a>');
$add('two doctypes', '<!DOCTYPE a><!DOCTYPE a><a/>');
$add('unterminated doctype', '<!DOCTYPE a [<!ELEMENT a ANY>');
$add('malformed internal subset', '<!DOCTYPE a [<!ELEMENT>]><a/>');

// Probes for recovery after an error, positions and line counting.
$add('lone cr lines', "<a>\r<b/>\r<c/></a>");
$add('crlf lines', "<a>\r\n<b/>\r\n<c/></a>");
$add('control char then mismatch', "<a>\x01<b></a>");
$add('nested start tag failure', '<r><a x=1/></r>');
$add('nested start tag failure then mismatch', '<r><a x=1/><b></r>');
$add('cdata eof', '<a><![CDATA[');
$add('cdata eof one char', '<a><![CDATA[x');
$add('cdata eof two chars', '<a><![CDATA[xy');
$add('pi name xmlfoo', '<?xmlfoo x?><a/>');
$add('pi name with colon', '<?a:b x?><a/>');
$add('pi xml-model', '<?xml-model href="x"?><a/>');
$add('pi missing space', '<a><?t"x"?></a>');
$add('text around comment', '<a>text<!-- c -->more</a>');
$add('char refs between text', '<a>&#65;&#66;x&amp;y</a>');
$add('attribute order with namespaces', '<a x="1" xmlns="urn:x" xmlns:p="urn:p" p:y="2" z="3"/>');
$add('escaped markup text', '<a>&lt;b&gt;</a>');
$add('prefixed child attributes', '<a xmlns:p="urn:p"><p:b p:c="1" c="2"/></a>');
$add('entity between text', '<!DOCTYPE a [<!ENTITY e "v">]><a>x&e;y</a>');
$add('declaration extra spaces', '<?xml  version = "1.0"  encoding = "UTF-8"  ?><a/>');
$add('declaration space before end', '<?xml version="1.0" encoding="UTF-8" ?><a/>');
$add('declaration no space before encoding', '<?xml version="1.0"encoding="UTF-8"?><a/>');
$add('declaration empty encoding', '<?xml version="1.0" encoding=""?><a/>');
$add('declaration mismatched quotes', '<?xml version="1.0" encoding="UTF-8\'?><a/>');
$add('declaration wrong order', '<?xml version="1.0" standalone="yes" encoding="UTF-8"?><a/>');
$add('declaration version unquoted', '<?xml version=1.0?><a/>');
$add('declaration uppercase', '<?XML version="1.0"?><a/>');
$add('declaration encoding utf8', '<?xml version="1.0" encoding="UTF8"?><a/>');
$add('declaration encoding iso-8859-15', "<?xml version=\"1.0\" encoding=\"ISO-8859-15\"?><a>\xA4</a>");
$add('utf-16le declared no bom', mb_convert_encoding('<?xml version="1.0" encoding="utf-16le"?><a>x</a>', 'UTF-16LE', 'UTF-8'));
$add('utf-16be no bom', mb_convert_encoding('<?xml version="1.0" encoding="UTF-16BE"?><a>x</a>', 'UTF-16BE', 'UTF-8'));
$add('utf-16le bom lone surrogate', "\xFF\xFE" . mb_convert_encoding('<a>', 'UTF-16LE', 'UTF-8') . "\x00\xD8" . mb_convert_encoding('</a>', 'UTF-16LE', 'UTF-8'));
$add('windows-1252 undefined byte', "<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>x\x81y</a>");
$add('misc after root', "<a></a><!-- c --> <?p?> \n");
$add('misc before root', "<?xml version=\"1.0\"?>\n<!-- c -->\n<?pi x?>\n<a/>\n<!-- after -->\n");
$add('brackets in text', '<a>a]b]]c]</a>');
$add('triple bracket end', '<a>]]]></a>');
$add('quotes in attributes', '<a b=\'"\' c="\'"/>');
$add('non-name char in name', "<a\u{d7}/>");
$add('noncharacter fffe', "<a>\u{fffe}</a>");
$add('noncharacter ffff', "<a>\u{ffff}</a>");
$add('max code point', "<a>\u{10ffff}</a>");
$add('char ref lt in attribute', '<a x="&#60;"/>');
$add('char ref missing semicolon', '<a>&#x41</a>');
$add('char ref empty', '<a>&#;</a>');
$add('char ref empty hex', '<a>&#x;</a>');
$add('char ref bad decimal', '<a>&#1a;</a>');
$add('char ref huge', '<a>&#99999999999999999999;</a>');
$add('char ref in attribute bad', '<a x="&#0;"/>');
$add('slash space gt', '<a/ >');
$add('attribute slash space gt', '<a b="1"/ >');
$add('eof in end tag', '<a></a');
$add('eof in nested end tag', '<a><b></b');
$add('eof in start tag', '<a');
$add('eof in attribute', '<a b="1"');
$add('eof after equals', '<a b=');
$add('eof in attribute value', '<a b="1');
$add('eof in comment', '<!-- unterminated');
$add('comment open only', '<!--');
$add('comment empty gt', '<!-->');
$add('comment hyphen gt', '<!--->');
$add('comment in content gt', '<a><!--></a>');
$add('comment ok then root', '<!----><a/>');
$add('bang garbage', '<a><!x></a>');
$add('doctype eof', '<!DOCTYPE');
$add('doctype name eof', '<!DOCTYPE a');
$add('doctype empty subset', '<!DOCTYPE a []><a/>');
$add('doctype missing space', '<!DOCTYPEa><a/>');
$add('doctype comment and pi in subset', '<!DOCTYPE a [<!-- c --><?p x?>]><a/>');
$add('doctype notation', '<!DOCTYPE a [<!NOTATION n SYSTEM "x">]><a/>');
$add('doctype entity with ref', '<!DOCTYPE a [<!ENTITY e "x"><!ENTITY f "&e;&e;">]><a>&f;</a>');
$add('doctype entity used in attribute twice', '<!DOCTYPE a [<!ENTITY e "x&#38;y">]><a b="&e;&e;"/>');
$add('doctype undefined entity', '<!DOCTYPE a [<!ELEMENT a ANY>]><a>&nope;</a>');
$add('doctype entity lt in attribute', '<!DOCTYPE a [<!ENTITY e "&#60;">]><a b="&e;"/>');
$add('duplicate namespace declaration', '<a xmlns:p="urn:p" xmlns:p="urn:q"/>');
$add('duplicate default namespace', '<a xmlns="u" xmlns="v"/>');
$add('xml prefix attribute unknown', '<a xml:x="1"/>');
$add('xmlns namespace bound', '<a xmlns:a="http://www.w3.org/2000/xmlns/"/>');
$add('default namespace xml uri', '<a xmlns="http://www.w3.org/XML/1998/namespace"/>');
$add('mismatched prefix', '<p:a xmlns:p="urn:p" xmlns:q="urn:p"></q:a>');
$add('matched prefix', '<p:a xmlns:p="urn:p"></p:a>');
$add('case mismatch', '<a></A>');
$add('unbound prefix in end tag', '<p:a></p:a>');
$add('tab and newline in tag', "<a\n\tb=\"1\"\n/>");
$add('newline in end tag', "<a></a\n>");
$add('deep namespaces', '<a xmlns="u1"><b xmlns:p="u2"><p:c xmlns="u3"><d p:e="1"/></p:c></b></a>');
$add('text after nested close', '<a><b>x</b>y<c/>z</a>');
$add('only comment', '<!-- c -->');
$add('only pi', '<?p x?>');
$add('only doctype', '<!DOCTYPE a>');
$add('leading text then root', 'x<a/>');
$add('unicode whitespace before root', "\u{a0}<a/>");
$add('tab before root', "\t<a/>");

// Probes for the declaration, the DTD and entity paths.
$add('declaration missing equals', '<?xml version "1.0"?><a/>');
$add('declaration encoding missing equals', '<?xml version="1.0" encoding "UTF-8"?><a/>');
$add('declaration standalone missing equals', '<?xml version="1.0" standalone "yes"?><a/>');
$add('declaration version letters', '<?xml version="abc"?><a/>');
$add('declaration gt only', '<?xml version="1.0"><a/>');
$add('nul first', "\x00<a/>");
$add('crlf before non-ascii', "<a>x\r\n\u{e9}<b/></a>");
$add('lf before non-ascii', "<a>x\n\u{e9}<b/></a>");
$add('adjacent cdata', '<a><![CDATA[x]]><![CDATA[y]]></a>');
$add('cdata then text then cdata', '<a><![CDATA[x]]>t<![CDATA[y]]></a>');
$add('comment before doctype', '<!-- c --><!DOCTYPE a><a/>');
$add('pi before doctype', '<?p x?><!DOCTYPE a><a/>');
$add('comment non-ascii double hyphen', "<a><!-- \u{e9} -- x --></a>");
$add('comment non-ascii unterminated', "<a><!-- \u{e9} x</a>");
$add('comment starting with double hyphen', '<a><!----x--></a>');
$add('comment crlf', "<a><!-- x\r\ny --></a>");
$add('comment lone cr', "<a><!-- x\ry --></a>");
$add('comment invalid char', "<a><!-- x\x01 --></a>");
$add('pi non-ascii data', "<a><?t \u{e9}?></a>");
$add('pi invalid char', "<a><?t x\x01?></a>");
$add('cdata invalid char', "<a><![CDATA[x\x01y]]></a>");
$add('cdata crlf', "<a><![CDATA[x\r\ny\rz]]></a>");
$add('xml space invalid', '<a xml:space="maybe"/>');
$add('xml space preserve', '<a xml:space="preserve"> </a>');
$add('truncated utf-8 near end', "<a>x\xFF");
$add('invalid utf-8 three bytes left', "<a>\xFF</");
$add('invalid utf-8 then more', "<a>\xFF\xFFabc</a>");
$add('four-byte above max', "<a>\xF4\x90\x80\x80</a>");
$add('attribute non-ascii', "<a x=\"caf\u{e9}\" y='\u{65e5}'/>");
$add('attribute non-ascii noncharacter', "<a x=\"\u{fffe}\"/>");
$add('attribute crlf', "<a x=\"1\r\n2\"/>");
$add('name non-ascii start', "<\u{e9}t\u{e9} \u{e0}=\"1\"/>");
$add('end tag non-ascii', "<\u{e9}></\u{e9}>");
$add('end tag non-ascii mismatch', "<\u{e9}a></\u{e9}b>");
$add('end tag prefix partial', '<ab></ac>');
$add('empty end tag', '<a></>');
$add('entity empty', '<!DOCTYPE a [<!ENTITY e "">]><a>&e;</a>');
$add('entity unbalanced start', '<!DOCTYPE a [<!ENTITY e "<b>">]><a>&e;</a>');
$add('entity unbalanced end', '<!DOCTYPE a [<!ENTITY e "</b>">]><a>&e;</a>');
$add('entity bare ampersand', '<!DOCTYPE a [<!ENTITY e "x&#38;y">]><a>&e;</a>');
$add('entity undefined inside', '<!DOCTYPE a [<!ENTITY e "&nope;">]><a>&e;</a>');
$add('entity used twice', '<!DOCTYPE a [<!ENTITY e "v">]><a>&e;&e;</a>');
$add('entity with element', '<!DOCTYPE a [<!ENTITY e "<b x=\'1\'>t</b>">]><a>&e;</a>');
$add('entity in attribute undefined inside', '<!DOCTYPE a [<!ENTITY e "&nope;">]><a b="&e;"/>');
$add('entity recursive in attribute', '<!DOCTYPE a [<!ENTITY e "&e;">]><a b="&e;"/>');
$add('entity external in attribute', '<!DOCTYPE a [<!ENTITY e SYSTEM "x.txt">]><a b="&e;"/>');
$add('entity unparsed', '<!DOCTYPE a [<!NOTATION n SYSTEM "n"><!ENTITY e SYSTEM "x" NDATA n>]><a>&e;</a>');
$add('entity predefined redeclared', '<!DOCTYPE a [<!ENTITY lt "&#38;#60;">]><a>&lt;</a>');
$add('entity declared twice', '<!DOCTYPE a [<!ENTITY e "1"><!ENTITY e "2">]><a b="&e;"/>');
$add('entity value percent', '<!DOCTYPE a [<!ENTITY e "100%">]><a/>');
$add('entity value pe reference', '<!DOCTYPE a [<!ENTITY % p "x"><!ENTITY e "%p;">]><a/>');
$add('entity value unterminated', '<!DOCTYPE a [<!ENTITY e "x>]><a/>');
$add('entity not terminated', '<!DOCTYPE a [<!ENTITY e "x" junk>]><a/>');
$add('entity no space after keyword', '<!DOCTYPE a [<!ENTITYe "x">]><a/>');
$add('entity no name', '<!DOCTYPE a [<!ENTITY "x">]><a/>');
$add('entity no space after name', '<!DOCTYPE a [<!ENTITY e"x">]><a/>');
$add('entity no value', '<!DOCTYPE a [<!ENTITY e>]><a/>');
$add('entity colon name', '<!DOCTYPE a [<!ENTITY a:b "x">]><a/>');
$add('entity system missing literal', '<!DOCTYPE a [<!ENTITY e SYSTEM>]><a/>');
$add('entity public', '<!DOCTYPE a [<!ENTITY e PUBLIC "-//X//EN" "x.txt">]><a/>');
$add('entity fragment', '<!DOCTYPE a [<!ENTITY e SYSTEM "x.txt#f">]><a/>');
$add('parameter entity undefined', '<!DOCTYPE a [%p;]><a/>');
$add('parameter entity no name', '<!DOCTYPE a [% ;]><a/>');
$add('parameter entity no semicolon', '<!DOCTYPE a [<!ENTITY % p "<!ELEMENT a ANY>"> %p ]><a/>');
$add('parameter entity element', '<!DOCTYPE a [<!ENTITY % p "<!ELEMENT a ANY>"> %p;]><a/>');
$add('parameter entity then undefined general', '<!DOCTYPE a [<!ENTITY % p ""> %p;]><a>&nope;</a>');
$add('element decl ok', '<!DOCTYPE a [<!ELEMENT a (b, (c | d)*, e?)+><!ELEMENT b EMPTY>]><a/>');
$add('element decl mixed', '<!DOCTYPE a [<!ELEMENT a (#PCDATA | b | c)*>]><a/>');
$add('element decl mixed without star', '<!DOCTYPE a [<!ELEMENT a (#PCDATA | b)>]><a/>');
$add('element decl mixed no name', '<!DOCTYPE a [<!ELEMENT a (#PCDATA | )*>]><a/>');
$add('element decl bad keyword', '<!DOCTYPE a [<!ELEMENT a FOO>]><a/>');
$add('element decl mixed separators', '<!DOCTYPE a [<!ELEMENT a (b, c | d)>]><a/>');
$add('element decl unfinished', '<!DOCTYPE a [<!ELEMENT a (b c)>]><a/>');
$add('element decl missing gt', '<!DOCTYPE a [<!ELEMENT a ANY]><a/>');
$add('element decl no name', '<!DOCTYPE a [<!ELEMENT (b)>]><a/>');
$add('element decl pe in content', '<!DOCTYPE a [<!ENTITY % p "x"><!ELEMENT a %p;>]><a/>');
$add('element decl no content', '<!DOCTYPE a [<!ELEMENT a ( )>]><a/>');
$add('attlist ok', '<!DOCTYPE a [<!ATTLIST a x CDATA #IMPLIED y ID #REQUIRED z (p|q) "p" w CDATA #FIXED "f">]><a y="1"/>');
$add('attlist normalized value', '<!DOCTYPE a [<!ATTLIST a x NMTOKENS #IMPLIED>]><a x="  p   q  "/>');
$add('attlist normalized char ref', '<!DOCTYPE a [<!ATTLIST a x NMTOKENS #IMPLIED>]><a x=" p&#32;&#32;q "/>');
$add('attlist entities quirk', '<!DOCTYPE a [<!ATTLIST a x ENTITIES #IMPLIED>]><a/>');
$add('attlist no element name', '<!DOCTYPE a [<!ATTLIST>]><a/>');
$add('attlist no space', '<!DOCTYPE a [<!ATTLISTa x CDATA #IMPLIED>]><a/>');
$add('attlist no type', '<!DOCTYPE a [<!ATTLIST a x #IMPLIED>]><a/>');
$add('attlist bad enumeration', '<!DOCTYPE a [<!ATTLIST a x (p q) #IMPLIED>]><a/>');
$add('attlist empty enumeration', '<!DOCTYPE a [<!ATTLIST a x () #IMPLIED>]><a/>');
$add('attlist notation', '<!DOCTYPE a [<!NOTATION n SYSTEM "n"><!ATTLIST a x NOTATION (n) #IMPLIED>]><a/>');
$add('attlist notation no paren', '<!DOCTYPE a [<!ATTLIST a x NOTATION n #IMPLIED>]><a/>');
$add('attlist bad default', '<!DOCTYPE a [<!ATTLIST a x CDATA dflt>]><a/>');
$add('attlist fixed no space', '<!DOCTYPE a [<!ATTLIST a x CDATA #FIXED"f">]><a/>');
$add('attlist no space after default', '<!DOCTYPE a [<!ATTLIST a x CDATA "d"y CDATA "e">]><a/>');
$add('notation public only', '<!DOCTYPE a [<!NOTATION n PUBLIC "-//X//EN">]><a/>');
$add('notation no name', '<!DOCTYPE a [<!NOTATION >]><a/>');
$add('notation no id', '<!DOCTYPE a [<!NOTATION n>]><a/>');
$add('notation not finished', '<!DOCTYPE a [<!NOTATION n SYSTEM "n" x>]><a/>');
$add('doctype system no literal', '<!DOCTYPE a SYSTEM><a/>');
$add('doctype public bad char', '<!DOCTYPE a PUBLIC "a{b}" "x"><a/>');
$add('doctype public no system', '<!DOCTYPE a PUBLIC "-//X//EN"><a/>');
$add('doctype unfinished literal', '<!DOCTYPE a SYSTEM "x');
$add('doctype garbage', '<!DOCTYPE a junk><a/>');
$add('doctype subset garbage', '<!DOCTYPE a [ junk ]><a/>');
$add('doctype subset no close', '<!DOCTYPE a [<!ELEMENT a ANY> <a/>');
$add('doctype standalone undefined entity', '<?xml version="1.0" standalone="yes"?><!DOCTYPE a [<!ENTITY % p ""> %p;]><a>&nope;</a>');
$add('doctype comment hyphen', '<!DOCTYPE a [<!-- a -- b -->]><a/>');
$add('doctype pi xml', '<!DOCTYPE a [<?xml x?>]><a/>');

// Upstream's XML fixtures and the demo's translations.
$files = [];
foreach (['/work/upstream/symfony/config/Tests/Fixtures', '/work/upstream/symfony/translation/Tests/Fixtures', '/work/upstream/symfony/validator/Tests', '/demo/translations'] as $dir) {
    $it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($dir, FilesystemIterator::SKIP_DOTS));
    foreach ($it as $file) {
        if (preg_match('/\.(xml|xlf|xliff|ts)$/', $file->getFilename()) && $file->getSize() > 0 && $file->getSize() < 200_000) {
            $files[] = $file->getPathname();
        }
    }
}
sort($files);
foreach ($files as $file) {
    $add('file ' . preg_replace('#^/work/upstream/|^/#', '', $file), file_get_contents($file));
}

// The tree in document order, one row per node, each starting with the
// node's depth (the document is 0), so deep documents stay flat.
function dump(DOMNode $node, int $depth = 0, array &$out = []): array
{
    $row = match ($node->nodeType) {
        XML_DOCUMENT_NODE => ['Doc', $node->encoding, $node->xmlEncoding, $node->xmlVersion, $node->xmlStandalone],
        XML_ELEMENT_NODE => ['E', $node->nodeName, $node->localName, $node->prefix, $node->namespaceURI, $node->getLineNo(), attrs($node)],
        XML_TEXT_NODE => ['T', $node->nodeValue],
        XML_CDATA_SECTION_NODE => ['C', $node->nodeValue],
        XML_COMMENT_NODE => ['#', $node->nodeValue],
        XML_PI_NODE => ['P', $node->target, $node->data],
        XML_ENTITY_REF_NODE => ['R', $node->nodeName],
        XML_DOCUMENT_TYPE_NODE => ['D', $node->name, $node->publicId, $node->systemId, $node->internalSubset],
        default => ['?', $node->nodeType, $node->nodeName],
    };
    $out[] = [$depth, ...$row];
    if (XML_ENTITY_REF_NODE !== $node->nodeType && XML_DOCUMENT_TYPE_NODE !== $node->nodeType) {
        foreach ($node->childNodes as $child) {
            dump($child, $depth + 1, $out);
        }
    }

    return $out;
}

function attrs(DOMElement $node): array
{
    $attrs = [];
    foreach ($node->attributes as $attr) {
        $attrs[] = [$attr->nodeName, $attr->localName, $attr->prefix, $attr->namespaceURI, $attr->value];
    }

    return $attrs;
}

function load(string $xml, bool $utils): array
{
    libxml_use_internal_errors(true);
    libxml_clear_errors();
    $dom = new DOMDocument();
    if ($utils) {
        $dom->validateOnParse = true;
    }
    try {
        $ok = $utils ? $dom->loadXML($xml, LIBXML_NONET | LIBXML_COMPACT) : $dom->loadXML($xml);
    } catch (Throwable $e) {
        return ['throws' => [get_class($e), $e->getMessage()]];
    }
    $errors = [];
    foreach (libxml_get_errors() as $error) {
        $errors[] = [$error->level, $error->code, trim($error->message), $error->line, $error->column];
    }
    libxml_clear_errors();
    $out = ['ok' => $ok, 'errors' => $errors];
    if ($ok) {
        if ($utils) {
            $dom->normalizeDocument();
        }
        $out['tree'] = dump($dom);
    }

    return $out;
}

$out = [];
foreach ($cases as [$name, $xml]) {
    $utils = load($xml, true);
    $plain = load($xml, false);
    // The plain tree is kept only where it differs from XmlUtils's.
    if (isset($plain['tree'], $utils['tree']) && $plain['tree'] === $utils['tree']) {
        $plain['tree'] = 'same';
    }
    try {
        XmlUtils::parse($xml);
        $parse = ['ok'];
    } catch (Throwable $e) {
        $parse = [get_class($e), $e->getMessage()];
    }
    libxml_clear_errors();
    // Large inputs are kept by their recipe, not their bytes.
    $big = strlen($xml) > 1_000_000;
    $out[] = [
        'name' => $name,
        'input' => $big ? null : base64_encode($xml),
        'sha256' => hash('sha256', $xml),
        'utils' => $big ? array_diff_key($utils, ['tree' => 1]) : $utils,
        'plain' => $big ? array_diff_key($plain, ['tree' => 1]) : $plain,
        'parse' => $parse,
    ];
}

// One case per line, so that a re-recording diffs by case.
$flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_THROW_ON_ERROR;
echo '{"libxml":', json_encode(LIBXML_DOTTED_VERSION), ",\"cases\":[\n";
echo implode(",\n", array_map(static fn ($case) => json_encode($case, $flags), $out));
echo "\n]}\n";
