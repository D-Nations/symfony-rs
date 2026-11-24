<?php
// Catalogue operations beyond the PHPUnit tests: fallbacks, ICU collisions,
// numeric/empty domains and ids, null values, metadata, every migration batch,
// error classes, lazy caches and shared input/result identity.
// scripts/record.sh translation-catalogue; pinned oracle only, no host PHP.
if (in_array('--without-intl', $argv, true)) {
    // Composer loads an intl polyfill even under php -n. Load the pinned
    // files directly here so MessageFormatter really is absent.
    foreach (['MessageCatalogueInterface.php', 'MetadataAwareInterface.php', 'CatalogueMetadataAwareInterface.php', 'Exception/ExceptionInterface.php', 'Exception/LogicException.php', 'Exception/InvalidArgumentException.php', 'MessageCatalogue.php', 'Catalogue/OperationInterface.php', 'Catalogue/AbstractOperation.php', 'Catalogue/MergeOperation.php', 'Catalogue/TargetOperation.php'] as $file) {
        require '/work/upstream/symfony/translation/'.$file;
    }
} else {
    require '/demo/vendor/autoload.php';
}
require __DIR__.'/lib/typed.php';

use Symfony\Component\Translation\Catalogue\MergeOperation;
use Symfony\Component\Translation\Catalogue\TargetOperation;
use Symfony\Component\Translation\MessageCatalogue;

