<?php
// What symfony/clock and the PHP date functions under it answer, for
// crates/clock/tests/data/php.json (scripts/record.sh clock):
//
// - timezones: new DateTimeZone($name): getName() and timezone_type, or
//   the exception;
// - parse: date_parse() fields of each date text (fraction in
//   microseconds), and new DateTimeImmutable($text, $zone) in four zones:
//   the result when it does not depend on the current time (a full date,
//   or a time without relative hours), else only its zone;
// - modify: $base->modify($text) for each text on dates around DST
//   changes, in identifier, offset and abbreviation zones;
// - datepoint: new DatePoint($text, $zone, $reference) with fixed
//   references, so nothing depends on the current time;
// - format: format() of every format character on dates from year -9999
//   to 100000;
// - from_format: createFromFormat() results or getLastErrors(), and what
//   DatePoint::createFromFormat() throws;
// - timestamps: createFromTimestamp() of integers and floats (a fixed
//   seed), or the DateRangeError;
// - sleep: MockClock::sleep() and modify();
// - setters: setTime(), setDate(), setISODate(), setTimestamp(),
//   setMicrosecond() (native and DatePoint) and setTimezone().
//
// Dates are written 'Y-m-d H:i:s.u P e T I'; exceptions as
// {"class", "message"}. NAN and INF travel as strings.
//
// The oracle image loads the pinned packages from /demo/vendor. Without
// it (only the official php:8.4.27-cli-bookworm image, which the oracle
// image is built on), the classes are loaded from upstream/symfony/clock
// and PSR-20's one interface is declared here.

if (is_file('/demo/vendor/autoload.php')) {
    require '/demo/vendor/autoload.php';
} else {
    spl_autoload_register(static function (string $class): void {
        $prefix = 'Symfony\\Component\\Clock\\';
        if (str_starts_with($class, $prefix)) {
            require __DIR__.'/../../upstream/symfony/clock/'.str_replace('\\', '/', substr($class, strlen($prefix))).'.php';
        }
    });
    if (!interface_exists(Psr\Clock\ClockInterface::class)) {
        eval('namespace Psr\Clock; interface ClockInterface { public function now(): \DateTimeImmutable; }');
    }
    require __DIR__.'/../../upstream/symfony/clock/Resources/now.php';
}

use Symfony\Component\Clock\Clock;
use Symfony\Component\Clock\DatePoint;
use Symfony\Component\Clock\MockClock;

const F = 'Y-m-d H:i:s.u P e T I';

function thrown(Throwable $e): array
{
    return ['class' => $e::class, 'message' => $e->getMessage()];
}

function attempt(callable $f): mixed
{
    try {
        return $f();
    } catch (Throwable $e) {
        return thrown($e);
    }
}

function num(float|int $x): float|int|string
{
    if (is_float($x) && (is_nan($x) || is_infinite($x))) {
        return (string) $x;
    }

    return $x;
}

$out = ['default_timezone' => date_default_timezone_get()];

// Time zones.
$zones = ['UTC', 'utc', 'Utc', 'GMT', 'gmt', 'Z', 'z', 'UCT', 'CET', 'EST', 'CEST', 'Europe/Paris', 'europe/paris',
    'EUROPE/PARIS', 'Asia/Tokyo', 'America/New_York', 'America/Sao_Paulo', 'Japan', 'Zulu', 'Etc/UTC', 'Etc/GMT+5',
    'US/Eastern', 'Foo', 'Foo/Bar', '', ' UTC', 'UTC ', 'Europe/Paris ', 'utc+1', 'Etc/Unknown',
    '+02:00', '+0200', '+02', '+2', '-05:30', '-0530', '+02:00:30', '+00:00', '-00:00', '+0', '+14:00', '-12:00',
    '+24:00', '+25:00', '+26:00', '+99:00', '+99:59', '+99:99', '+100', '+12:60', '+1:30', '+123'];
foreach ($zones as $z) {
    $out['timezones'][] = ['name' => $z, 'result' => attempt(static function () use ($z) {
        $t = new DateTimeZone($z);
        $a = (array) new DateTimeImmutable('2020-01-01', $t);

        return ['name' => $t->getName(), 'type' => $a['timezone_type']];
    })];
}

