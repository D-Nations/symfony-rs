<?php
// What libxml (through PHP's DOM extension) answers when a document is
// validated against an XML Schema, for crates/php-dom/tests/data/schema.json
// (scripts/record.sh php-dom-schema).
//
// Every document is loaded as XmlUtils::parse() loads it (validateOnParse,
// LIBXML_NONET | LIBXML_COMPACT, normalizeDocument()), then validated with
// DOMDocument::schemaValidateSource() as XmlUtils::parse() and
// XliffUtils::validateSchema() do. Recorded: the result and every
// libxml_get_errors() entry (level, code, trimmed message, file, line,
// column).
//
// Schemas: the ones Symfony validates against at this pin (XLIFF 1.2
// transitional, 2.0 and 2.2 with xml.xsd, Validator's constraint-mapping,
// the Config and Validator test schemas) and small schemas written for the
// gate, one per feature the inventory lists. Imports are resolved as
// XliffUtils resolves them: its schema source with the xml.xsd location
// rewritten to a file URL; the recording keeps that URL and the imported
// file's bytes so the replay needs no file system.
//
// Documents: upstream's fixtures and the demo's translations for their
// schema, written cases, and mutations of a few fixtures (one change at a
// time: a required attribute removed, an unknown attribute or element
// added, an enumerated value replaced, an id duplicated, text added, two
// children swapped).

require '/demo/vendor/autoload.php';

use Symfony\Component\Translation\Util\XliffUtils;

$up = '/work/upstream/symfony';

// ---------------------------------------------------------------- schemas
$schemas = [];
$imports = [];

$getSchema = (new ReflectionMethod(XliffUtils::class, 'getSchema'))->getClosure(null);
foreach (['1.2' => 'xliff-1.2', '2.0' => 'xliff-2.0', '2.2' => 'xliff-2.2'] as $version => $id) {
    $schemas[$id] = $getSchema($version);
}
// The URL XliffUtils rewrote the import to, and its bytes.
preg_match('#schemaLocation="(file:///[^"]+)"#', $schemas['xliff-2.0'], $m);
$imports[$m[1]] = file_get_contents($up . '/translation/Resources/schemas/xml.xsd');

$schemas['constraint-mapping'] = file_get_contents($up . '/validator/Mapping/Loader/schema/dic/constraint-mapping/constraint-mapping-1.0.xsd');
$schemas['config-util'] = file_get_contents($up . '/config/Tests/Fixtures/Util/schema.xsd');
$schemas['validator-example'] = file_get_contents($up . '/validator/Tests/Constraints/Fixtures/example.xsd');
$schemas['validator-invalid'] = file_get_contents($up . '/validator/Tests/Constraints/Fixtures/invalid.xsd');

$xs = static fn (string $body, string $attrs = '') => '<?xml version="1.0"?>' . "\n"
    . '<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"' . ($attrs ? ' ' . $attrs : '') . ">\n" . $body . "\n</xs:schema>\n";
$tns = 'xmlns="urn:t" targetNamespace="urn:t" elementFormDefault="qualified"';

$cases = [];
$add = static function (string $schema, string $name, string $xml) use (&$cases): void {
    $cases[] = [$schema, $name, $xml];
};

// Feature schemas. Each gets valid and invalid documents.
$schemas['f-sequence'] = $xs(<<<'X'
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="a" type="xs:string"/>
        <xs:element name="b" type="xs:string" minOccurs="0"/>
        <xs:element name="c" type="xs:string" maxOccurs="unbounded"/>
        <xs:element name="d" type="xs:string" minOccurs="0" maxOccurs="unbounded"/>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
X);
$s = 'f-sequence';
$add($s, 'minimal', '<r><a/><c/></r>');
$add($s, 'full', '<r><a>x</a><b/><c/><c/><c/><d/><d/><d/></r>');
$add($s, 'missing first', '<r><c/><c/></r>');
$add($s, 'empty root', '<r/>');
$add($s, 'empty root with text', '<r>  </r>');
$add($s, 'no c', '<r><a/></r>');
$add($s, 'no c after b', '<r><a/><b/></r>');
$add($s, 'many c', '<r><a/><c/><c/><c/><c/></r>');
$add($s, 'unknown after d', '<r><a/><c/><d/><x/></r>');
$add($s, 'unknown element', '<r><a/><x/><c/><c/></r>');
$add($s, 'unknown element at end', '<r><a/><c/><c/><x/></r>');
$add($s, 'wrong order', '<r><b/><a/><c/><c/></r>');
$add($s, 'duplicate a', '<r><a/><a/><c/><c/></r>');
$add($s, 'text in element only', '<r>text<a/><c/><c/></r>');
$add($s, 'text after children', '<r><a/><c/><c/>tail</r>');
$add($s, 'comment and pi allowed', '<r><!-- c --><a/><?p x?><c/><c/></r>');
$add($s, 'cdata in element only', '<r><a/><![CDATA[x]]><c/><c/></r>');
$add($s, 'child element in simple type', '<r><a><z/></a><c/><c/></r>');
$add($s, 'unknown root', '<q/>');
$add($s, 'namespaced root', '<r xmlns="urn:other"><a/><c/><c/></r>');
$add($s, 'namespaced child', '<r><a xmlns="urn:other"/><c/><c/></r>');
$add($s, 'multi line errors', "<r>\n  <a/>\n  <x/>\n  <c/>\n</r>");
$add($s, 'attribute on untyped', '<r q="1"><a/><c/><c/></r>');
$add($s, 'attribute on simple', '<r><a q="1"/><c/><c/></r>');
$add($s, 'xsi type ignored attribute', '<r xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:nil="true"><a/><c/><c/></r>');
$add($s, 'two errors in siblings', '<r><a><z/></a><c><z/></c><c/></r>');

