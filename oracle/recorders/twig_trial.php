<?php
// Twig trial (spikes/twig-trial): renders symfony/demo's
// blog/_post.html.twig (which includes blog/_post_tags.html.twig) with
// real Twig 3.30 and the ux-twig-component pre-lexer, for each case in
// spikes/twig-trial/demo/tests/data/cases.json, and the language snippets
// in spikes/twig-trial/demo/tests/snippets/. Output goes to
// spikes/twig-trial/demo/tests/data/php.json (scripts/record.sh twig-trial).
//
// The HTTP and provider bridges are stubbed the same way in PHP and in
// Rust (spikes/twig-trial/demo/src/lib.rs): path(), app.request,
// component() (UX Icons) and format_datetime (intl-extra). Their real
// output is not part of this trial.

if (is_file('/demo/vendor/autoload.php')) {
    require '/demo/vendor/autoload.php';
} else {
    // Without the demo's vendor/: the pinned checkouts in upstream/.
    require '/work/upstream/symfony/deprecation-contracts/function.php';
    spl_autoload_register(function (string $class): void {
        $roots = [
            'Twig\\' => '/work/upstream/twig/twig/src/',
            'Symfony\\UX\\TwigComponent\\' => '/work/upstream/symfony/ux-twig-component/src/',
        ];
        foreach ($roots as $prefix => $dir) {
            if (str_starts_with($class, $prefix)) {
                $file = $dir.str_replace('\\', '/', substr($class, strlen($prefix))).'.php';
                if (is_file($file)) {
                    require $file;
                }
            }
        }
    });
}

use Symfony\UX\TwigComponent\Twig\ComponentLexer;
use Twig\Environment;
use Twig\Loader\FilesystemLoader;
use Twig\TwigFilter;
use Twig\TwigFunction;

// Fixture classes with the public methods of the demo's entities
// (src/Entity/Post.php, Tag.php, User.php), so Twig's attribute lookup
// sees the same method lists as in the application.
final class Post
{
    public function __construct(
        private string $title, private string $slug, private string $summary,
        private \DateTimeImmutable $publishedAt, private User $author, private Collection $tags,
    ) {
    }
    public function getId(): ?int { return null; }
    public function getTitle(): string { return $this->title; }
    public function setTitle(string $t): void {}
    public function getSlug(): string { return $this->slug; }
    public function setSlug(string $s): void {}
    public function getContent(): ?string { return null; }
    public function setContent(?string $c): void {}
    public function getPublishedAt(): \DateTimeImmutable { return $this->publishedAt; }
    public function setPublishedAt(\DateTimeImmutable $d): void {}
    public function getAuthor(): User { return $this->author; }
    public function setAuthor(User $a): void {}
    public function getComments(): Collection { return new Collection([]); }
    public function addComment(mixed $c): void {}
    public function removeComment(mixed $c): void {}
    public function getSummary(): string { return $this->summary; }
    public function setSummary(string $s): void {}
    public function addTag(Tag ...$tags): void {}
    public function removeTag(Tag $tag): void {}
    public function getTags(): Collection { return $this->tags; }
}

final class Tag implements \JsonSerializable
{
    public function __construct(private string $name) {}
    public function getId(): ?int { return null; }
    public function getName(): string { return $this->name; }
    public function jsonSerialize(): string { return $this->name; }
    public function __toString(): string { return $this->name; }
}

final class User
{
    public function __construct(private string $fullName) {}
    public function getId(): ?int { return null; }
    public function setFullName(string $n): void {}
    public function getFullName(): string { return $this->fullName; }
}

// The part of Doctrine's ArrayCollection that the templates use.
final class Collection implements \IteratorAggregate, \Countable
{
    public function __construct(private array $items) {}
    public function isEmpty(): bool { return [] === $this->items; }
    public function count(): int { return \count($this->items); }
    public function toArray(): array { return $this->items; }
    public function getIterator(): \ArrayIterator { return new \ArrayIterator($this->items); }
}

// Stubs for the HTTP path (M2-A, done elsewhere): AppVariable,
// Request with its public $query, and InputBag::get().
final class InputBag
{
    public function __construct(private array $parameters) {}
    public function get(string $key, mixed $default = null): mixed { return $this->parameters[$key] ?? $default; }
    public function all(): array { return $this->parameters; }
}
final class Request
{
    public function __construct(public InputBag $query) {}
}
final class AppVariable
{
    public function __construct(private Request $request) {}
    public function getRequest(): Request { return $this->request; }
}

function stub_path(string $name, array $parameters = []): string
{
    $pairs = [];
    foreach ($parameters as $key => $value) {
        $pairs[] = $key.'='.(null === $value ? '~' : (string) $value);
    }

    return '/'.$name.'?'.implode('&', $pairs);
}

function stub_component(string $name, array $props = []): string
{
    $html = '<stub-component name="'.htmlspecialchars($name, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8').'"';
    foreach ($props as $key => $value) {
        $html .= ' '.$key.'="'.htmlspecialchars((string) $value, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8').'"';
    }

    return $html.'/>';
}

function stub_format_datetime(\DateTimeInterface $date, ?string $dateFormat = 'medium', ?string $timeFormat = 'medium', string $pattern = '', $timezone = null): string
{
    return \sprintf('[%s %s/%s/%s/%s]', $date->format('Y-m-d H:i:s'), $dateFormat, $timeFormat, $pattern, $timezone);
}

$loader = new FilesystemLoader('/work/spikes/twig-trial/demo/templates');
$loader->addPath('/work/spikes/twig-trial/demo/tests');
$twig = new Environment($loader, [
    'debug' => false,
    'strict_variables' => false,
    'autoescape' => 'name',
    'cache' => false,
]);
$twig->setLexer(new ComponentLexer($twig));
$twig->addFunction(new TwigFunction('path', 'stub_path'));
$twig->addFunction(new TwigFunction('component', 'stub_component', ['is_safe' => ['all']]));
$twig->addFilter(new TwigFilter('format_datetime', 'stub_format_datetime'));

$input = json_decode(file_get_contents('/work/spikes/twig-trial/demo/tests/data/cases.json'), true, flags: JSON_THROW_ON_ERROR);
$cases = [];
foreach ($input['cases'] as $case) {
    $p = $case['post'];
    $post = new Post(
        $p['title'], $p['slug'], $p['summary'],
        new \DateTimeImmutable($p['publishedAt'], new \DateTimeZone('UTC')),
        new User($p['author']),
        new Collection(array_map(fn (string $t) => new Tag($t), $p['tags'])),
    );
    $app = new AppVariable(new Request(new InputBag($case['query'])));
    $cases[] = [
        'name' => $case['name'],
        'output' => $twig->render('blog/_post.html.twig', ['post' => $post, 'app' => $app]),
    ];
}

// Language snippets (tests/snippets/*.twig), all with one context.
$snippetContext = json_decode(file_get_contents('/work/spikes/twig-trial/demo/tests/data/snippets.json'), true, flags: JSON_THROW_ON_ERROR)['context'];
$snippets = [];
foreach (glob('/work/spikes/twig-trial/demo/tests/snippets/*.twig') as $file) {
    $name = 'snippets/'.basename($file);
    $snippets[] = ['name' => $name, 'output' => $twig->render($name, $snippetContext)];
}

echo json_encode([
    'php' => PHP_VERSION,
    'twig' => Environment::VERSION,
    'cases' => $cases,
    'snippets' => $snippets,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
