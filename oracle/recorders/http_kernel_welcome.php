<?php
// RouterListener's welcome page for crates/http-kernel/resources/welcome.html
// (scripts/record.sh http-kernel-welcome): upstream's
// Resources/welcome.html.php rendered as RouterListener::createWelcomeResponse()
// renders it, with Kernel::MAJOR_VERSION.'.'.Kernel::MINOR_VERSION as the
// version and placeholders for what changes per response: @@PROJECT_DIR@@
// (realpath($projectDir).DIRECTORY_SEPARATOR, echoed as is) and @@HUE@@
// (random_int(0, 360)).

require '/demo/vendor/autoload.php';

use Symfony\Component\HttpKernel\Kernel;

$file = (new ReflectionClass(Kernel::class))->getFileName();
$template = file_get_contents(dirname($file).'/Resources/welcome.html.php');
$hue = '<?php echo random_int(0, 360); ?>';
if (1 !== substr_count($template, $hue) || str_contains($template, '@@')) {
    fwrite(STDERR, "unexpected welcome.html.php\n");
    exit(1);
}
$template = str_replace($hue, '@@HUE@@', $template);

$projectDir = '@@PROJECT_DIR@@';
$version = $docVersion = Kernel::MAJOR_VERSION.'.'.Kernel::MINOR_VERSION;

ob_start();
eval('?>'.$template);
echo ob_get_clean();