$schemas['f-choice'] = $xs(<<<'X'
  <xs:element name="r">
    <xs:complexType>
      <xs:choice minOccurs="0" maxOccurs="unbounded">
        <xs:element name="a" type="xs:string"/>
        <xs:element name="b" type="xs:string"/>
        <xs:element name="c" type="xs:string" maxOccurs="unbounded"/>
      </xs:choice>
    </xs:complexType>
  </xs:element>
  <xs:element name="s">
    <xs:complexType>
      <xs:choice>
        <xs:element name="a" type="xs:string"/>
        <xs:sequence>
          <xs:element name="b" type="xs:string"/>
          <xs:element name="c" type="xs:string"/>
        </xs:sequence>
      </xs:choice>
    </xs:complexType>
  </xs:element>
  <xs:element name="t">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="a" type="xs:string" minOccurs="0"/>
        <xs:choice minOccurs="1" maxOccurs="unbounded">
          <xs:element name="b" type="xs:string"/>
          <xs:element name="c" type="xs:string"/>
        </xs:choice>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
X);
$s = 'f-choice';
$add($s, 'r empty', '<r/>');
$add($s, 'r mixed', '<r><a/><b/><c/><a/><c/><c/></r>');
$add($s, 'r unknown', '<r><a/><x/></r>');
$add($s, 's a', '<s><a/></s>');
$add($s, 's bc', '<s><b/><c/></s>');
$add($s, 's empty', '<s/>');
$add($s, 's b only', '<s><b/></s>');
$add($s, 's a and b', '<s><a/><b/></s>');
$add($s, 's c first', '<s><c/></s>');
$add($s, 't b', '<t><b/></t>');
$add($s, 't abc', '<t><a/><b/><c/><b/></t>');
$add($s, 't a only', '<t><a/></t>');
$add($s, 't empty', '<t/>');
$add($s, 't a twice', '<t><a/><a/></t>');
$add($s, 't unknown after b', '<t><b/><x/></t>');

$schemas['f-groups'] = $xs(<<<'X'
  <xs:group name="g">
    <xs:sequence>
      <xs:element name="a" type="xs:string"/>
      <xs:element name="b" type="xs:string" minOccurs="0"/>
    </xs:sequence>
  </xs:group>
  <xs:group name="h">
    <xs:choice>
      <xs:element ref="e1"/>
      <xs:element ref="e2"/>
    </xs:choice>
  </xs:group>
  <xs:attributeGroup name="ag">
    <xs:attribute name="x" type="xs:string" use="required"/>
    <xs:attribute name="y" type="xs:positiveInteger"/>
  </xs:attributeGroup>
  <xs:element name="e1" type="xs:string"/>
  <xs:element name="e2">
    <xs:complexType>
      <xs:attributeGroup ref="ag"/>
    </xs:complexType>
  </xs:element>
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:group ref="g" maxOccurs="unbounded"/>
        <xs:group ref="h" minOccurs="0" maxOccurs="unbounded"/>
      </xs:sequence>
      <xs:attributeGroup ref="ag"/>
    </xs:complexType>
  </xs:element>
X);
$s = 'f-groups';
$add($s, 'valid', '<r x="1"><a/><b/><a/><e1/><e2 x="z" y="3"/></r>');
$add($s, 'missing required x', '<r><a/></r>');
$add($s, 'bad y', '<r x="1" y="0"><a/></r>');
$add($s, 'bad y text', '<r x="1" y="abc"><a/></r>');
$add($s, 'three groups', '<r x="1"><a/><a/><a/></r>');
$add($s, 'group then unknown', '<r x="1"><a/><b/><x/></r>');
$add($s, 'e2 missing x', '<r x="1"><a/><e2/></r>');
$add($s, 'unknown attr', '<r x="1" z="2"><a/></r>');
$add($s, 'two unknown attrs', '<r x="1" z="2" w="3"><a/></r>');
$add($s, 'missing and unknown attr', '<r z="2"><a/></r>');
$add($s, 'e1 as root', '<e1>text</e1>');
$add($s, 'e2 as root', '<e2 x="1"/>');
$add($s, 'e2 as root with content', '<e2 x="1">t</e2>');
$add($s, 'e2 as root with child', '<e2 x="1"><a/></e2>');
$add($s, 'b first', '<r x="1"><b/></r>');