// Date texts.
$texts = ['now', '', ' ', 'today', 'midnight', 'noon', 'tomorrow', 'yesterday', 'tomorrow noon', 'yesterday noon',
    'noon today', 'midnight noon', 'today +1 hour', 'midnight +1 hour', 'now +1 day', 'now now',
    '+1 day', '-1 day', '+1 days', '+2 days', '1 day', '10 days', '1 day ago', '+1 day ago', '-1 day ago', '+1 day ago ago',
    '+10 sec ago 2 min', '+1 day 2 hours', '+1 hour', '+2 hours', '+3 hours', '+24 hours', '+36 hours', '-1 hour', '+1 hours',
    '+30 minutes', '+90 min', '+1 min', '+2 mins', '+60 min', '+1 minute', '+1 sec', '+1 secs', '+1 second',
    '+3600 sec', '+1 seconds', '+999999999 sec', '+1 usec', '+5 microseconds', '+1 microsecond', '+1 msec', '+1 ms',
    '+1 millisecond', '+250 milliseconds', '+1 µs', '+1 week', '+2 weeks', '+1 fortnight', '+1 month', '-1 month',
    '+1 months', '+13 months', '+1 year', '+1 years', '-100 years', '+ 1 day', '+1day', '+1  day', "+1\tday",
    '+0 day', '+0 sec', '-0 day', '+007 days',
    '+1 hour 2016-09-08', '2016-09-08 +1 hour', '2016-09-08 -1 hour', '2016-09-08 +36 hours', '2016-09 +1 hour',
    '2016-09-08', '2016-9-8', '2016-09', '2016-1', '2024-04', '2024-04-09', '2024-04-09 03:00', '2024-04-09 00:00:00.123456',
    '2016-02-30', '2016-02-29', '2015-02-29', '2016-00-10', '2016-09-00', '2016-00-00', '2016-01-31 +1 month', '2016-01-31 -1 month',
    '+1 month 2016-01-31', '2016-12-31 23:59:59.999999 +1 usec', '2016-09-08T1030', '2016-09-08t1030',
    '2016-09-08 t1030', '2016-09-08 T1030', '2016-09-08 t1030 +1 day', '2016-09-08T10:30:00Z', '2016-09-08T10:30:00.5Z',
    '2016-09-08T10:30', '2016-09-08t10', '2016-09-08T103000',
    '2016-09-08 10:30:00.1234567', '2016-09-08 10:00:00.', '2016-09-08 10:00 Europe/Paris', '2016-09-08 10:00Europe/Paris',
    '2016-09-08 15:00 +02:00', '2016-09-08 15:00 +0200', '2016-09-08 15:00+02', '2016-09-08 15:00 -05:30', '2016-09-08 15:00-0530',
    '2010-01-28 15:00:00', '2010-01-28 15:00:00 UTC', '2010-01-28 15:00:00.123456', '2022-01-28 15:00:00 Europe/Paris',
    '2010-01-28 15:00:00 Z', '2010-01-28 15:00:00 GMT', '2010-01-28 15:00:00 Japan',
    '2016-10-30 02:30', '2016-10-30 02:30 Europe/Paris', '2016-10-30 02:30:00+02:00', '2016-10-30 02:30:00+01:00',
    '2016-03-27 02:30', '2016-03-27 02:30 Europe/Paris', '2016-11-06 01:30', '2016-11-06 01:30 America/New_York',
    '2016-03-13 02:30 America/New_York', '2016-04-03 02:30 Australia/Sydney', '2018-11-04', '2018-11-04 America/Sao_Paulo',
    '3000-01-01', '1800-01-01', '1800-01-01 Europe/Paris', '3000-07-01 Europe/Paris', '9999-12-31', '0000-01-01', '0001-01-01',
    '2112-09-17 23:53:03.001', '2112-09-17 23:53:00.999Z', '2016-10-30', '2000-01-01 12:00:00 UTC', '2016-10-30 00:30:00 UTC',
    '10:00', '1:02', '10:00:00', '10:00:00.5', '10:00Z', '10:00 Z', '10:00:00.5Z', '10:00 UTC', '10:00 +02:00', '10:00+02:00',
    '10:00 Europe/Paris', '24:00', '00:00:60', '23:59:60', '0:00', 't1030', 'T10:30', 't10', 't103000', 'T10:30:00.25',
    '02:30', '02:45', '15:00 +1 hour', 'midnight 10:00', '10:00 midnight', '10:00 noon', 'today 2016-09-08',
    '2016-09-08 today', '2016-09-08 noon', '2016-09-08 midnight', 'tomorrow 10:00',
    '@0', '@1234567', '@-1', '@-1.5', '@123.5', '@1.123456', '@1359188516.123456', ' @5 ', '@-100.25',
    'UTC', 'utc', 'Utc', 'GMT', 'gmt', 'Z', 'z', 'Europe/Paris', 'Europe/PARIS', 'Asia/Tokyo', 'America/Argentina/Buenos_Aires',
    'America/Port-au-Prince', 'Japan', 'japan', 'JAPAN', 'Cuba', 'Zulu', 'zulu', 'Israel', 'Eire', 'GB',
    'UTC Foo', 'UTC UTC', 'UTC Europe/Paris', 'Europe/Paris Foo', 'Europe/Paris UTC', 'Z Foo', '10:00 +02:00 Foo',
    '+1 day Europe/Paris', '+1 day UTC', 'now UTC', 'now Europe/Paris', 'Europe/Paris +1 day', 'Asia/Tokyo 10:00',
    'invalid date', 'Bad Date', 'Halloween', 'nowhere', 'todayx', 'utcx', 'Europe/Parisx', 'secondary', 'Singapore',
    'Hongkong', 'Iceland', 'Foo/Bar', 'europe/paris', 'EUROPE/PARIS', 'Foo UTC', 'foo', 'Foo 10:00', '10:00 Bar1',
    '2016-09-08 2016-09-09', '10:00 11:00', 'noon 10:00', 't1030 10:00', '2016-09-08T10:00 11:00',
    'mondays', 'monday', '+1 hourss', '1.5 days', '16-09-08', '10:00:00 PM', 'next monday', 'first day of next month',
    '+1 weekday', '+1 sat', 'Etc/GMT+5', 'EST5EDT', '+-1 day', '--1 day', '+ +1 day', '2016-09-08 25:00', '2016-13-01',
    'EST', 'cet', '2016-09-08 10', '@1.123456789', '@ 123', '+1 day @0', 'Europe/Paris @0', 'Israelx', 'midnightfoo',
    '+1 day,+1 hour', 't1030:00', '+99999999999999999999 sec', '2016', '20160908', '1 2', 'sec', 'day', '+1', '+02:00',
    '+1 day +02:00', '10:00 +2', '10:00 +99:99', '10:00 -', 'US/Eastern', '2016-09-08 10:00 US/Eastern', '10.30', '10:5',
    '2016-09-08 10:00:00,5', '2016/09/08', '08.09.2016', '9/8/2016', '2016-09-08 10:00:00 +02:00 Europe/Paris'];