function output_cases(array $cases, bool $intl): void {
    // One scenario per line keeps the differential corpus reviewable.
    echo '{"php":', json_encode(PHP_VERSION), ',"intl":', $intl ? 'true' : 'false', ',"pin":"d9e1caba0d6b6f9a26710af8a2f88d37f001215a","cases":[', "\n";
    foreach ($cases as $i => $case) {
        if ($i) { echo ",\n"; }
        echo json_encode($case, JSON_UNESCAPED_SLASHES|JSON_THROW_ON_ERROR);
    }
    echo "\n]}\n";
}
function make_catalogue(array $spec): MessageCatalogue {
    [$locale, $messages, $metadata, $catalogueMetadata, $fallback] = $spec;
    $c = new MessageCatalogue($locale, $messages);
    foreach ($metadata as $domain => $pairs) {
        foreach ($pairs as $id => $value) { $c->setMetadata((string) $id, $value, (string) $domain); }
    }
    foreach ($catalogueMetadata as $domain => $pairs) {
        foreach ($pairs as $id => $value) { $c->setCatalogueMetadata((string) $id, $value, (string) $domain); }
    }
    if (null !== $fallback) { $c->addFallbackCatalogue(make_catalogue($fallback)); }
    return $c;
}
function snapshot(MessageCatalogue $c): array {
    return [(new ReflectionProperty(MessageCatalogue::class, 'messages'))->getValue($c),
        $c->getMetadata('', ''), $c->getCatalogueMetadata('', ''),
        array_map(fn ($r) => (string) $r, $c->getResources()), null !== $c->getFallbackCatalogue()];
}
function record_case(string $kind, array $source, array $target, array $steps, bool $intl = true): array {
    $s = make_catalogue($source); $t = make_catalogue($target);
    try {
        $op = 'merge' === $kind ? new MergeOperation($s, $t) : new TargetOperation($s, $t);
    } catch (Throwable $e) {
        return [$kind, typed_value($source), typed_value($target), $steps, $intl, [['throws', $e::class, $e->getMessage()]]];
    }
    $out = [];
    foreach ($steps as $step) {
        $out[] = typed_call(function () use ($op, $s, $t, $step, $intl) {
            return match ($step[0]) {
                'domains' => $op->getDomains(),
                'all' => $op->getMessages($step[1]),
                'new' => $op->getNewMessages($step[1]),
                'obsolete' => $op->getObsoleteMessages($step[1]),
                'result' => snapshot($op->getResult()),
                'move' => $op->moveMessagesToIntlDomainsIfPossible($step[1]),
                'source' => $s->set($step[1], $step[2], $step[3]),
                'target' => $t->set($step[1], $step[2], $step[3]),
                'result_set' => $op->getResult()->set($step[1], $step[2], $step[3]),
            };
        });
    }
    return [$kind, typed_value($source), typed_value($target), $steps, $intl, $out];
}
$spec = fn ($messages = [], $metadata = [], $catMetadata = [], $fallback = null, $locale = 'en') => [$locale, $messages, $metadata, $catMetadata, $fallback];
$fixtures = [
    $spec(),
    $spec(['messages' => ['a' => 'plain-a', 'b' => 'plain-b', '0' => '', '01' => 'leading-zero']], ['messages' => ['a' => ['note' => 'plain'], 'b' => null]], ['messages' => ['format' => 'plain', '0' => 'zero-key', '' => 'empty-key']]),
    $spec(['messages+intl-icu' => ['a' => 'icu-a', 'c' => 'icu-c']], ['messages+intl-icu' => ['a' => ['note' => 'icu']]], ['messages+intl-icu' => ['format' => 'icu', 'nullable' => null]]),
    $spec(['messages' => ['a' => 'plain', 'b' => 'b'], 'messages+intl-icu' => ['a' => 'icu', 'c' => 'c'], 'empty' => []], ['messages' => ['a' => 'plain-meta'], 'messages+intl-icu' => ['c' => 'icu-meta']], ['messages' => ['keep' => 'base'], 'messages+intl-icu' => ['keep' => 'intl']]),
    $spec(['messages+intl-icu+intl-icu' => ['nested' => 'twice'], 'messages+intl-icu' => ['a' => 'once'], '' => ['' => 'empty']], [], ['' => ['e' => 'empty-domain'], 'messages+intl-icu+intl-icu' => ['nested' => 'twice']]),
    $spec(['messages' => ['b' => 'local']], [], [], $spec(['messages' => ['a' => 'fallback', 'c' => 'fallback-c']], [], [], null, 'fr')),
    $spec(['messages' => ['a' => null, '-1' => 'negative', 'c' => '0']], ['messages' => ['a' => null, 'c' => []]], ['messages' => ['null' => null, 'empty' => []]]),
    $spec(['0' => ['0' => 'zero'], '01' => ['01' => 'leading'], '-1+intl-icu' => ['a' => 'numeric'], 'messages' => []]),
];
$cases=[];
// php -n runs the same pinned oracle without extension configuration. This
// exercises the actual class_exists() early return, rather than simulating it.
if (!class_exists(\MessageFormatter::class)) {
    foreach (['merge', 'target'] as $kind) {
        foreach (['all', 'invalid'] as $batch) {
            $cases[] = record_case($kind,$fixtures[1],$fixtures[2],[['move',$batch],['domains'],['result']],false);
        }
    }
    output_cases($cases, false);
    exit;
}
foreach (['merge', 'target'] as $kind) {
    foreach ($fixtures as $si => $source) {
        foreach ($fixtures as $ti => $target) {
            foreach (['all', 'new', 'obsolete', 'invalid'] as $batch) {
                // Rotate early domain reads: process order is observable for ICU.
                $domain = ($si + $ti) % 2 ? 'messages+intl-icu' : 'messages';
                $steps = [['domains'], ['all', $domain], ['new', $domain], ['obsolete', $domain], ['result'], ['move', $batch], ['result'], ['all', $domain], ['all', 'missing'], ['new', 'missing'], ['obsolete', 'missing'], ['all', '0'], ['all', '-1'], ['all', '01'], ['result']];
                $cases[] = record_case($kind,$source,$target,$steps);
            }
        }
    }
    $cases[] = record_case($kind,$spec(),$spec(locale:'fr'),[]);
    foreach ([[], [['domains']], [['all','messages']]] as $prefix) {
        $steps = [...$prefix, ['source','a','source-mutated','messages'], ['target','c','target-mutated','messages'], ['source','late','late-domain','late'], ['domains'], ['all','messages'], ['result'], ['source','a','cached-unchanged','messages'], ['all','messages'], ['result_set','a','shared-result','messages'], ['result'], ['all','messages'], ['all','late']];
        $cases[] = record_case($kind,$fixtures[1],$fixtures[2],$steps);
    }
    $cases[] = record_case($kind,$fixtures[1],$fixtures[2],[['move','new'],['move','all'],['domains'],['result']]);
    $cases[] = record_case($kind,$spec(),$spec(),[['move','invalid'],['result']]);
}
output_cases($cases, true);