$schemas['f-simple'] = $xs(<<<'X'
  <xs:simpleType name="color">
    <xs:restriction base="xs:string">
      <xs:enumeration value="red"/>
      <xs:enumeration value="green"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="tok">
    <xs:restriction base="xs:NMTOKEN">
      <xs:enumeration value="a-b"/>
      <xs:enumeration value="c"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="code">
    <xs:restriction base="xs:string">
      <xs:pattern value="x-[^\s]+"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="mime">
    <xs:restriction base="xs:string">
      <xs:pattern value="(text|image)(/.+)*"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="coord">
    <xs:restriction base="xs:string">
      <xs:pattern value="(-?\d+|#);(-?\d+|#)"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="ud">
    <xs:restriction base="xs:string">
      <xs:pattern value="[^\s:]+:[^\s:]+"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="prio">
    <xs:restriction base="xs:positiveInteger">
      <xs:minInclusive value="1"/>
      <xs:maxInclusive value="10"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:simpleType name="colorOrCode">
    <xs:union memberTypes="color code"/>
  </xs:simpleType>
  <xs:simpleType name="langOrEmpty">
    <xs:union memberTypes="xs:language">
      <xs:simpleType>
        <xs:restriction base="xs:string">
          <xs:enumeration value=""/>
        </xs:restriction>
      </xs:simpleType>
    </xs:union>
  </xs:simpleType>
  <xs:simpleType name="colors">
    <xs:list itemType="color"/>
  </xs:simpleType>
  <xs:simpleType name="codes">
    <xs:list>
      <xs:simpleType>
        <xs:restriction base="xs:NMTOKEN">
          <xs:enumeration value="p"/>
          <xs:enumeration value="q"/>
        </xs:restriction>
      </xs:simpleType>
    </xs:list>
  </xs:simpleType>
  <xs:simpleType name="ncn">
    <xs:restriction base="xs:NCName">
      <xs:enumeration value="default"/>
      <xs:enumeration value="preserve"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="v" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:attribute name="color" type="color"/>
            <xs:attribute name="tok" type="tok"/>
            <xs:attribute name="code" type="code"/>
            <xs:attribute name="mime" type="mime"/>
            <xs:attribute name="coord" type="coord"/>
            <xs:attribute name="ud" type="ud"/>
            <xs:attribute name="prio" type="prio"/>
            <xs:attribute name="cc" type="colorOrCode"/>
            <xs:attribute name="lang" type="langOrEmpty"/>
            <xs:attribute name="colors" type="colors"/>
            <xs:attribute name="codes" type="codes"/>
            <xs:attribute name="ncn" type="ncn"/>
            <xs:attribute name="s" type="xs:string"/>
            <xs:attribute name="nm" type="xs:NMTOKEN"/>
            <xs:attribute name="nms" type="xs:NMTOKENS"/>
            <xs:attribute name="nc" type="xs:NCName"/>
            <xs:attribute name="uri" type="xs:anyURI"/>
            <xs:attribute name="hex" type="xs:hexBinary"/>
            <xs:attribute name="pi" type="xs:positiveInteger"/>
            <xs:attribute name="dt" type="xs:dateTime"/>
            <xs:attribute name="lg" type="xs:language"/>
            <xs:attribute name="untyped"/>
          </xs:complexType>
        </xs:element>
        <xs:element name="e" type="color" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="p" type="prio" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="d" type="xs:dateTime" minOccurs="0" maxOccurs="unbounded"/>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
X);
$s = 'f-simple';
$values = [
    'color' => ['red', 'green', 'blue', ' red', 'red ', '', 'RED'],
    'tok' => ['a-b', 'c', ' c ', 'c d', 'x', ''],
    'code' => ['x-1', 'x-abc', 'x-', 'y-1', 'x-a b', "x-a\tb", ' x-1'],
    'mime' => ['text', 'text/plain', 'image/png/x', 'audio', 'text/', 'textual'],
    'coord' => ['1;2', '-1;#', '#;#', '1;', 'a;b', '1;2;3'],
    'ud' => ['a:b', 'a:', ':b', 'a:b:c', 'a b:c', 'ab'],
    'prio' => ['1', '10', '11', '0', '5', '+5', '05', ' 5 ', '-1', '1.0', 'x', ''],
    'cc' => ['red', 'x-z', 'blue'],
    'lang' => ['en', 'en-US', '', 'x', 'english-language-tag', '1en', 'en_US', 'a-b-c-d'],
    'colors' => ['red', 'red green', '  red   green ', '', 'red blue'],
    'codes' => ['p q', 'p', 'p r', ''],
    'ncn' => ['default', 'preserve', 'other', 'a:b'],
    's' => ['anything', '', ' x '],
    'nm' => ['abc', 'a.b-c_d:e', ' abc ', 'a b', '', '#', "\u{e9}t\u{e9}"],
    'nms' => ['a b', 'a', '', '  a  ', 'a #'],
    'nc' => ['abc', 'a:b', '1a', '_a', '-a', ' abc'],
    'uri' => ['http://x', 'a b', '', '%zz', '#frag', 'urn:x', ':x', 'http://a/[b]'],
    'hex' => ['0a', '0A1f', '', 'abc', 'zz', ' 0a '],
    'pi' => ['1', '0', '99999999999999999999999', '-1', '+1', ' 7 ', '1e3', ''],
    'dt' => ['2008-01-01T00:00:00', '2008-01-01T00:00:00Z', '2008-01-01T00:00:00+02:00', '2008-01-01T00:00:00.123', '2008-01-01', '2008-13-01T00:00:00', '2008-02-30T00:00:00', '2008-01-01T24:00:00', '-0001-01-01T00:00:00', '2008-01-01 00:00:00', ' 2008-01-01T00:00:00 ', '2008-01-01T00:00:60', '20080-01-01T00:00:00', '2008-01-01T00:00:00+14:01'],
    'lg' => ['en', 'en-US', '', 'x-klingon', 'abcdefghi', 'en--US'],
    'untyped' => ['anything'],
];
foreach ($values as $attr => $list) {
    foreach ($list as $i => $v) {
        $add($s, "attr $attr #$i", '<r><v ' . $attr . '="' . htmlspecialchars($v, ENT_XML1 | ENT_QUOTES) . '"/></r>');
    }
}
foreach (['red', ' red ', 'blue', ''] as $i => $v) {
    $add($s, "element color #$i", "<r><e>$v</e></r>");
}
foreach (['3', ' 3 ', '0', 'x', ''] as $i => $v) {
    $add($s, "element prio #$i", "<r><p>$v</p></r>");
}
foreach (['2008-01-01T00:00:00', 'x', ''] as $i => $v) {
    $add($s, "element dateTime #$i", "<r><d>$v</d></r>");
}
$add($s, 'element color with child', '<r><e><x/></e></r>');
$add($s, 'element color with comment', '<r><e>re<!-- c -->d</e></r>');
$add($s, 'element color cdata', '<r><e><![CDATA[red]]></e></r>');
$add($s, 'element color entity', '<r><e>&#114;ed</e></r>');
$add($s, 'two bad attributes', '<r><v color="x" prio="99"/></r>');

$schemas['f-attributes'] = $xs(<<<'X'
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="a" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:attribute name="req" use="required"/>
            <xs:attribute name="opt" use="optional" type="xs:NMTOKEN"/>
            <xs:attribute name="def" default="d" type="xs:string"/>
            <xs:attribute name="fix" fixed="f" type="xs:string"/>
            <xs:attribute ref="xml:lang"/>
            <xs:anyAttribute namespace="##other" processContents="lax"/>
          </xs:complexType>
        </xs:element>
        <xs:element name="b" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:anyAttribute namespace="##any" processContents="skip"/>
          </xs:complexType>
        </xs:element>
        <xs:element name="c" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:attribute name="x" type="xs:string"/>
            <xs:anyAttribute namespace="##other" processContents="strict"/>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
X, 'xmlns:xml="http://www.w3.org/XML/1998/namespace"');
// The xml namespace's attributes come from the imported xml.xsd.
$schemas['f-attributes'] = str_replace(
    '<xs:element name="r">',
    '<xs:import namespace="http://www.w3.org/XML/1998/namespace" schemaLocation="' . array_key_first($imports) . '"/>' . "\n  <xs:element name=\"r\">",
    $schemas['f-attributes'],
);
$s = 'f-attributes';
$add($s, 'required only', '<r><a req="1"/></r>');
$add($s, 'missing required', '<r><a/></r>');
$add($s, 'missing required twice', '<r><a/><a opt="x"/></r>');
$add($s, 'fixed right', '<r><a req="" fix="f"/></r>');
$add($s, 'fixed wrong', '<r><a req="" fix="g"/></r>');
$add($s, 'default given', '<r><a req="" def="z"/></r>');
$add($s, 'opt invalid', '<r><a req="" opt="a b"/></r>');
$add($s, 'xml lang', '<r><a req="" xml:lang="en"/></r>');
$add($s, 'xml lang empty', '<r><a req="" xml:lang=""/></r>');
$add($s, 'xml lang invalid', '<r><a req="" xml:lang="1-english"/></r>');
$add($s, 'xml space not declared', '<r><a req="" xml:space="preserve"/></r>');
$add($s, 'other ns lax', '<r xmlns:o="urn:o"><a req="" o:x="1"/></r>');
$add($s, 'unqualified unknown', '<r><a req="" zz="1"/></r>');
$add($s, 'any skip', '<r xmlns:o="urn:o"><b x="1" o:y="2" xml:lang="??"/></r>');
$add($s, 'other strict undeclared', '<r xmlns:o="urn:o"><c o:y="2"/></r>');
$add($s, 'other strict unqualified', '<r><c y="2"/></r>');
$add($s, 'xmlns declarations', '<r xmlns:o="urn:o"><a req="" xmlns:p="urn:p"/></r>');
$add($s, 'attribute order of errors', '<r><a zz="1" opt="a b" fix="g"/></r>');