$paramZones = ['UTC', 'Europe/Paris', 'America/New_York', '+05:30'];
foreach ($texts as $text) {
    $p = date_parse($text);
    $row = ['text' => $text, 'date_parse' => [
        'year' => $p['year'], 'month' => $p['month'], 'day' => $p['day'],
        'hour' => $p['hour'], 'minute' => $p['minute'], 'second' => $p['second'],
        'fraction' => false === $p['fraction'] ? false : (int) round($p['fraction'] * 1e6),
        'error_count' => $p['error_count'],
    ]];
    $timeOnly = false !== $p['hour'] && (!isset($p['relative']) || (0 === $p['relative']['hour'] && 0 === $p['relative']['minute'] && 0 === $p['relative']['second'] && 0 === $p['relative']['day']));
    $fullDate = false !== $p['year'] && false !== $p['month'] && false !== $p['day'];
    foreach ($paramZones as $z) {
        $row['ctor'][$z] = attempt(static function () use ($text, $z, $fullDate, $timeOnly) {
            $d = new DateTimeImmutable($text, new DateTimeZone($z));
            $a = (array) $d;
            $r = ['zone' => $d->getTimezone()->getName(), 'type' => $a['timezone_type']];
            if ($fullDate) {
                $r['date'] = $d->format(F);
            } elseif ($timeOnly) {
                $r['time'] = $d->format('H:i:s.u');
            }

            return $r;
        });
    }
    $out['parse'][] = $row;
}

