<?php
// symfony/string's English, French and Spanish inflectors on a word corpus
// built here (scripts/record.sh string), for
// crates/string/tests/inflector_vectors.rs: every word from the three
// upstream tests' providers and from the inflectors' own tables (read by
// reflection), each with case variants, prefixes, a trailing newline and
// non-ASCII letters, then every inflector's singularize() and pluralize()
// of every word. Strings are base64: the corpus holds bytes that are not
// UTF-8.

require '/demo/vendor/autoload.php';

use Symfony\Component\String\Inflector\EnglishInflector;
use Symfony\Component\String\Inflector\FrenchInflector;
use Symfony\Component\String\Inflector\SpanishInflector;

$tests = '/work/upstream/symfony/string/Tests/Inflector';
$seeds = [];
foreach (['EnglishInflectorTest', 'FrenchInflectorTest', 'SpanishInflectorTest'] as $class) {
    require_once "$tests/$class.php";
    $reflection = new ReflectionClass('Symfony\\Component\\String\\Tests\\Inflector\\'.$class);
    foreach ($reflection->getMethods() as $method) {
        if ($method->isStatic() && $method->getDeclaringClass()->getName() === $reflection->getName() && str_ends_with($method->getName(), 'Provider')) {
            foreach ($method->invoke(null) as $row) {
                array_walk_recursive($row, static function ($word) use (&$seeds) {
                    $seeds[] = $word;
                });
            }
        }
    }
}

// The suffixes of the English tables, and the words of the French and
// Spanish rules' alternations.
$english = new ReflectionClass(EnglishInflector::class);
foreach (['PLURAL_MAP', 'SINGULAR_MAP'] as $name) {
    foreach ($english->getConstant($name) as $map) {
        $seeds[] = strrev($map[0]);
        foreach ((array) $map[4] as $target) {
            $seeds[] = $target;
        }
    }
}
foreach ($english->getConstant('UNINFLECTED') as $word) {
    $seeds[] = strrev($word);
}
foreach ([FrenchInflector::class, SpanishInflector::class] as $class) {
    $reflection = new ReflectionClass($class);
    $patterns = [];
    foreach ($reflection->getConstants() as $value) {
        foreach ((array) $value as $rule) {
            $patterns[] = \is_array($rule) ? $rule[0] : $rule;
        }
    }
    foreach ($patterns as $pattern) {
        preg_match_all('/\(([^()]*)\)/', $pattern, $groups);
        foreach ($groups[1] as $group) {
            foreach (explode('|', $group) as $word) {
                if ('' !== $word && !preg_match('/[\\\\.*?{}]/', $word)) {
                    $seeds[] = $word;
                }
            }
        }
    }
}
$seeds = array_values(array_unique($seeds));

$corpus = ['', 's', 'S', 'es', 'x', "\n", "s\n", ' ', 'a b', "\xff", "\xffs", 'É', 'Ébauche', 'ÉMAIL', 'émaux', 'ÉMAUX', 'naïves', 'İs', 'ﬁles', '123s', 'O\'s'];
foreach ($seeds as $word) {
    $variants = [$word, ucfirst($word), strtoupper($word), strtolower($word), lcfirst(strtoupper($word)),
        'b'.$word, 'a'.$word, 'Xy'.$word, $word."\n", $word.'s', $word.'es', $word.'x', $word.' '.$word, 'é'.$word];
    foreach ($variants as $variant) {
        $corpus[] = $variant;
    }
}
$corpus = array_values(array_unique($corpus));

$inflectors = ['english' => new EnglishInflector(), 'french' => new FrenchInflector(), 'spanish' => new SpanishInflector()];
$out = [];
foreach ($inflectors as $name => $inflector) {
    $rows = [];
    foreach ($corpus as $word) {
        $rows[] = [
            base64_encode($word),
            array_map('base64_encode', $inflector->singularize($word)),
            array_map('base64_encode', $inflector->pluralize($word)),
        ];
    }
    $out[$name] = $rows;
}

$lines = [];
foreach ($out as $name => $rows) {
    $lines[] = '  '.json_encode($name).': ['."\n".implode(",\n", array_map(static fn ($r) => '    '.json_encode($r, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), $rows))."\n  ]";
}
echo "{\n", implode(",\n", $lines), "\n}\n";