$schemas['f-content'] = $xs(<<<'X'
  <xs:complexType name="ext">
    <xs:simpleContent>
      <xs:extension base="xs:string">
        <xs:attribute name="k" type="xs:string" use="required"/>
      </xs:extension>
    </xs:simpleContent>
  </xs:complexType>
  <xs:simpleType name="color">
    <xs:restriction base="xs:string">
      <xs:enumeration value="red"/>
    </xs:restriction>
  </xs:simpleType>
  <xs:complexType name="ext2">
    <xs:simpleContent>
      <xs:extension base="color">
        <xs:attribute name="k" type="xs:string"/>
      </xs:extension>
    </xs:simpleContent>
  </xs:complexType>
  <xs:complexType name="m" mixed="true">
    <xs:sequence>
      <xs:element name="i" type="xs:string" minOccurs="0" maxOccurs="unbounded"/>
    </xs:sequence>
  </xs:complexType>
  <xs:complexType name="emptyType"/>
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="s" type="ext" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="s2" type="ext2" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="m" type="m" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="e" type="emptyType" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="any" minOccurs="0" maxOccurs="unbounded"/>
        <xs:element name="w" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:sequence>
              <xs:any namespace="##other" processContents="lax" minOccurs="0" maxOccurs="unbounded"/>
              <xs:element name="n" type="xs:string" minOccurs="0"/>
              <xs:any namespace="##any" processContents="skip" minOccurs="0"/>
            </xs:sequence>
          </xs:complexType>
        </xs:element>
        <xs:element name="ws" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:sequence>
              <xs:any namespace="##other" processContents="strict" minOccurs="0" maxOccurs="unbounded"/>
            </xs:sequence>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
X, $tns);
$s = 'f-content';
$add($s, 'simple content', '<r xmlns="urn:t"><s k="1">text</s></r>');
$add($s, 'simple content missing attr', '<r xmlns="urn:t"><s>text</s></r>');
$add($s, 'simple content child', '<r xmlns="urn:t"><s k="1"><i/></s></r>');
$add($s, 'simple content enum', '<r xmlns="urn:t"><s2>red</s2><s2 k="">blue</s2></r>');
$add($s, 'mixed', '<r xmlns="urn:t"><m>a<i>b</i>c<i/>d</m></r>');
$add($s, 'mixed unknown child', '<r xmlns="urn:t"><m>a<j/></m></r>');
$add($s, 'empty type with text', '<r xmlns="urn:t"><e>x</e></r>');
$add($s, 'empty type with space', '<r xmlns="urn:t"><e> </e></r>');
$add($s, 'empty type with child', '<r xmlns="urn:t"><e><i/></e></r>');
$add($s, 'empty type with attr', '<r xmlns="urn:t"><e a="1"/></r>');
$add($s, 'anyType element', '<r xmlns="urn:t"><any a="1">t<x><y/></x></any></r>');
$add($s, 'any other lax', '<r xmlns="urn:t" xmlns:o="urn:o"><w><o:x><o:y/></o:x><n/></w></r>');
$add($s, 'any other lax with local', '<r xmlns="urn:t"><w><x xmlns=""/></w></r>');
$add($s, 'any other with target ns', '<r xmlns="urn:t"><w><zz/></w></r>');
$add($s, 'any skip after n', '<r xmlns="urn:t"><w><n/><zz/></w></r>');
$add($s, 'any strict undeclared', '<r xmlns="urn:t" xmlns:o="urn:o"><ws><o:x/></ws></r>');
$add($s, 'unqualified child', '<r xmlns="urn:t"><s xmlns="" k="1"/></r>');
$add($s, 'unqualified root', '<r/>');

$schemas['f-identity'] = $xs(<<<'X'
  <xs:element name="r">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="g" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:sequence>
              <xs:element name="u" minOccurs="0" maxOccurs="unbounded">
                <xs:complexType>
                  <xs:attribute name="id" type="xs:string"/>
                  <xs:attribute name="ref" type="xs:string"/>
                </xs:complexType>
              </xs:element>
            </xs:sequence>
            <xs:attribute name="id" type="xs:string"/>
            <xs:attribute name="name" type="xs:string"/>
          </xs:complexType>
        </xs:element>
        <xs:element name="t" minOccurs="0" maxOccurs="unbounded">
          <xs:complexType>
            <xs:attribute name="xid" type="xs:ID"/>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
    <xs:unique name="U_g">
      <xs:selector xpath=".//t:g"/>
      <xs:field xpath="@name"/>
    </xs:unique>
    <xs:key name="K_u">
      <xs:selector xpath=".//t:u|.//t:g"/>
      <xs:field xpath="@id"/>
    </xs:key>
    <xs:keyref name="KR_u" refer="t:K_u">
      <xs:selector xpath="./t:g/t:u"/>
      <xs:field xpath="@ref"/>
    </xs:keyref>
  </xs:element>
X, 'xmlns:t="urn:t" targetNamespace="urn:t" elementFormDefault="qualified"');
$s = 'f-identity';
$add($s, 'valid', '<r xmlns="urn:t"><g id="1" name="a"><u id="2" ref="1"/></g><g id="3"/></r>');
$add($s, 'unique duplicate', '<r xmlns="urn:t"><g id="1" name="a"/><g id="2" name="a"/></r>');
$add($s, 'unique absent twice', '<r xmlns="urn:t"><g id="1"/><g id="2"/></r>');
$add($s, 'key duplicate', '<r xmlns="urn:t"><g id="1"><u id="1"/></g></r>');
$add($s, 'key missing', '<r xmlns="urn:t"><g id="1"><u/></g></r>');
$add($s, 'keyref missing target', '<r xmlns="urn:t"><g id="1"><u id="2" ref="9"/></g></r>');
$add($s, 'keyref ok forward', '<r xmlns="urn:t"><g id="1"><u id="2" ref="3"/></g><g id="3"/></r>');
$add($s, 'id duplicate', '<r xmlns="urn:t"><t xid="a"/><t xid="a"/></r>');
$add($s, 'id invalid', '<r xmlns="urn:t"><t xid="1a"/></r>');
$add($s, 'id ok', '<r xmlns="urn:t"><t xid="a"/><t xid="b"/></r>');
$add($s, 'multi line identity', "<r xmlns=\"urn:t\">\n<g id=\"1\" name=\"a\"/>\n<g id=\"1\" name=\"a\"/>\n</r>");