// modify() on fixed dates.
$bases = [
    ['2010-01-28 15:00:00.123456', 'UTC'],
    ['2016-10-29 02:30:00', 'Europe/Paris'],
    ['2016-10-30 00:30:00 UTC', 'Europe/Paris'],
    ['2016-10-30 01:30:00 UTC', 'Europe/Paris'],
    ['2016-10-31 02:30:00', 'Europe/Paris'],
    ['2016-03-26 02:30:00.5', 'Europe/Paris'],
    ['2016-03-27 01:30:00.123456', 'Europe/Paris'],
    ['2016-11-06 05:30:00 UTC', 'America/New_York'],
    ['2016-11-06 06:30:00 UTC', 'America/New_York'],
    ['2016-03-12 02:30', 'America/New_York'],
    ['2016-04-02 15:30:00 UTC', 'Australia/Sydney'],
    ['2016-04-02 16:30:00 UTC', 'Australia/Sydney'],
    ['2024-01-10 22:20:02.654321', 'Asia/Tokyo'],
    ['2024-02-29 12:00', '+05:30'],
    ['2112-09-17 23:53:00.999Z', null],
    ['1969-12-31 23:59:59.5', 'UTC'],
    ['3000-01-01 05:06:07.123456', 'UTC'],
    ['1800-01-01 05:06:07.123456', 'Europe/Paris'],
    ['2018-11-03 12:00', 'America/Sao_Paulo'],
    ['2026-10-25 00:59:59.9 UTC', 'Europe/Madrid'],
];
$baseDates = [];
foreach ($bases as [$text, $zone]) {
    $d = new DateTimeImmutable($text, new DateTimeZone($zone ?? 'UTC'));
    if (null !== $zone && str_ends_with($text, 'UTC')) {
        $d = $d->setTimezone(new DateTimeZone($zone));
    }
    $baseDates[] = $d;
    $out['bases'][] = ['text' => $text, 'zone' => $zone, 'date' => $d->format(F)];
}
foreach ($baseDates as $i => $d) {
    foreach ($texts as $text) {
        $out['modify'][] = ['base' => $i, 'text' => $text, 'result' => attempt(static fn () => $d->modify($text)->format(F))];
    }
}

// new DatePoint() with a reference.
$pointTexts = ['now', '', 'today', 'midnight', 'noon', 'tomorrow', '+1 day', '+1 day Europe/Paris', '-1 hour', '+1 hour',
    '+1 second', '+36 hours', '1 day ago', '2016-09-08', '2016-09-08 +1 hour', '+1 hour 2016-09-08', '2016-09 +1 hour',
    '2016-09-08T1030', '2016-09-08 t1030 +1 day', '2018-11-04', '2016-10-30 02:30', '2016-10-30 02:30 Europe/Paris',
    '2016-10-30 02:30:00.5', '2016-11-06 01:30', '2016-11-06 01:30 America/New_York', '10:00', 't1030', '02:30', '02:30:00.25', '01:30',
    '10:00:00.5', 't10', 'T10:30', '24:00', '3000-01-01', '1800-01-01', '2010-01-28 15:00:00', '2010-01-28 15:00:00.123456',
    '2022-01-28 15:00:00 Europe/Paris', 'Europe/Paris', 'UTC', 'Z', 'Asia/Tokyo', '+1 day UTC', '10:00 UTC', '10:00 +02:00',
    '@0', '@1234567', '@-1.5', '2024-04', '2024-04-09 03:00', 'invalid date', 'Bad Date', ' ', 'Europe/PARIS', 'japan'];