// Content models, one per shape the validator supports, to pin which
// elements libxml lists as expected after each prefix of children.
$E = static fn (string $n, $min = 1, $max = 1) => "<xs:element name=\"$n\" minOccurs=\"$min\" maxOccurs=\"$max\"/>";
$C = static fn ($min, $max, string ...$p) => "<xs:choice minOccurs=\"$min\" maxOccurs=\"$max\">" . implode('', $p) . '</xs:choice>';
$S = static fn ($min, $max, string ...$p) => "<xs:sequence minOccurs=\"$min\" maxOccurs=\"$max\">" . implode('', $p) . '</xs:sequence>';
$W = '<xs:any namespace="##other" processContents="lax" minOccurs="0" maxOccurs="unbounded"/>';
$models = [];
foreach ([[0, 'unbounded'], [1, 'unbounded'], [1, 1], [0, 1]] as [$mn, $mx]) {
    $models[] = $C($mn, $mx, $E('a'), $E('b'));
    $models[] = $S(1, 1, $E('p', 0), $C($mn, $mx, $E('a'), $E('b')), $E('z', 0));
    $models[] = $S($mn, $mx, $E('a'), $E('b', 0));
    $models[] = $S(1, 1, $E('a', $mn, $mx), $E('b', 0));
    $models[] = $C($mn, $mx, $E('a', 0, 'unbounded'), $E('b', 0, 'unbounded'));
    $models[] = $C($mn, $mx, $E('a', 1, 'unbounded'), $E('b'));
    $models[] = $C($mn, $mx, $E('a', 0, 1), $E('b', 0, 1));
    $models[] = $S(1, 1, $E('p', 0), $C($mn, $mx, $E('a', 0, 'unbounded'), $E('b')), $E('z', 0));
    $models[] = $S(1, 1, $E('p', 0), $S($mn, $mx, $E('a'), $E('b', 0)), $E('z', 0));
}
$models[] = $S(1, 1, $W, $E('a', 0), $C(1, 'unbounded', $E('b'), $E('z')));
$models[] = $C(0, 1, $S(1, 1, $E('a'), $E('b', 0)), $E('z'));
$models[] = $S(0, 'unbounded', $E('a', 0, 'unbounded'), $E('b', 0));
$models[] = $S(1, 1, $E('a', 0, 'unbounded'), $E('b', 0, 'unbounded'));
foreach ($models as $i => $m) {
    $schemas["m-$i"] = $xs('<xs:element name="r"><xs:complexType>' . $m . '</xs:complexType></xs:element>');
    foreach (['<r/>', '<r><x/></r>', '<r><a/></r>', '<r><a/><x/></r>', '<r><a/><a/><x/></r>', '<r><b/><x/></r>', '<r><a/><b/><x/></r>', '<r><p/><x/></r>', '<r><p/><a/><x/></r>', '<r><b/><a/></r>', '<r><a/><b/><a/><x/></r>', '<r><z/><x/></r>'] as $doc) {
        $add("m-$i", $doc, $doc);
    }
}