$references = [
    ['2016-10-30 00:30:00 UTC', null],
    ['2016-10-30 01:30:00 UTC', null],
    ['2016-07-14 12:00:00 UTC', null],
    ['2016-01-14 12:00:00 UTC', null],
    ['2024-01-10 22:20:02.654321', 'UTC'],
    ['2024-01-10 22:20:02.654321', 'America/Sao_Paulo'],
    ['3000-01-01 05:06:07.123456', 'UTC'],
    ['1800-01-01 05:06:07.123456', 'UTC'],
    ['2016-11-06 05:30:00.5 UTC', 'America/New_York'],
    ['2016-11-06 06:30:00 UTC', 'America/New_York'],
    ['2016-10-29 23:30:00.75 UTC', 'Europe/Paris'],
];
$referenceDates = [];
foreach ($references as [$text, $zone]) {
    $d = new DateTimeImmutable($text, null !== $zone ? new DateTimeZone($zone) : null);
    if (null !== $zone && str_ends_with($text, 'UTC')) {
        $d = $d->setTimezone(new DateTimeZone($zone));
    }
    $referenceDates[] = $d;
    $out['references'][] = ['text' => $text, 'zone' => $zone, 'date' => $d->format(F)];
}
foreach ($pointTexts as $text) {
    foreach ($referenceDates as $i => $reference) {
        foreach ([null, 'UTC', 'Europe/Paris', 'America/New_York'] as $z) {
            $out['datepoint'][] = ['text' => $text, 'reference' => $i, 'zone' => $z, 'result' => attempt(
                static fn () => (new DatePoint($text, null !== $z ? new DateTimeZone($z) : null, $reference))->format(F)
            )];
        }
    }
}

// format().
$formatChars = str_split('dDjlNSwzWFmMntLoXxYyaABgGhHisuveIOPpTZcrU');
$formatString = implode('|', $formatChars).'|\\Y\\m|\\\\|%|é';
$instants = [[0, 0, 'UTC'], [-1, 999999, 'UTC'], [1359188516, 123456, 'UTC'], [1477787400, 0, 'Europe/Paris'],
    [1477791000, 0, 'Europe/Paris'], [1458960000, 500000, 'Europe/Paris'], [-5364662400, 0, 'Europe/Paris'],
    [-62135596800, 0, 'UTC'], [-62135596801, 0, 'UTC'], [-62167219200, 0, 'UTC'], [-62198755200, 1, 'UTC'],
    [-377705116800, 0, 'UTC'], [253402300799, 999999, 'UTC'], [253402300800, 0, 'UTC'], [3093527980800, 0, 'UTC'],
    [32503680000, 0, 'Europe/Paris'], [32519318400, 0, 'Europe/Paris'], [1704925202, 654321, 'Asia/Tokyo'],
    [1609632000, 0, 'UTC'], [1609718400, 0, 'UTC'], [1230681600, 0, 'UTC'], [1262217600, 0, 'UTC'], [1293840000, 0, 'UTC'],
    [1704067199, 0, 'America/Argentina/Buenos_Aires'], [1709208000, 0, '+05:30'], [1709208000, 0, '-00:30'],
    [1709208000, 0, '+02:00:30'], [1709208000, 0, 'Z'], [1709208000, 0, 'GMT'], [1709208000, 0, 'utc'],
    [1709208000, 0, 'europe/paris'], [1478413800, 0, 'America/New_York'], [1478410200, 0, 'America/New_York'],
    [43200, 0, 'UTC'], [3600, 0, 'UTC'], [82800, 0, 'UTC'], [-3600, 0, 'UTC'], [-1, 0, 'Asia/Tokyo'],
    [-9223372036854775807 - 1, 0, 'UTC'], [9223372036854775807, 999999, 'UTC'], [253402300800, 0, 'Europe/Paris'],
    [-377705116801, 0, 'Europe/Paris'], [10000000000000, 0, 'America/New_York'], [-10000000000000, 0, 'America/New_York']];
foreach ($instants as [$ts, $us, $zone]) {
    $d = (new DateTimeImmutable('@'.$ts))->setTimezone(new DateTimeZone($zone))->setMicrosecond($us);
    $out['format'][] = ['timestamp' => $ts, 'microsecond' => $us, 'zone' => $zone, 'format' => $formatString, 'result' => $d->format($formatString)];
}