// What libxml does after an error, local type names, facets and
// wildcards in lists, identity constraints on invalid values.
$schemas['p-misc'] = $xs(
    '<xs:element name="r"><xs:complexType><xs:sequence><xs:element name="a" maxOccurs="unbounded"><xs:complexType><xs:attribute name="n" type="xs:NMTOKEN"/></xs:complexType></xs:element><xs:element name="b" minOccurs="0"/></xs:sequence></xs:complexType></xs:element>'
    . '<xs:element name="e"><xs:complexType/></xs:element>'
    . '<xs:element name="s" type="xs:NMTOKEN"/>'
    . '<xs:element name="l"><xs:simpleType><xs:restriction base="xs:positiveInteger"><xs:minInclusive value="3"/></xs:restriction></xs:simpleType></xs:element>'
    . '<xs:element name="u"><xs:simpleType><xs:restriction base="xs:NMTOKEN"/></xs:simpleType></xs:element>'
    . '<xs:element name="li"><xs:simpleType><xs:list itemType="xs:NCName"/></xs:simpleType></xs:element>'
    . '<xs:element name="w"><xs:complexType><xs:sequence><xs:any namespace="##any" processContents="lax" minOccurs="0"/></xs:sequence></xs:complexType></xs:element>'
    . '<xs:element name="w2"><xs:complexType><xs:sequence><xs:any namespace="##local urn:x" processContents="lax" minOccurs="0"/></xs:sequence></xs:complexType></xs:element>'
    . '<xs:element name="w3"><xs:complexType><xs:sequence><xs:any namespace="##targetNamespace" processContents="lax" minOccurs="0"/></xs:sequence></xs:complexType></xs:element>'
    . '<xs:element name="w4"><xs:complexType><xs:sequence><xs:any namespace="##other" processContents="lax" minOccurs="0"/></xs:sequence></xs:complexType></xs:element>'
    . '<xs:element name="m"><xs:complexType mixed="true"/></xs:element>'
    . '<xs:element name="k"><xs:complexType><xs:sequence><xs:element name="k" minOccurs="0" maxOccurs="unbounded"><xs:complexType><xs:attribute name="id" type="xs:NMTOKEN"/></xs:complexType></xs:element></xs:sequence></xs:complexType>'
    . '<xs:key name="K"><xs:selector xpath="k"/><xs:field xpath="@id"/></xs:key><xs:unique name="U"><xs:selector xpath="k"/><xs:field xpath="@id"/></xs:unique></xs:element>'
);
foreach ([
    'after unexpected, bad attribute' => '<r><x/><a n="a b"/></r>',
    'inside unexpected' => '<r><x><a n="a b"/></x><a/></r>',
    'two unexpected' => '<r><a/><x/><b/><zz/></r>',
    'three texts' => '<r>t1<a/>t2<a/>t3</r>',
    'text split by comment' => "<r>t1\n<a/>\n<!--c-->t2</r>",
    'empty, two children' => '<e><a/><b/></e>',
    'empty, text child text' => '<e>x<a/>y</e>',
    'simple, two children' => '<s><a/><b/></s>',
    'simple, text child text' => '<s>x<a/>y z</s>',
    'minInclusive' => '<l>2</l>',
    'local atomic lexical' => '<l>x</l>',
    'local atomic derived' => '<u>a b</u>',
    'local list' => '<li>a 1</li>',
    'any ##any twice' => '<w><q/><q/></w>',
    'any list' => '<w2><q xmlns="urn:y"/></w2>',
    'any ##targetNamespace absent' => '<w3><q/></w3>',
    'any ##other without target namespace' => '<w4><q/></w4>',
    'lax without declaration' => '<w><a n="a b"/></w>',
    'lax with global declaration' => '<w><s>a b</s></w>',
    'mixed empty with child' => '<m>text<q/></m>',
    'xsi:type' => '<r xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="xs:string"><a/></r>',
    'other xsi attributes' => '<r xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="x.xsd" xsi:bogus="1"><a/></r>',
    'errors on two siblings' => '<r><a/><a n="x" n2="1"/><b q="1"/></r>',
    'key, normalized duplicate' => '<k><k id=" a "/><k id="a"/></k>',
    'key, invalid values' => '<k><k id="a b"/><k id="a b"/></k>',
    'key, three' => '<k><k id="a"/><k id="a"/><k id="a"/></k>',
] as $name => $doc) {
    $add('p-misc', $name, $doc);
}

// Schemas that are themselves wrong, and features outside the inventory.
$schemas['bad-unknown-type'] = $xs('<xs:element name="r" type="nope"/>');
$add('bad-unknown-type', 'any document', '<r/>');
$schemas['bad-unknown-ref'] = $xs('<xs:element name="r"><xs:complexType><xs:sequence><xs:element ref="nope"/></xs:sequence></xs:complexType></xs:element>');
$add('bad-unknown-ref', 'any document', '<r/>');
$schemas['bad-not-schema'] = '<?xml version="1.0"?><notaschema/>';
$add('bad-not-schema', 'any document', '<r/>');
$schemas['bad-not-wellformed'] = '<?xml version="1.0"?><xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:element name="r">';
$add('bad-not-wellformed', 'any document', '<r/>');
$schemas['bad-duplicate'] = $xs('<xs:element name="r"/><xs:element name="r"/>');
$add('bad-duplicate', 'any document', '<r/>');
$schemas['bad-unknown-element'] = $xs('<xs:element name="r"/><xs:bogus/>');
$add('bad-unknown-element', 'any document', '<r/>');
$schemas['unsupported-complexcontent'] = $xs(<<<'X'
  <xs:complexType name="b"><xs:sequence><xs:element name="a"/></xs:sequence></xs:complexType>
  <xs:element name="r"><xs:complexType><xs:complexContent><xs:extension base="b"/></xs:complexContent></xs:complexType></xs:element>
X);
$add('unsupported-complexcontent', 'valid', '<r><a/></r>');
$schemas['unsupported-all'] = $xs('<xs:element name="r"><xs:complexType><xs:all><xs:element name="a"/><xs:element name="b"/></xs:all></xs:complexType></xs:element>');
$add('unsupported-all', 'valid', '<r><b/><a/></r>');
$schemas['unsupported-counts'] = $xs('<xs:element name="r"><xs:complexType><xs:sequence><xs:element name="c" minOccurs="2" maxOccurs="3"/></xs:sequence></xs:complexType></xs:element>');
$add('unsupported-counts', 'too few', '<r><c/></r>');
$schemas['empty-schema'] = $xs('');
$add('empty-schema', 'any document', '<r/>');

// --------------------------------------------------------------- fixtures
$files = static function (string $dir, string $pattern): array {
    $out = [];
    foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($dir, FilesystemIterator::SKIP_DOTS)) as $file) {
        if (preg_match($pattern, $file->getFilename())) {
            $out[] = $file->getPathname();
        }
    }
    sort($out);

    return $out;
};
$xliffSchema = static function (string $xml): ?string {
    $dom = new DOMDocument();
    if ('' === $xml || !@$dom->loadXML($xml)) {
        return null;
    }
    try {
        $v = XliffUtils::getVersionNumber($dom);
    } catch (Throwable) {
        return null;
    }

    return ['1.2' => 'xliff-1.2', '2.0' => 'xliff-2.0', '2.1' => 'xliff-2.0', '2.2' => 'xliff-2.2'][$v] ?? null;
};
foreach ([...$files($up . '/translation/Tests/Fixtures', '/\.(xlf|xliff)$/'), ...$files('/demo/translations', '/\.xlf$/')] as $file) {
    $xml = file_get_contents($file);
    if ($id = $xliffSchema($xml)) {
        $add($id, 'file ' . preg_replace('#^/work/upstream/|^/#', '', $file), $xml);
    }
}
foreach ($files($up . '/validator/Tests/Mapping/Loader', '/^constraint-mapping.*\.xml$|^withdoctype\.xml$/') as $file) {
    $add('constraint-mapping', 'file ' . preg_replace('#^/work/upstream/#', '', $file), file_get_contents($file));
}
foreach ($files($up . '/config/Tests/Fixtures/Util', '/\.xml$/') as $file) {
    $add('config-util', 'file ' . preg_replace('#^/work/upstream/#', '', $file), file_get_contents($file));
}
foreach (['<book><title>T</title><author>A</author></book>', '<book><title>T</title></book>', '<book><author>A</author><title>T</title></book>', '<library/>'] as $i => $xml) {
    $add('validator-example', "XmlValidatorTest-like #$i", $xml);
    $add('validator-invalid', "XmlValidatorTest-like #$i", $xml);
}