// createFromFormat().
$fromFormat = [['Y-m-d H:i:s', '2010-01-28 15:00:00'], ['Y-m-d H:i:s', 'Bad Date'], ['Y-m-d H:i:s', '2010-01-28 15:00'],
    ['Y-m-d H:i:s', '2010-01-28 15:00:0'], ['Y-m-d H:i:s', '2010-01-28T15:00:00'], ['Y-m-d H:i:s', '2010-01-28  15:00:00'],
    ['Y-m-d H:i:s', '2010-01-28 1:00:00'], ['Y-m-d H:i:s', '2010-01-28 15:0x:00'], ['Y-m-d H:i:s', '2010-1-2 3:04:05'],
    ['Y-m-d H:i:s', 'x010-01-28 15:00:00'], ['Y-m-d H:i:s', '201-01-28 15:00:00'], ['Y-m-d H:i:s', '20100-01-28 15:00:00'],
    ['Y-m-d H:i:s', ''], ['Y-m-d H:i:s', '2010'], ['Y-m-d H:i:s', '2010-'], ['Y-m-d H:i:s', '2010-x1-28 15:00:00'],
    ['Y-m-d H:i:s', '2010-01-x8 15:00:00'], ['Y-m-d H:i:s', '2010-01-28 x5:00:00'], ['Y-m-d H:i:s', '2010-01-28 15:x0:00'],
    ['Y-m-d H:i:s', '2010-01-28 15:00:x0'], ['Y-m-d H:i:s', '2010-01-28 15:00:00.5'], ['Y-m-d H:i:s', '2010-01-28 24:00:00'],
    ['Y-m-d H:i:s', '2010-01-28 23:60:61'], ['Y-m-d H:i:s', '2010:01:28 15:00:00'], ['Y-m-d H:i:s', '2010-01-28 15-00-00'],
    ['Y-m-d H:i:s', '2010-01-28 15.00.00'], ['Y-m-d H:i:s', '-010-01-28 15:00:00'], ['Y-m-d H:i:s', '+2010-01-28 15:00:00'],
    ['Y-m-d H:i:s', '0000-00-00 00:00:00'], ['Y-m-d H:i:s', '2010-02-29 15:00:00'], ['Y-m-d H:i:s', '2010-01-28 15:00:00 '],
    ['Y-m-d H:i:s', 'B'], ['Y-m-d H:i:s', 'Bad'], ['Y-m-d H:i:s', 'x010'], ['Y-m-d H:i:s', 'Bad Date 15:00:00 xx'],
    ['Y-m-d H:i:s', 'xxxx-01-28 15:00:00'], ['Y-m-d H:i:s', 'x0'], ['Y-m-d H:i:s', 'Bad-Date'], ['Y-m-d H:i:s', '2010-01-28 15:00:00x'],
    ['Y-m-d H:i:s', '2016-10-30 02:30:00'], ['Y-m-d H:i:s', '2016-03-27 02:30:00'],
    ['Y-m-d H:i:s.u', '2010-01-28 15:00:00.1234567'], ['Y-m-d H:i:s.u', '2010-01-28 15:00:00.'], ['Y-m-d H:i:s.u', '2010-01-28 15:00:00.x'],
    ['Y-m-d H:i:s.u', '2010-01-28 15:00:00.5'], ['Y-m-d H:i:s.u', '2010-01-28 15:00:00.123456'], ['Y-m-d H:i:s.v', '2010-01-28 15:00:00.5'],
    ['Y-m-d H:i:s.v', '2010-01-28 15:00:00.1234'], ['Y-m-d H:i:s.v', '2010-01-28 15:00:00.123'],
    ['U.u', '1.5'], ['U.u', '1.'], ['U.u', '1'], ['U.u', '.5'], ['U.u', '+1.5'], ['U.u', '-0.5'], ['U.u', '-1.500000'],
    ['U.u', '1.1234567'], ['U.u', '12345678901234567890.5'], ['U.u', 'x'], ['U.u', ''], ['U.u', '1.x'], ['U.u', '1,5'],
    ['U.u', '1.000001'], ['U.u', '-62135596801.000000'], ['U.u', '4295923200.500000'], ['U', '1 '], ['U', '-5'], ['U', '0'], ['U', 'x'],
    ['U', '9223372036854775807'], ['U', '9223372036854775808'], ['U', '-9223372036854775808'],
    ['!Y-m-d', '2010-01-28'], ['Y-m-d|', '2010-01-28'], ['!H:i', '10:30'], ['!H:i:s', '25:00:00'], ['!Y', '2010'],
    ['!d/m/Y', '28/01/2010'], ['!Y-n-j G:i', '2010-1-2 3:04'], ['!Y-m-d', '2010-13-01'], ['!Y-m-d', '2010-01-32'],
    ['Y', 'B'], ['Y', 'x'], ['m', 'x'], ['d', 'x'], ['H', 'x'], ['i', 'x'], ['s', 'x'], ['u', 'x'], ['v', 'x'], ['U', 'B'],
    ['Y-m', 'Bx'], ['Y-m', 'B-'], ['Y', 'Bx'], ['!Y', '12345'], ['!Y', '123'], ['-Y', 'x'], ['-Y', '2010'], ['!(Y)', '(2010)'],
    ['!Y;m', '2010,01'], ['!Y,m', '2010,01']];