// -------------------------------------------------------------- mutations
$mutate = static function (string $schema, string $label, string $xml) use ($add): void {
    $base = new DOMDocument();
    $base->loadXML($xml);
    $seen = [];
    $elements = [];
    foreach ($base->getElementsByTagName('*') as $el) {
        if (!isset($seen[$el->localName])) {
            $seen[$el->localName] = true;
            $elements[] = $el->getNodePath();
        }
    }
    $apply = static function (string $path, callable $change) use ($xml): ?string {
        $dom = new DOMDocument();
        $dom->loadXML($xml);
        $el = (new DOMXPath($dom))->query($path)->item(0);
        if (!$el instanceof DOMElement || false === $change($dom, $el)) {
            return null;
        }

        return $dom->saveXML();
    };
    foreach ($elements as $path) {
        $tag = (new DOMXPath($base))->query($path)->item(0)->localName;
        $variants = [
            'unknown attribute' => static fn ($d, $e) => $e->setAttribute('bogus', '1'),
            'unknown child' => static fn ($d, $e) => $e->appendChild($d->createElementNS($e->namespaceURI, 'bogus')),
            'unknown first child' => static fn ($d, $e) => $e->insertBefore($d->createElementNS($e->namespaceURI, 'bogus'), $e->firstChild),
            'text added' => static fn ($d, $e) => $e->insertBefore($d->createTextNode('stray'), $e->firstChild),
            'children removed' => static function ($d, $e) {
                if (!$e->firstChild) {
                    return false;
                }
                while ($e->firstChild) {
                    $e->removeChild($e->firstChild);
                }
            },
            'first two element children swapped' => static function ($d, $e) {
                $kids = [];
                foreach ($e->childNodes as $c) {
                    if ($c instanceof DOMElement) {
                        $kids[] = $c;
                    }
                }
                if (\count($kids) < 2 || $kids[0]->localName === $kids[1]->localName) {
                    return false;
                }
                $e->insertBefore($kids[1], $kids[0]);
            },
            'first element child duplicated' => static function ($d, $e) {
                foreach ($e->childNodes as $c) {
                    if ($c instanceof DOMElement) {
                        $e->insertBefore($c->cloneNode(true), $c);

                        return null;
                    }
                }

                return false;
            },
        ];
        foreach ($e = (new DOMXPath($base))->query($path)->item(0)->attributes as $attr) {
            $name = $attr->nodeName;
            $variants["attribute $name removed"] = static fn ($d, $el) => $el->removeAttribute($name);
            $variants["attribute $name set to zzz zzz"] = static fn ($d, $el) => $el->setAttribute($name, 'zzz zzz');
            $variants["attribute $name emptied"] = static fn ($d, $el) => $el->setAttribute($name, '');
        }
        foreach ($variants as $what => $change) {
            if (null !== $out = $apply($path, $change)) {
                $add($schema, "mutation $label $tag: $what", $out);
            }
        }
    }
};
$mutate('xliff-1.2', 'resources.xlf', file_get_contents($up . '/translation/Tests/Fixtures/resources.xlf'));
$mutate('xliff-1.2', 'resources-tool-info.xlf', file_get_contents($up . '/translation/Tests/Fixtures/resources-tool-info.xlf'));
$mutate('xliff-2.0', 'resources-2.0.xlf', file_get_contents($up . '/translation/Tests/Fixtures/resources-2.0.xlf'));
$mutate('xliff-2.0', 'resources-notes-meta.xlf', file_get_contents($up . '/translation/Tests/Fixtures/resources-notes-meta.xlf'));
$mutate('xliff-2.2', 'resources-2.2.xlf', file_get_contents($up . '/translation/Tests/Fixtures/resources-2.2.xlf'));
$mutate('constraint-mapping', 'constraint-mapping.xml', file_get_contents($up . '/validator/Tests/Mapping/Loader/constraint-mapping.xml'));