foreach ($fromFormat as [$format, $string]) {
    foreach (['UTC', 'Europe/Paris'] as $z) {
        $d = DateTimeImmutable::createFromFormat($format, $string, new DateTimeZone($z));
        $errors = DateTimeImmutable::getLastErrors();
        $out['from_format'][] = ['format' => $format, 'string' => $string, 'zone' => $z,
            'result' => false === $d ? false : $d->format(F),
            'last_errors' => $errors,
            'datepoint' => attempt(static fn () => DatePoint::createFromFormat($format, $string, new DateTimeZone($z))->format(F)),
        ];
    }
}

// createFromTimestamp().
mt_srand(20261125);
$timestamps = [1359188516, 1359188516.123456, 1359188516.0, 0, 0.0, -100, -100.123456, -100.0, -0.0, 0.5, -0.5,
    0.0000005, 0.0000004, -0.0000004, 1e15, 1.5e15 + 0.25, 9.2e18, -9.2e18, 9223372036854775807, -9223372036854775807 - 1,
    9223372036854775807.0, -9223372036854775808.0, 1e20, -1e20, NAN, INF, -INF, 1.5e19, 9.3e18, -9.3e18, 1.234567890123e25,
    1e300, -1e-300, 123.9999995, 123.9999994, -123.9999995];
for ($i = 0; $i < 300; ++$i) {
    $timestamps[] = (mt_rand(-2000000000, 2000000000) + mt_rand(0, 999999) / 1000000) * (mt_rand(0, 1) ? 1 : -1);
    $timestamps[] = mt_rand(-4000000000, 4000000000) / mt_rand(1, 1000);
}
foreach ($timestamps as $t) {
    $out['timestamps'][] = ['value' => num($t), 'int' => is_int($t), 'result' => attempt(static fn () => DatePoint::createFromTimestamp($t)->format(F))];
}

// MockClock.
$sleeps = [['2112-09-17 23:53:00.999Z', 'UTC', 2.002001], ['2024-01-01T00:00:00.5Z', 'UTC', 3], ['2024-01-01T00:00:00.999999Z', 'UTC', 0.000002],
    ['2024-01-01T00:00:00.5Z', 'UTC', 0.0000004], ['2024-01-01T00:00:00.5Z', 'UTC', 3600], ['1969-12-31T23:59:59.5Z', 'UTC', 0.75],
    ['1961-07-09T12:00:31.961111Z', 'UTC', 0.5], ['2026-10-25T00:59:59.9Z', 'Europe/Madrid', 0.2], ['2000-01-01 12:00:00 UTC', 'UTC', -10.5],
    ['2000-01-01 12:00:00 UTC', 'UTC', 0], ['2000-01-01 12:00:00 UTC', 'UTC', 0.0], ['2000-01-01 12:00:00 UTC', 'UTC', 1e20],
    ['2000-01-01 12:00:00 UTC', 'UTC', 1e10], ['2000-01-01 12:00:00.999999 UTC', 'Asia/Tokyo', 0.9999995],
    ['2016-10-30 00:30:00 UTC', 'Europe/Paris', 3600], ['2016-10-30 00:30:00 UTC', 'Europe/Paris', 1800.5],
    ['2016-11-06 05:30:00 UTC', 'America/New_York', 3600], ['0001-01-01 00:00:00.5 UTC', 'UTC', 0.75]];