// XLIFF identity constraints and xml:* attributes, written.
$h12 = '<?xml version="1.0" encoding="utf-8"?>' . "\n" . '<xliff version="1.2" xmlns="urn:oasis:names:tc:xliff:document:1.2">' . "\n";
$f12 = static fn (string $body, string $fileAttrs = 'source-language="en" datatype="plaintext" original="file.ext"') => "  <file $fileAttrs>\n    <body>\n$body\n    </body>\n  </file>\n</xliff>\n";
$tu = static fn (string $id, string $extra = '') => "      <trans-unit id=\"$id\"$extra><source>s$id</source><target>t$id</target></trans-unit>";
$add('xliff-1.2', 'written duplicate trans-unit id', $h12 . $f12($tu('1') . "\n" . $tu('1')));
$add('xliff-1.2', 'written trans-unit without id', $h12 . $f12('      <trans-unit><source>s</source></trans-unit>'));
$add('xliff-1.2', 'written group ids', $h12 . $f12('      <group id="g"><trans-unit id="1"><source>s</source></trans-unit></group><group id="g"><trans-unit id="2"><source>s</source></trans-unit></group>'));
$add('xliff-1.2', 'written xml:space', $h12 . $f12($tu('1', ' xml:space="preserve"')));
$add('xliff-1.2', 'written bad xml:space', $h12 . $f12($tu('1', ' xml:space="keep"')));
$add('xliff-1.2', 'written xml:lang on target', $h12 . $f12('      <trans-unit id="1"><source>s</source><target xml:lang="fr">t</target></trans-unit>'));
$add('xliff-1.2', 'written bad xml:lang', $h12 . $f12('      <trans-unit id="1"><source>s</source><target xml:lang="not a lang">t</target></trans-unit>'));
$add('xliff-1.2', 'written state values', $h12 . $f12('      <trans-unit id="1"><source>s</source><target state="needs-translation">t</target></trans-unit>' . "\n" . '      <trans-unit id="2"><source>s</source><target state="x-custom">t</target></trans-unit>' . "\n" . '      <trans-unit id="3"><source>s</source><target state="bogus">t</target></trans-unit>'));
$add('xliff-1.2', 'written date', $h12 . $f12($tu('1'), 'source-language="en" datatype="plaintext" original="f" date="2008-01-01T00:00:00Z"'));
$add('xliff-1.2', 'written bad date', $h12 . $f12($tu('1'), 'source-language="en" datatype="plaintext" original="f" date="yesterday"'));
$add('xliff-1.2', 'written datatype x-', $h12 . $f12($tu('1'), 'source-language="en" datatype="x-mine" original="f"'));
$add('xliff-1.2', 'written bad datatype', $h12 . $f12($tu('1'), 'source-language="en" datatype="mine" original="f"'));
$add('xliff-1.2', 'written no file', $h12 . '</xliff>');
$add('xliff-1.2', 'written no namespace', '<xliff version="1.2"><file source-language="en" datatype="plaintext" original="f"><body/></file></xliff>');
$add('xliff-1.2', 'written inline', $h12 . $f12('      <trans-unit id="1"><source>a <g id="1">b</g> <x id="2"/> <bpt id="3">[</bpt>c<ept id="3">]</ept> <ph id="4">ph</ph> <mrk mtype="term">m</mrk></source></trans-unit>'));
$add('xliff-1.2', 'written note and context', $h12 . $f12('      <trans-unit id="1"><source>s</source><note priority="2" from="me">n</note><context-group purpose="location"><context context-type="sourcefile">f.php</context></context-group></trans-unit>'));
$add('xliff-1.2', 'written bad note priority', $h12 . $f12('      <trans-unit id="1"><source>s</source><note priority="11">n</note></trans-unit>'));
$add('xliff-1.2', 'written header tool', $h12 . '  <file source-language="en" datatype="plaintext" original="f"><header><tool tool-id="t" tool-name="T"/><tool tool-id="t" tool-name="U"/></header><body/></file></xliff>');
$add('xliff-1.2', 'written alt-trans', $h12 . $f12('      <trans-unit id="1"><source>s</source><alt-trans match-quality="80"><target>t</target></alt-trans></trans-unit>'));
$add('xliff-1.2', 'written foreign element', $h12 . $f12('      <trans-unit id="1" xmlns:o="urn:o"><source>s</source><o:x/></trans-unit>'));
$add('xliff-1.2', 'written foreign attribute', $h12 . $f12('      <trans-unit id="1" xmlns:o="urn:o" o:a="1"><source>s</source></trans-unit>'));
$h20 = '<?xml version="1.0" encoding="utf-8"?>' . "\n" . '<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0" srcLang="en">' . "\n";
$add('xliff-2.0', 'written minimal', $h20 . '  <file id="f"><unit id="u"><segment><source>s</source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written no unit', $h20 . '  <file id="f"></file>' . "\n</xliff>");
$add('xliff-2.0', 'written bad id', $h20 . '  <file id="a b"><unit id="u"><segment><source>s</source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written bad state', $h20 . '  <file id="f"><unit id="u"><segment state="done"><source>s</source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written priority', $h20 . '  <file id="f"><notes><note priority="11">n</note><note priority="3">n</note></notes><unit id="u"><segment><source>s</source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written inline', $h20 . '  <file id="f"><unit id="u"><originalData><data id="d">x</data></originalData><segment><source>a<ph id="1" dataRef="d"/><pc id="2">b<cp hex="0001"/></pc><mrk id="m" type="term">c</mrk><mrk id="n" type="my:type">d</mrk><mrk id="o" type="bad">e</mrk><sc id="3"/><ec startRef="3"/><sm id="4"/><em startRef="4"/></source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written bad hex', $h20 . '  <file id="f"><unit id="u"><segment><source><cp hex="xyz"/></source></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written missing srcLang', '<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0"><file id="f"><unit id="u"><segment><source>s</source></segment></unit></file></xliff>');
$add('xliff-2.0', 'written ignorable and group', $h20 . '  <file id="f"><group id="g" type="a:b"><unit id="u"><ignorable><source> </source></ignorable><segment><source>s</source><target order="1">t</target></segment></unit></group></file>' . "\n</xliff>");
$add('xliff-2.0', 'written bad group type', $h20 . '  <file id="f"><group id="g" type="ab"><unit id="u"><segment><source>s</source></segment></unit></group></file>' . "\n</xliff>");
$add('xliff-2.0', 'written target order zero', $h20 . '  <file id="f"><unit id="u"><segment><source>s</source><target order="0">t</target></segment></unit></file>' . "\n</xliff>");
$add('xliff-2.0', 'written data space', $h20 . '  <file id="f"><unit id="u"><originalData><data id="d" xml:space="default">x</data></originalData><segment><source>s</source></segment></unit></file>' . "\n</xliff>");

// ------------------------------------------------------------- recording
function validate(string $xml, string $schema): array
{
    libxml_use_internal_errors(true);
    libxml_clear_errors();
    $dom = new DOMDocument();
    $dom->validateOnParse = true;
    if ('' === $xml || !$dom->loadXML($xml, LIBXML_NONET | LIBXML_COMPACT)) {
        libxml_clear_errors();

        return ['load' => false];
    }
    $dom->normalizeDocument();
    libxml_clear_errors();
    $ok = @$dom->schemaValidateSource($schema);
    $errors = [];
    foreach (libxml_get_errors() as $error) {
        $errors[] = [$error->level, $error->code, trim($error->message), $error->file, $error->line, $error->column];
    }
    libxml_clear_errors();

    return ['ok' => $ok, 'errors' => $errors];
}

$rows = [];
foreach ($cases as [$schema, $name, $xml]) {
    $r = validate($xml, $schemas[$schema]);
    if (isset($r['load'])) {
        continue; // not well-formed for XmlUtils: X1's corpus covers it
    }
    $input = mb_check_encoding($xml, 'UTF-8') ? ['xml' => $xml] : ['xml64' => base64_encode($xml)];
    $rows[] = json_encode(['schema' => $schema, 'name' => $name] + $input + $r, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR);
}

$flags = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR;
echo '{"libxml":', json_encode(LIBXML_DOTTED_VERSION), ',"php":', json_encode(PHP_VERSION), ",
";
echo '"schemas":', json_encode($schemas, $flags), ",
";
echo '"imports":', json_encode($imports, $flags), ",
";
echo "\"cases\":[
", implode(",
", $rows), "
]}
";