for ($i = 0; $i < 50; ++$i) {
    $sleeps[] = ['@'.mt_rand(-3000000000, 3000000000).'.'.sprintf('%06d', mt_rand(0, 999999)), 'UTC', mt_rand(0, 100000000) / mt_rand(1, 10000)];
}
foreach ($sleeps as [$now, $zone, $seconds]) {
    $out['sleep'][] = ['now' => $now, 'zone' => $zone, 'seconds' => $seconds, 'int' => is_int($seconds), 'result' => attempt(static function () use ($now, $zone, $seconds) {
        $clock = new MockClock(new DateTimeImmutable($now), $zone);
        $clock->sleep($seconds);

        return $clock->now()->format(F);
    })];
}
foreach (['2112-09-17 23:53:03.001', '+2 days', 'Halloween', '', 'Europe/Paris', '@0', '+1 hour'] as $m) {
    $out['mock_modify'][] = ['text' => $m, 'result' => attempt(static function () use ($m) {
        $clock = new MockClock((new DateTimeImmutable('2112-09-17 23:53:00.999Z'))->setTimezone(new DateTimeZone('UTC')));
        $clock->modify($m);

        return $clock->now()->format(F);
    })];
}
foreach ([['now', null], ['Europe/Paris', null], ['now', 'Europe/Paris'], ['2022-06-20 00:00:00', null], ['2022-06-20 00:00:00', 'Asia/Tokyo'],
    ['2022-06-20 00:00:00 Asia/Tokyo', 'Europe/Paris'], ['invalid date', null], ['now', 'Foo']] as [$now, $zone]) {
    $out['mock_construct'][] = ['now' => $now, 'zone' => $zone, 'result' => attempt(static function () use ($now, $zone) {
        $d = (new MockClock($now, $zone))->now();

        return 'now' === $now || 'Europe/Paris' === $now ? $d->getTimezone()->getName() : $d->format(F);
    })];
}

// Setters.
$setterBases = [[1477787400, 0, 'Europe/Paris'], [1477791000, 250000, 'Europe/Paris'], [1478413800, 0, 'America/New_York'],
    [1478410200, 0, 'America/New_York'], [1264690800, 123456, 'UTC'], [1709208000, 5, '+05:30'], [1458963000, 0, 'Europe/Paris'],
    [1459036800, 0, 'Europe/Paris']];
$ops = [['setTime', [10, 0]], ['setTime', [2, 30]], ['setTime', [1, 30]], ['setTime', [2, 30, 15, 500]], ['setTime', [24, 0]],
    ['setTime', [-1, 0]], ['setTime', [25, 61, 61, 1000001]], ['setTime', [0, 0, 0, -1]], ['setDate', [2016, 10, 30]],
    ['setDate', [2016, 11, 6]], ['setDate', [2016, 3, 27]], ['setDate', [2016, 2, 30]], ['setDate', [2016, 0, 0]],
    ['setDate', [2016, 13, 1]], ['setDate', [-5, 1, 1]], ['setDate', [10000, 1, 1]], ['setISODate', [2016, 1]],
    ['setISODate', [2016, 1, 7]], ['setISODate', [2020, 53, 7]], ['setISODate', [2021, 53, 1]], ['setISODate', [2016, 0, 0]],
    ['setISODate', [2016, 60, 10]], ['setISODate', [2016, 43, 7]], ['setTimestamp', [0]], ['setTimestamp', [-1]],
    ['setTimestamp', [1477789200]], ['setMicrosecond', [0]], ['setMicrosecond', [999999]], ['setMicrosecond', [1000000]],
    ['setMicrosecond', [-1]], ['setTimezone', ['Asia/Tokyo']], ['setTimezone', ['+01:00']], ['setTimezone', ['Z']]];
foreach ($setterBases as $i => [$ts, $us, $zone]) {
    $base = (new DateTimeImmutable('@'.$ts))->setTimezone(new DateTimeZone($zone))->setMicrosecond($us);
    $out['setter_bases'][] = ['timestamp' => $ts, 'microsecond' => $us, 'zone' => $zone, 'date' => $base->format(F)];
    $point = DatePoint::createFromInterface($base);
    foreach ($ops as [$method, $args]) {
        $call = static fn ($d) => $d->$method(...('setTimezone' === $method ? [new DateTimeZone($args[0])] : $args))->format(F);
        $out['setters'][] = ['base' => $i, 'method' => $method, 'args' => $args,
            'native' => attempt(static fn () => $call($base)),
            'datepoint' => attempt(static fn () => $call($point)),
        ];
    }
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR), "\n";
